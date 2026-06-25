//! Integration tests for the second batch of repository aggregates
//! shipped in Phase 2 milestone 3: `pool_meta`, `connection_session`,
//! `treasury`, `share_window`, `share_allocation`, `nacho_rebate`,
//! and the `payout` family (cycle + payout + KRC-20).

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_arithmetic
)]

use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;

use katpool_db::repo::block::BlockStatus;
use katpool_db::repo::payout::{Krc20TransferStatus, PayoutCycleStatus, PayoutKind, PayoutStatus};
use katpool_db::repo::{
    block, coinbase_reward, connection_session, nacho_rebate, payout, pool_meta, share_allocation,
    share_window, treasury, wallet, worker,
};
use katpool_db::{PoolConfig, build_pool, migrate};
use katpool_domain::{BlockHash, CorrelationId, DaaScore, WalletAddress, WorkerName};
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
        application_name: "katpool-db-payouts-test".to_owned(),
    };

    let pool = build_pool(&cfg).await.expect("pool");
    migrate::run(&pool).await.expect("migrate");
    (pool, container)
}

fn sample_wallet_addr() -> WalletAddress {
    WalletAddress::new("kaspa:qz4j8mu269z8llgcczmfukm9fan2fq822kzxu4cfukd5fqrhxpsv2zhs9jxnp")
        .expect("valid")
}

fn second_wallet_addr() -> WalletAddress {
    WalletAddress::new("kaspa:qrxd24c5w6pl2qa9k7q5e0lyepuu4r5t2f6awvxllk0a83qqfys9rhxpsv2zhs9j")
        .expect("valid")
}

fn sample_worker_name() -> WorkerName {
    WorkerName::new("rig-01").expect("valid")
}

// ---- pool_meta ------------------------------------------------------

#[tokio::test]
async fn pool_meta_set_then_get_roundtrip() {
    let (pool, _ctr) = fresh_pool().await;
    let row = pool_meta::set(&pool, "last_daa_processed", "12345")
        .await
        .expect("set");
    assert_eq!(row.value, "12345");

    let fetched = pool_meta::get(&pool, "last_daa_processed")
        .await
        .expect("get")
        .expect("present");
    assert_eq!(fetched.value, "12345");
    assert_eq!(fetched.key, "last_daa_processed");
}

#[tokio::test]
async fn pool_meta_set_is_idempotent_and_refreshes_timestamp() {
    let (pool, _ctr) = fresh_pool().await;
    let first = pool_meta::set(&pool, "k", "v1").await.expect("first");
    tokio::time::sleep(Duration::from_millis(50)).await;
    let second = pool_meta::set(&pool, "k", "v2").await.expect("second");
    assert_eq!(second.value, "v2");
    assert!(second.updated_at > first.updated_at);
}

#[tokio::test]
async fn pool_meta_get_returns_none_for_missing_key() {
    let (pool, _ctr) = fresh_pool().await;
    let result = pool_meta::get(&pool, "nonexistent").await.expect("query");
    assert!(result.is_none());
}

// ---- connection_session --------------------------------------------

#[tokio::test]
async fn session_open_and_close() {
    let (pool, _ctr) = fresh_pool().await;
    let id = connection_session::open(
        &pool,
        None,
        IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
        Some("test-miner"),
        None,
        chrono::Utc::now(),
    )
    .await
    .expect("open");

    // The freshly-opened row counts as active until closed.
    let active = connection_session::active_summary(&pool)
        .await
        .expect("active");
    assert_eq!(active.sessions, 1);
    assert_eq!(active.workers, 0);

    connection_session::close(&pool, id).await.expect("close");
    // Re-close is idempotent.
    connection_session::close(&pool, id)
        .await
        .expect("close again");

    // Closed rows are no longer active.
    let active = connection_session::active_summary(&pool)
        .await
        .expect("active after close");
    assert_eq!(active.sessions, 0);
}

#[tokio::test]
async fn close_all_open_finalizes_orphans() {
    let (pool, _ctr) = fresh_pool().await;
    for n in 1..=3 {
        connection_session::open(
            &pool,
            None,
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, n)),
            None,
            None,
            chrono::Utc::now(),
        )
        .await
        .expect("open");
    }
    let closed = connection_session::close_all_open(&pool)
        .await
        .expect("sweep");
    assert_eq!(closed, 3);
    let active = connection_session::active_summary(&pool)
        .await
        .expect("active");
    assert_eq!(active.sessions, 0);
    // Idempotent: a second sweep closes nothing.
    assert_eq!(
        connection_session::close_all_open(&pool)
            .await
            .expect("sweep2"),
        0
    );
}

#[tokio::test]
async fn session_open_with_worker_then_increment_counters() {
    let (pool, _ctr) = fresh_pool().await;
    let w = wallet::ensure(&pool, &sample_wallet_addr(), "mainnet")
        .await
        .expect("wallet");
    let wk = worker::ensure(&pool, w.id, &sample_worker_name())
        .await
        .expect("worker");

    // worker_id is bound at open (authorize), so list_for_worker finds the
    // live session immediately — no post-open backfill step.
    let sid = connection_session::open(
        &pool,
        Some(wk.id),
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        Some("rig"),
        None,
        chrono::Utc::now(),
    )
    .await
    .expect("open");
    connection_session::increment_counters(&pool, sid, 100, 2, 1)
        .await
        .expect("inc");
    connection_session::increment_counters(&pool, sid, 50, 0, 0)
        .await
        .expect("inc2");

    let sessions = connection_session::list_for_worker(&pool, wk.id, 10)
        .await
        .expect("list");
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].shares_credited, 150);
    assert_eq!(sessions[0].shares_rejected, 2);
    assert_eq!(sessions[0].malformed_frames, 1);
    assert_eq!(sessions[0].remote_ip, "127.0.0.1");
}

// ---- treasury -------------------------------------------------------

#[tokio::test]
async fn treasury_insert_then_latest() {
    let (pool, _ctr) = fresh_pool().await;
    let id1 = treasury::insert(
        &pool,
        1_000,
        50_000,
        100,
        90,
        Some("post-rotation snapshot"),
    )
    .await
    .expect("insert 1");
    tokio::time::sleep(Duration::from_millis(50)).await;
    let id2 = treasury::insert(&pool, 2_000, 60_000, 200, 180, None)
        .await
        .expect("insert 2");
    assert_ne!(id1, id2);

    let latest = treasury::latest(&pool)
        .await
        .expect("latest")
        .expect("present");
    assert_eq!(latest.kas_balance_sompi, 2_000);
    assert_eq!(latest.nacho_balance, 60_000);
    assert!(latest.notes.is_none());
    // The full-field insert path does not populate utxo_count.
    assert_eq!(latest.utxo_count, None);
}

#[tokio::test]
async fn treasury_insert_snapshot_records_utxo_count() {
    let (pool, _ctr) = fresh_pool().await;
    let id = treasury::insert_snapshot(&pool, 1_234_567, 61_205, 9_000, Some("consolidation tick"))
        .await
        .expect("insert snapshot");
    assert!(id > 0);

    let latest = treasury::latest(&pool)
        .await
        .expect("latest")
        .expect("present");
    assert_eq!(latest.kas_balance_sompi, 1_234_567);
    assert_eq!(latest.utxo_count, Some(61_205));
    assert_eq!(latest.daa_score, 9_000);
    // Consolidation snapshots do not observe these fields.
    assert_eq!(latest.nacho_balance, 0);
    assert_eq!(latest.blue_score, 0);
    assert_eq!(latest.notes.as_deref(), Some("consolidation tick"));
}

#[tokio::test]
async fn treasury_list_recent_orders_newest_first() {
    let (pool, _ctr) = fresh_pool().await;
    for i in 0..3 {
        treasury::insert(&pool, 1_000 + i, 0, i, i, None)
            .await
            .expect("insert");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let listed = treasury::list_recent(&pool, 10).await.expect("list");
    assert_eq!(listed.len(), 3);
    assert!(listed[0].captured_at > listed[2].captured_at);
}

// ---- share_window ---------------------------------------------------

#[tokio::test]
async fn share_window_insert_then_find() {
    let (pool, _ctr) = fresh_pool().await;
    let w = wallet::ensure(&pool, &sample_wallet_addr(), "mainnet")
        .await
        .expect("wallet");
    let now = chrono::Utc::now();
    let _ = share_window::insert(
        &pool,
        w.id,
        DaaScore::new(100),
        DaaScore::new(200),
        now,
        now + chrono::Duration::seconds(60),
        500.5,
        42,
    )
    .await
    .expect("insert");

    let fetched = share_window::find(&pool, w.id, DaaScore::new(100), DaaScore::new(200))
        .await
        .expect("find")
        .expect("present");
    assert_eq!(fetched.share_count, 42);
    assert!((fetched.total_weight - 500.5).abs() < 1e-9);
}

#[tokio::test]
async fn share_window_unique_constraint_rejects_duplicates() {
    let (pool, _ctr) = fresh_pool().await;
    let w = wallet::ensure(&pool, &sample_wallet_addr(), "mainnet")
        .await
        .expect("wallet");
    let now = chrono::Utc::now();
    share_window::insert(
        &pool,
        w.id,
        DaaScore::new(0),
        DaaScore::new(10),
        now,
        now,
        1.0,
        1,
    )
    .await
    .expect("first");
    let err = share_window::insert(
        &pool,
        w.id,
        DaaScore::new(0),
        DaaScore::new(10),
        now,
        now,
        2.0,
        2,
    )
    .await
    .expect_err("must reject duplicate window");
    assert_eq!(
        err.sqlstate(),
        Some("23505"),
        "expected unique_violation: {err:?}"
    );
}

// ---- share_allocation -----------------------------------------------

/// Build the full lifecycle fixture used by the share-allocation
/// tests: a matured `block` row (telemetry) plus the matured
/// `coinbase_reward` row that PROP allocation now anchors on. Returns
/// `(block_id, coinbase_reward_id, wallet_id)`.
async fn make_matured_block(
    pool: &sqlx::PgPool,
) -> (
    katpool_db::repo::BlockId,
    katpool_db::repo::CoinbaseRewardId,
    katpool_db::repo::WalletId,
) {
    let w = wallet::ensure(pool, &sample_wallet_addr(), "mainnet")
        .await
        .expect("wallet");
    let wk = worker::ensure(pool, w.id, &sample_worker_name())
        .await
        .expect("worker");
    let hash = BlockHash::from_bytes([7_u8; 32]);
    let id = block::insert(
        pool,
        hash,
        w.id,
        wk.id,
        DaaScore::new(1),
        0,
        CorrelationId::new_v4(),
    )
    .await
    .expect("insert block");
    block::mark_submitted(pool, hash).await.expect("submit");
    block::mark_confirmed_blue(pool, hash, Some(1))
        .await
        .expect("confirm");
    block::mark_matured(pool, hash, 5_000_000_000)
        .await
        .expect("mature");
    let (reward_id, _) = coinbase_reward::ensure(pool, &[7_u8; 32], 0, 5_000_000_000, 1)
        .await
        .expect("coinbase reward");
    (id, reward_id, w.id)
}

#[tokio::test]
async fn share_allocation_balance_check_enforced() {
    let (pool, _ctr) = fresh_pool().await;
    let (_block_id, reward_id, wallet_id) = make_matured_block(&pool).await;

    // Unbalanced row — client-side rejection before SQL.
    let bad = share_allocation::NewAllocation {
        wallet_id,
        weight: 1.0,
        window_total: 1.0,
        gross_share_sompi: 1_000,
        pool_fee_sompi: 100,
        nacho_accrual_sompi: 100,
        net_payout_sompi: 100, // sum is 300, not 1_000
        applied_topline_bps: 75,
        applied_rebate_bps: 3_300,
        applied_tier: share_allocation::DbWalletTier::Standard,
    };
    let err = share_allocation::insert_batch(&pool, reward_id, &[bad])
        .await
        .expect_err("client-side balance check");
    let msg = format!("{err:?}");
    assert!(msg.contains("unbalanced"), "{msg}");
}

#[tokio::test]
async fn share_allocation_insert_batch_round_trip() {
    let (pool, _ctr) = fresh_pool().await;
    let (_block_id, reward_id, wallet_id_a) = make_matured_block(&pool).await;

    let w2 = wallet::ensure(&pool, &second_wallet_addr(), "mainnet")
        .await
        .expect("wallet 2");

    // Block reward 5e9 sompi; two wallets, 60/40 split.
    // Topline 75bps (0.75%), standard tier (33% rebate of fee_share):
    //   gross  = 3e9  → fee_share=22_500_000 → nacho=7_425_000 →
    //                   pool_fee=15_075_000  → net=2_977_500_000
    //   gross  = 2e9  → fee_share=15_000_000 → nacho=4_950_000 →
    //                   pool_fee=10_050_000  → net=1_985_000_000
    let rows = vec![
        share_allocation::NewAllocation {
            wallet_id: wallet_id_a,
            weight: 60.0,
            window_total: 100.0,
            gross_share_sompi: 3_000_000_000,
            pool_fee_sompi: 15_075_000,
            nacho_accrual_sompi: 7_425_000,
            net_payout_sompi: 2_977_500_000,
            applied_topline_bps: 75,
            applied_rebate_bps: 3_300,
            applied_tier: share_allocation::DbWalletTier::Standard,
        },
        share_allocation::NewAllocation {
            wallet_id: w2.id,
            weight: 40.0,
            window_total: 100.0,
            gross_share_sompi: 2_000_000_000,
            pool_fee_sompi: 10_050_000,
            nacho_accrual_sompi: 4_950_000,
            net_payout_sompi: 1_985_000_000,
            applied_topline_bps: 75,
            applied_rebate_bps: 3_300,
            applied_tier: share_allocation::DbWalletTier::Standard,
        },
    ];

    let inserted = share_allocation::insert_batch(&pool, reward_id, &rows)
        .await
        .expect("insert batch");
    assert_eq!(inserted, 2);

    let listed = share_allocation::list_for_reward(&pool, reward_id)
        .await
        .expect("list");
    assert_eq!(listed.len(), 2);
    // list_for_reward is weight DESC, so wallet_id_a (60) comes before w2 (40).
    assert_eq!(listed[0].wallet_id, wallet_id_a);

    let total = listed.iter().map(|a| a.gross_share_sompi).sum::<i64>();
    assert_eq!(total, 5_000_000_000, "all 5 KAS should be allocated");

    let pending_a = share_allocation::pending_balance_for_wallet(&pool, wallet_id_a)
        .await
        .expect("pending");
    assert_eq!(pending_a, 2_977_500_000);
}

#[tokio::test]
async fn share_allocation_db_check_constraint_rejects_unbalanced_directly() {
    // Bypass our client-side guard to confirm the DB CHECK still bites.
    let (pool, _ctr) = fresh_pool().await;
    let (_block_id, reward_id, wallet_id) = make_matured_block(&pool).await;

    let err = sqlx::query(
        "INSERT INTO share_allocation
            (coinbase_reward_id, wallet_id, weight, window_total,
             gross_share_sompi, pool_fee_sompi, nacho_accrual_sompi, net_payout_sompi,
             applied_topline_bps, applied_rebate_bps, applied_tier)
         VALUES ($1, $2, 1.0, 1.0, 1000, 1, 1, 1, 75, 3300, 'standard')",
    )
    .bind(reward_id.0)
    .bind(wallet_id.0)
    .execute(&pool)
    .await
    .expect_err("DB CHECK must reject");
    assert_eq!(
        err.as_database_error()
            .and_then(sqlx::error::DatabaseError::code)
            .as_deref(),
        Some("23514")
    );
}

// ---- nacho_rebate ---------------------------------------------------

#[tokio::test]
async fn nacho_rebate_accrue_and_pay() {
    let (pool, _ctr) = fresh_pool().await;
    let w = wallet::ensure(&pool, &sample_wallet_addr(), "mainnet")
        .await
        .expect("wallet");

    let r1 = nacho_rebate::accrue(&pool, w.id, 1_000)
        .await
        .expect("accrue 1");
    assert_eq!(r1.accrued_sompi, 1_000);
    assert_eq!(r1.paid_sompi, 0);
    assert_eq!(r1.pending_sompi(), 1_000);

    let r2 = nacho_rebate::accrue(&pool, w.id, 500)
        .await
        .expect("accrue 2");
    assert_eq!(r2.accrued_sompi, 1_500);
    assert_eq!(r2.pending_sompi(), 1_500);

    let r3 = nacho_rebate::mark_paid(&pool, w.id, 800)
        .await
        .expect("pay");
    assert_eq!(r3.paid_sompi, 800);
    assert_eq!(r3.pending_sompi(), 700);
}

#[tokio::test]
async fn nacho_rebate_pay_exceeding_accrued_fails() {
    let (pool, _ctr) = fresh_pool().await;
    let w = wallet::ensure(&pool, &sample_wallet_addr(), "mainnet")
        .await
        .expect("wallet");
    let _ = nacho_rebate::accrue(&pool, w.id, 1_000)
        .await
        .expect("accrue");
    let err = nacho_rebate::mark_paid(&pool, w.id, 2_000)
        .await
        .expect_err("paid > accrued must be rejected");
    assert_eq!(
        err.sqlstate(),
        Some("23514"),
        "expected CHECK violation: {err:?}"
    );
}

#[tokio::test]
async fn nacho_rebate_accrue_rejects_negative_delta() {
    let (pool, _ctr) = fresh_pool().await;
    let w = wallet::ensure(&pool, &sample_wallet_addr(), "mainnet")
        .await
        .expect("wallet");
    let err = nacho_rebate::accrue(&pool, w.id, -1)
        .await
        .expect_err("negative");
    let msg = format!("{err:?}");
    assert!(msg.contains("non-negative"), "{msg}");
}

#[tokio::test]
async fn nacho_rebate_list_pending_excludes_below_threshold() {
    let (pool, _ctr) = fresh_pool().await;
    let w1 = wallet::ensure(&pool, &sample_wallet_addr(), "mainnet")
        .await
        .expect("w1");
    let w2 = wallet::ensure(&pool, &second_wallet_addr(), "mainnet")
        .await
        .expect("w2");
    let _ = nacho_rebate::accrue(&pool, w1.id, 10_000)
        .await
        .expect("acc 1");
    let _ = nacho_rebate::accrue(&pool, w2.id, 500)
        .await
        .expect("acc 2");

    let pending = nacho_rebate::list_pending(&pool, 1_000, 10)
        .await
        .expect("list");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].wallet_id, w1.id);
}

// ---- payout cycle + payout -----------------------------------------

#[tokio::test]
async fn payout_cycle_create_is_idempotent() {
    let (pool, _ctr) = fresh_pool().await;
    let c1 = payout::create_cycle(
        &pool,
        PayoutKind::Kas,
        DaaScore::new(100),
        DaaScore::new(200),
    )
    .await
    .expect("create");
    let c2 = payout::create_cycle(
        &pool,
        PayoutKind::Kas,
        DaaScore::new(100),
        DaaScore::new(200),
    )
    .await
    .expect("create again");
    assert_eq!(c1.id, c2.id);
    assert_eq!(c1.idempotency_key, "kas-100-200");
    assert_eq!(c1.status, PayoutCycleStatus::Planned);
}

#[tokio::test]
async fn payout_cycle_lifecycle_transitions() {
    let (pool, _ctr) = fresh_pool().await;
    let c = payout::create_cycle(
        &pool,
        PayoutKind::Krc20Nacho,
        DaaScore::new(1),
        DaaScore::new(2),
    )
    .await
    .expect("create");

    payout::mark_cycle_broadcasting(&pool, c.id)
        .await
        .expect("broadcasting");
    payout::mark_cycle_partially_settled(&pool, c.id)
        .await
        .expect("partial");
    payout::mark_cycle_settled(&pool, c.id)
        .await
        .expect("settled");

    let fetched = payout::get_cycle(&pool, c.id).await.expect("get");
    assert_eq!(fetched.status, PayoutCycleStatus::Settled);
    assert!(fetched.broadcast_at.is_some());
    assert!(fetched.settled_at.is_some());
}

#[tokio::test]
async fn payout_uniqueness_per_cycle_and_wallet() {
    let (pool, _ctr) = fresh_pool().await;
    let w = wallet::ensure(&pool, &sample_wallet_addr(), "mainnet")
        .await
        .expect("wallet");
    let c = payout::create_cycle(&pool, PayoutKind::Kas, DaaScore::new(1), DaaScore::new(2))
        .await
        .expect("cycle");

    let _ = payout::insert_payout(&pool, c.id, w.id, 1_000)
        .await
        .expect("first");
    let err = payout::insert_payout(&pool, c.id, w.id, 2_000)
        .await
        .expect_err("must reject duplicate");
    assert_eq!(err.sqlstate(), Some("23505"));
}

#[tokio::test]
async fn payout_lifecycle_submit_and_confirm() {
    let (pool, _ctr) = fresh_pool().await;
    let w = wallet::ensure(&pool, &sample_wallet_addr(), "mainnet")
        .await
        .expect("wallet");
    let c = payout::create_cycle(&pool, PayoutKind::Kas, DaaScore::new(1), DaaScore::new(2))
        .await
        .expect("cycle");
    let p = payout::insert_payout(&pool, c.id, w.id, 1_000)
        .await
        .expect("payout");

    let tx_hash = BlockHash::from_bytes([42_u8; 32]);
    payout::mark_payout_submitted(&pool, p.id, tx_hash)
        .await
        .expect("submit");
    payout::mark_payout_confirmed(&pool, p.id)
        .await
        .expect("confirm");

    let after = payout::get_payout(&pool, p.id).await.expect("get");
    assert_eq!(after.status, PayoutStatus::Confirmed);
    assert!(after.submitted_at.is_some());
    assert!(after.confirmed_at.is_some());
    assert_eq!(after.tx_hash.as_deref().map(<[u8]>::len), Some(32));
}

#[tokio::test]
async fn payout_mark_failed_records_reason() {
    let (pool, _ctr) = fresh_pool().await;
    let w = wallet::ensure(&pool, &sample_wallet_addr(), "mainnet")
        .await
        .expect("wallet");
    let c = payout::create_cycle(&pool, PayoutKind::Kas, DaaScore::new(1), DaaScore::new(2))
        .await
        .expect("cycle");
    let p = payout::insert_payout(&pool, c.id, w.id, 1_000)
        .await
        .expect("payout");

    payout::mark_payout_failed(&pool, p.id, "mempool rejected")
        .await
        .expect("fail");

    let after = payout::get_payout(&pool, p.id).await.expect("get");
    assert_eq!(after.status, PayoutStatus::Failed);
    assert_eq!(after.failure_reason.as_deref(), Some("mempool rejected"));
}

// ---- krc20 ----------------------------------------------------------

#[tokio::test]
async fn krc20_lifecycle_transitions() {
    let (pool, _ctr) = fresh_pool().await;
    let w = wallet::ensure(&pool, &sample_wallet_addr(), "mainnet")
        .await
        .expect("wallet");
    let c = payout::create_cycle(
        &pool,
        PayoutKind::Krc20Nacho,
        DaaScore::new(1),
        DaaScore::new(2),
    )
    .await
    .expect("cycle");
    let p = payout::insert_payout(&pool, c.id, w.id, 1_000)
        .await
        .expect("payout");

    let _ = payout::insert_krc20_pending(
        &pool,
        p.id,
        1_000,
        50_000,
        "kaspa:qz4j8mu269z8llgcczmfukm9fan2fq822kzxu4cfukd5fqrhxpsv2zhs9jxnp",
    )
    .await
    .expect("krc20 pending");

    payout::mark_krc20_commit_submitted(&pool, p.id)
        .await
        .expect("commit");
    payout::mark_krc20_reveal_submitted(&pool, p.id)
        .await
        .expect("reveal");
    payout::mark_krc20_completed(&pool, p.id)
        .await
        .expect("complete");

    let pending = payout::list_krc20_by_status(&pool, &[Krc20TransferStatus::Completed], 10)
        .await
        .expect("list");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].payout_id, p.id);
}

#[tokio::test]
async fn krc20_commit_reveal_hashes_persist_on_the_payout_row() {
    let (pool, _ctr) = fresh_pool().await;
    let w = wallet::ensure(&pool, &sample_wallet_addr(), "mainnet")
        .await
        .expect("wallet");
    let c = payout::create_cycle(
        &pool,
        PayoutKind::Krc20Nacho,
        DaaScore::new(10),
        DaaScore::new(20),
    )
    .await
    .expect("cycle");
    let p = payout::insert_payout(&pool, c.id, w.id, 1_000)
        .await
        .expect("payout");

    assert!(
        payout::get_payout(&pool, p.id)
            .await
            .expect("get")
            .krc20_commit_hash
            .is_none()
    );

    let commit = BlockHash::from_bytes([0xAB; 32]);
    let reveal = BlockHash::from_bytes([0xCD; 32]);
    payout::record_krc20_commit_hash(&pool, p.id, commit)
        .await
        .expect("record commit");
    payout::record_krc20_reveal_hash(&pool, p.id, reveal)
        .await
        .expect("record reveal");

    let row = payout::get_payout(&pool, p.id).await.expect("reload");
    assert_eq!(
        row.krc20_commit_hash.as_deref(),
        Some(commit.as_bytes().as_slice())
    );
    assert_eq!(
        row.krc20_reveal_hash.as_deref(),
        Some(reveal.as_bytes().as_slice())
    );

    // Idempotent re-record with the same deterministic hash is a no-op overwrite.
    payout::record_krc20_commit_hash(&pool, p.id, commit)
        .await
        .expect("re-record commit");
    let row = payout::get_payout(&pool, p.id).await.expect("reload 2");
    assert_eq!(
        row.krc20_commit_hash.as_deref(),
        Some(commit.as_bytes().as_slice())
    );
}

#[tokio::test]
async fn krc20_payout_unique_per_payout() {
    let (pool, _ctr) = fresh_pool().await;
    let w = wallet::ensure(&pool, &sample_wallet_addr(), "mainnet")
        .await
        .expect("wallet");
    let c = payout::create_cycle(
        &pool,
        PayoutKind::Krc20Nacho,
        DaaScore::new(1),
        DaaScore::new(2),
    )
    .await
    .expect("cycle");
    let p = payout::insert_payout(&pool, c.id, w.id, 1_000)
        .await
        .expect("payout");

    let _ = payout::insert_krc20_pending(
        &pool,
        p.id,
        1_000,
        50_000,
        "kaspa:qz4j8mu269z8llgcczmfukm9fan2fq822kzxu4cfukd5fqrhxpsv2zhs9jxnp",
    )
    .await
    .expect("first");
    let err = payout::insert_krc20_pending(
        &pool,
        p.id,
        2_000,
        100_000,
        "kaspa:qrxd24c5w6pl2qa9k7q5e0lyepuu4r5t2f6awvxllk0a83qqfys9rhxpsv2zhs9j",
    )
    .await
    .expect_err("must reject duplicate payout-id");
    assert_eq!(err.sqlstate(), Some("23505"));
}

#[tokio::test]
async fn kas_eligible_wallets_subtracts_confirmed_kas_payouts() {
    let (pool, _ctr) = fresh_pool().await;
    let (_block_id, reward_id, wallet_id) = make_matured_block(&pool).await;

    let row = share_allocation::NewAllocation {
        wallet_id,
        weight: 1.0,
        window_total: 1.0,
        gross_share_sompi: 1_000_000_000,
        pool_fee_sompi: 5_000_000,
        nacho_accrual_sompi: 2_000_000,
        net_payout_sompi: 993_000_000,
        applied_topline_bps: 75,
        applied_rebate_bps: 3_300,
        applied_tier: share_allocation::DbWalletTier::Standard,
    };
    share_allocation::insert_batch(&pool, reward_id, &[row])
        .await
        .expect("alloc");

    let eligible = payout::list_kas_eligible_wallets(&pool, 500_000_000)
        .await
        .expect("eligible");
    assert_eq!(eligible.len(), 1);
    assert_eq!(eligible[0].payable_sompi, 993_000_000);

    let cycle = payout::create_cycle(&pool, PayoutKind::Kas, DaaScore::new(1), DaaScore::new(2))
        .await
        .expect("cycle");
    let p = payout::insert_payout(&pool, cycle.id, wallet_id, 200_000_000)
        .await
        .expect("payout");
    payout::mark_payout_submitted(&pool, p.id, BlockHash::from_bytes([1_u8; 32]))
        .await
        .expect("submit");
    // Submitted but not confirmed — still fully payable.
    let mid = payout::list_kas_eligible_wallets(&pool, 500_000_000)
        .await
        .expect("mid");
    assert_eq!(mid[0].payable_sompi, 993_000_000);

    payout::mark_payout_confirmed(&pool, p.id)
        .await
        .expect("confirm");
    let after = payout::list_kas_eligible_wallets(&pool, 500_000_000)
        .await
        .expect("after");
    assert_eq!(after[0].confirmed_paid_sompi, 200_000_000);
    assert_eq!(after[0].payable_sompi, 793_000_000);

    let cycle2 = payout::create_cycle(&pool, PayoutKind::Kas, DaaScore::new(10), DaaScore::new(11))
        .await
        .expect("cycle2");
    let ensured = payout::ensure_payout(&pool, cycle2.id, wallet_id, 793_000_000)
        .await
        .expect("ensure");
    let again = payout::ensure_payout(&pool, cycle2.id, wallet_id, 999_000_000)
        .await
        .expect("ensure again");
    assert_eq!(ensured.id, again.id);
    assert_eq!(again.amount_sompi, 793_000_000);
}

#[tokio::test]
async fn kas_eligible_excludes_legacy_imported_payouts() {
    // A legacy payout imported at cutover (cycle keyed `kas-legacy-<hash>`)
    // settles pre-cutover earnings that were never imported into
    // share_allocation. It must NOT reduce the post-cutover payable balance —
    // otherwise every migrated wallet shows a massively negative payable and
    // the payout engine never selects it (regression: cutover balance bug).
    let (pool, _ctr) = fresh_pool().await;
    let (_block_id, reward_id, wallet_id) = make_matured_block(&pool).await;

    let row = share_allocation::NewAllocation {
        wallet_id,
        weight: 1.0,
        window_total: 1.0,
        gross_share_sompi: 1_000_000_000,
        pool_fee_sompi: 5_000_000,
        nacho_accrual_sompi: 2_000_000,
        net_payout_sompi: 993_000_000,
        applied_topline_bps: 75,
        applied_rebate_bps: 3_300,
        applied_tier: share_allocation::DbWalletTier::Standard,
    };
    share_allocation::insert_batch(&pool, reward_id, &[row])
        .await
        .expect("alloc");

    // Insert a legacy-tagged cutover cycle directly — create_cycle only mints
    // `kas-<daa>-<daa>` keys; the importer uses `kas-legacy-<hash>`.
    sqlx::query(
        "INSERT INTO payout_cycle (kind, daa_start, daa_end, idempotency_key)
         VALUES ('kas'::payout_kind, 0, 1, 'kas-legacy-deadbeef')",
    )
    .execute(&pool)
    .await
    .expect("insert legacy cycle");
    let legacy = payout::find_cycle_by_idempotency_key(&pool, "kas-legacy-deadbeef")
        .await
        .expect("find legacy cycle")
        .expect("legacy cycle exists");

    // A large confirmed legacy payout that would drive payable negative if counted.
    let p = payout::insert_payout(&pool, legacy.id, wallet_id, 5_000_000_000)
        .await
        .expect("legacy payout");
    payout::mark_payout_submitted(&pool, p.id, BlockHash::from_bytes([2_u8; 32]))
        .await
        .expect("submit");
    payout::mark_payout_confirmed(&pool, p.id)
        .await
        .expect("confirm");

    // Legacy payout is excluded: payable stays at the post-cutover allocation.
    let eligible = payout::list_kas_eligible_wallets(&pool, 500_000_000)
        .await
        .expect("eligible");
    assert_eq!(eligible.len(), 1);
    assert_eq!(eligible[0].confirmed_paid_sompi, 0);
    assert_eq!(eligible[0].payable_sompi, 993_000_000);

    // Single-wallet path agrees.
    let bal = payout::kas_payable_for_wallet(&pool, wallet_id)
        .await
        .expect("balance");
    assert_eq!(bal.confirmed_paid_sompi, 0);
    assert_eq!(bal.payable_sompi, 993_000_000);
}

#[tokio::test]
async fn idempotency_key_format_is_stable() {
    assert_eq!(
        payout::idempotency_key(PayoutKind::Kas, DaaScore::new(100), DaaScore::new(200)),
        "kas-100-200"
    );
    assert_eq!(
        payout::idempotency_key(
            PayoutKind::Krc20Nacho,
            DaaScore::new(0),
            DaaScore::new(u64::from(u32::MAX))
        ),
        format!("krc20-0-{}", u64::from(u32::MAX))
    );
}

// ---- session keeps block + wallet anchored --------------------------

#[tokio::test]
async fn block_status_filter_with_extras() {
    // Sanity-check that `block::list_by_status` works for the new
    // workflow (planned + matured for the accountant's view).
    let (pool, _ctr) = fresh_pool().await;
    let (block_id, _reward_id, _wallet_id) = make_matured_block(&pool).await;
    let matured = block::list_by_status(&pool, &[BlockStatus::Matured], 5)
        .await
        .expect("list");
    assert_eq!(matured.len(), 1);
    assert_eq!(matured[0].id, block_id);
}
