//! Integration tests for the read-only repository functions added for the
//! Phase 6 public HTTP API (ADR-0021): per-worker stats, active-participant
//! counts, hashrate time-series, paginated block/cycle listings, single-wallet
//! KAS payable balance, pool-wide payout totals, per-wallet payout history, and
//! the persisted-tier lookup behind `/full_rebate`.
//!
//! Each `#[tokio::test]` spins up its own ephemeral postgres container, applies
//! the migrations, and exercises one contract against a real driver + query
//! planner + constraint engine (ADR-0013 layer 3 — no mocks).

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_arithmetic,
    clippy::cast_precision_loss
)]

use std::time::Duration;

use chrono::{Duration as ChronoDuration, Utc};
use katpool_db::repo::block::BlockStatus;
use katpool_db::repo::payout::{PayoutCycleStatus, PayoutKind};
use katpool_db::repo::share_allocation::{DbWalletTier, NewAllocation};
use katpool_db::repo::{
    block, coinbase_reward, payout, share, share_allocation, share_stats, wallet, worker,
};
use katpool_db::{PoolConfig, build_pool, migrate};
use katpool_domain::{
    BlockHash, CorrelationId, DaaScore, ShareDifficulty, WalletAddress, WorkerName,
};
use testcontainers::ContainerAsync;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;

async fn fresh_pool() -> (sqlx::PgPool, ContainerAsync<Postgres>) {
    let container = Postgres::default().start().await.expect("start postgres");
    let port = container.get_host_port_ipv4(5432).await.expect("port");
    let url = format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres");

    let cfg = PoolConfig {
        url,
        min_connections: 1,
        max_connections: 4,
        acquire_timeout: Duration::from_secs(10),
        idle_timeout: Duration::from_secs(60),
        max_lifetime: Duration::from_secs(300),
        statement_timeout: Duration::from_secs(30),
        application_name: "katpool-db-api-reads-test".to_owned(),
    };

    let pool = build_pool(&cfg).await.expect("pool");
    migrate::run(&pool).await.expect("migrate");
    (pool, container)
}

fn wallet_one() -> WalletAddress {
    WalletAddress::new("kaspa:qz4j8mu269z8llgcczmfukm9fan2fq822kzxu4cfukd5fqrhxpsv2zhs9jxnp")
        .expect("valid")
}

fn wallet_two() -> WalletAddress {
    WalletAddress::new("kaspa:qrxd24c5w6pl2qa9k7q5e0lyepuu4r5t2f6awvxllk0a83qqfys9rhxpsv2zhs9j")
        .expect("valid")
}

fn diff(d: f64) -> ShareDifficulty {
    ShareDifficulty::new(d).expect("valid difficulty")
}

const fn block_hash(byte: u8) -> BlockHash {
    BlockHash::from_bytes([byte; 32])
}

// ---- share_stats: per-worker + active counts ------------------------

#[tokio::test]
async fn per_worker_stats_and_active_counts() {
    let (pool, _ctr) = fresh_pool().await;
    let w = wallet::ensure(&pool, &wallet_one(), "mainnet")
        .await
        .expect("wallet");
    let rig_a = worker::ensure(&pool, w.id, &WorkerName::new("rig-a").unwrap())
        .await
        .expect("worker a");
    let rig_b = worker::ensure(&pool, w.id, &WorkerName::new("rig-b").unwrap())
        .await
        .expect("worker b");

    // rig-a: two shares (1000 + 3000 weight); rig-b: one share (500).
    for d in [1000.0, 3000.0] {
        share::insert_credited(
            &pool,
            w.id,
            rig_a.id,
            None,
            diff(d),
            DaaScore::new(100),
            CorrelationId::new_v4(),
        )
        .await
        .expect("insert share a");
    }
    share::insert_credited(
        &pool,
        w.id,
        rig_b.id,
        None,
        diff(500.0),
        DaaScore::new(100),
        CorrelationId::new_v4(),
    )
    .await
    .expect("insert share b");

    let since = Utc::now() - ChronoDuration::hours(1);
    let until = Utc::now() + ChronoDuration::hours(1);

    let a = share_stats::accepted_for_worker(&pool, rig_a.id, since)
        .await
        .expect("accepted a");
    assert_eq!(a.share_count, 2);
    assert!((a.total_weight - 4000.0).abs() < f64::EPSILON);

    let b = share_stats::accepted_for_worker(&pool, rig_b.id, since)
        .await
        .expect("accepted b");
    assert_eq!(b.share_count, 1);
    assert!((b.total_weight - 500.0).abs() < f64::EPSILON);

    // Worker hashrate: weight * 2^32 / window_secs, strictly positive.
    let hr = share_stats::hashrate_estimate_for_worker(&pool, rig_a.id, since, until)
        .await
        .expect("hashrate a");
    assert!(hr > 0.0);

    // until <= since is rejected.
    assert!(
        share_stats::hashrate_estimate_for_worker(&pool, rig_a.id, until, since)
            .await
            .is_err()
    );

    let counts = share_stats::active_participant_counts(&pool, since)
        .await
        .expect("counts");
    assert_eq!(counts.wallets, 1);
    assert_eq!(counts.workers, 2);
}

// ---- share_stats: hashrate series -----------------------------------

#[tokio::test]
async fn hashrate_series_reconstructs_total_weight() {
    let (pool, _ctr) = fresh_pool().await;
    let w = wallet::ensure(&pool, &wallet_one(), "mainnet")
        .await
        .expect("wallet");
    let rig = worker::ensure(&pool, w.id, &WorkerName::new("rig-a").unwrap())
        .await
        .expect("worker");

    let weights = [1000.0_f64, 2000.0, 4096.0];
    for d in weights {
        share::insert_credited(
            &pool,
            w.id,
            rig.id,
            None,
            diff(d),
            DaaScore::new(100),
            CorrelationId::new_v4(),
        )
        .await
        .expect("insert share");
    }
    let total: f64 = weights.iter().sum();

    let from = Utc::now() - ChronoDuration::hours(1);
    let until = Utc::now() + ChronoDuration::hours(1);
    let bucket_secs = 3600_i64;

    // Reconstruct summed difficulty from the series: weight == hr * bucket / 2^32.
    let two_pow_32 = 4_294_967_296.0_f64;
    for series in [
        share_stats::hashrate_series_pool_wide(&pool, from, until, bucket_secs)
            .await
            .expect("pool series"),
        share_stats::hashrate_series_for_wallet(&pool, w.id, from, until, bucket_secs)
            .await
            .expect("wallet series"),
    ] {
        assert!(!series.is_empty(), "series must have at least one bucket");
        let reconstructed: f64 = series
            .iter()
            .map(|p| p.hashrate * bucket_secs as f64 / two_pow_32)
            .sum();
        assert!(
            (reconstructed - total).abs() < 1e-3,
            "reconstructed {reconstructed} != total {total}"
        );
        // Buckets are epoch-aligned and ascending.
        for pair in series.windows(2) {
            assert!(pair[0].bucket_start < pair[1].bucket_start);
        }
    }

    // Argument validation.
    assert!(
        share_stats::hashrate_series_pool_wide(&pool, until, from, bucket_secs)
            .await
            .is_err()
    );
    assert!(
        share_stats::hashrate_series_pool_wide(&pool, from, until, 0)
            .await
            .is_err()
    );
}

// ---- block: recent + count_by_status --------------------------------

#[tokio::test]
async fn block_recent_and_status_counts() {
    let (pool, _ctr) = fresh_pool().await;
    let w = wallet::ensure(&pool, &wallet_one(), "mainnet")
        .await
        .expect("wallet");
    let rig = worker::ensure(&pool, w.id, &WorkerName::new("rig-a").unwrap())
        .await
        .expect("worker");

    for i in 0..3u8 {
        block::insert(
            &pool,
            block_hash(i + 1),
            w.id,
            rig.id,
            DaaScore::new(200 + u64::from(i)),
            u64::from(i),
            CorrelationId::new_v4(),
        )
        .await
        .expect("insert block");
    }
    // Advance one block to matured so count_by_status spans two states.
    block::mark_submitted(&pool, block_hash(1))
        .await
        .expect("sub");
    block::mark_confirmed_blue(&pool, block_hash(1), Some(5))
        .await
        .expect("conf");
    block::mark_matured(&pool, block_hash(1), 5_000)
        .await
        .expect("mat");

    let counts = block::count_by_status(&pool).await.expect("counts");
    let found = counts
        .iter()
        .find(|(s, _)| *s == BlockStatus::Found)
        .map(|(_, c)| *c);
    let matured = counts
        .iter()
        .find(|(s, _)| *s == BlockStatus::Matured)
        .map(|(_, c)| *c);
    assert_eq!(found, Some(2));
    assert_eq!(matured, Some(1));

    // Newest-first, keyset pagination.
    let page1 = block::list_recent(&pool, 2, None).await.expect("page1");
    assert_eq!(page1.len(), 2);
    assert!(page1[0].id.0 > page1[1].id.0);
    let cursor = page1[1].id.0;
    let page2 = block::list_recent(&pool, 2, Some(cursor))
        .await
        .expect("page2");
    assert_eq!(page2.len(), 1);
    assert!(page2[0].id.0 < cursor);
}

// ---- payout: balances, totals, listings -----------------------------

#[tokio::test]
async fn payout_balances_totals_and_history() {
    let (pool, _ctr) = fresh_pool().await;
    let w1 = wallet::ensure(&pool, &wallet_one(), "mainnet")
        .await
        .expect("wallet1");
    let w2 = wallet::ensure(&pool, &wallet_two(), "mainnet")
        .await
        .expect("wallet2");

    // One matured coinbase reward, allocated to w1 (net 900, Elite tier).
    let (reward_id, _) = coinbase_reward::ensure(&pool, &[7u8; 32], 0, 5_000, 300)
        .await
        .expect("reward");
    share_allocation::insert_batch(
        &pool,
        reward_id,
        &[NewAllocation {
            wallet_id: w1.id,
            weight: 10.0,
            window_total: 10.0,
            gross_share_sompi: 1_000,
            pool_fee_sompi: 75,
            nacho_accrual_sompi: 25,
            net_payout_sompi: 900,
            applied_topline_bps: 75,
            applied_rebate_bps: 10_000,
            applied_tier: DbWalletTier::Elite,
        }],
    )
    .await
    .expect("alloc");

    // A KAS cycle with one confirmed payout of 200 to w1.
    let cycle = payout::create_cycle(
        &pool,
        PayoutKind::Kas,
        DaaScore::new(0),
        DaaScore::new(1_000),
    )
    .await
    .expect("cycle");
    let p = payout::insert_payout(&pool, cycle.id, w1.id, 200)
        .await
        .expect("payout");
    payout::mark_payout_submitted(&pool, p.id, block_hash(9))
        .await
        .expect("submit");
    payout::mark_payout_confirmed(&pool, p.id)
        .await
        .expect("confirm");

    // Single-wallet KAS payable = allocated(900) - confirmed(200) = 700.
    let bal = payout::kas_payable_for_wallet(&pool, w1.id)
        .await
        .expect("payable");
    assert_eq!(bal.allocated_sompi, 900);
    assert_eq!(bal.confirmed_paid_sompi, 200);
    assert_eq!(bal.payable_sompi, 700);

    // Unknown-but-valid wallet with no allocation reads all-zero.
    let bal2 = payout::kas_payable_for_wallet(&pool, w2.id)
        .await
        .expect("payable2");
    assert_eq!(bal2.payable_sompi, 0);

    // Pool totals: 200 KAS confirmed, 0 NACHO, 1 confirmed payout.
    let totals = payout::pool_payout_totals(&pool).await.expect("totals");
    assert_eq!(totals.kas_confirmed_sompi, 200);
    assert_eq!(totals.nacho_confirmed_sompi, 0);
    assert_eq!(totals.confirmed_payouts, 1);

    // Recent cycles.
    let cycles = payout::list_recent_cycles(&pool, 10, None)
        .await
        .expect("cycles");
    assert_eq!(cycles.len(), 1);
    assert_eq!(cycles[0].kind, PayoutKind::Kas);
    assert!(matches!(
        cycles[0].status,
        PayoutCycleStatus::Planned | PayoutCycleStatus::Broadcasting
    ));

    // Per-wallet detailed history carries the cycle kind.
    let hist = payout::list_for_wallet_detailed(&pool, w1.id, 10, None)
        .await
        .expect("history");
    assert_eq!(hist.len(), 1);
    assert_eq!(hist[0].kind, PayoutKind::Kas);
    assert_eq!(hist[0].amount_sompi, 200);

    // Persisted tier behind /full_rebate.
    let tier = share_allocation::latest_applied_tier_for_wallet(&pool, w1.id)
        .await
        .expect("tier");
    assert_eq!(tier, Some(DbWalletTier::Elite));
    let none = share_allocation::latest_applied_tier_for_wallet(&pool, w2.id)
        .await
        .expect("tier none");
    assert_eq!(none, None);
}
