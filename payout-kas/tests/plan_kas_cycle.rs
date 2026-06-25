//! Integration tests for KAS cycle planning (M4.3) and the restart-safe
//! cycle state machine (M4.4).

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_arithmetic
)]

use std::time::Duration;

use katpool_db::repo::payout::{self, PayoutCycleStatus, PayoutKind, PayoutStatus};
use katpool_db::repo::{audit, block, coinbase_reward, share_allocation, wallet, worker};
use katpool_db::{PoolConfig, build_pool, migrate};
use katpool_domain::{BlockHash, CorrelationId, DaaScore, WalletAddress, WorkerName};
use payout_kas::{
    DEFAULT_KAS_PAYOUT_THRESHOLD_SOMPI, PlanKasCycleParams, plan_kas_cycle, reconcile_cycle_status,
    resume_or_plan_kas_cycle,
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
        application_name: "payout-kas-plan-test".to_owned(),
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

async fn seed_two_wallet_allocations(
    pool: &sqlx::PgPool,
) -> (
    katpool_db::repo::BlockId,
    katpool_db::repo::WalletId,
    katpool_db::repo::WalletId,
) {
    let w1 = wallet::ensure(pool, &sample_wallet_addr(), "mainnet")
        .await
        .expect("wallet 1");
    let wk = worker::ensure(pool, w1.id, &sample_worker_name())
        .await
        .expect("worker");
    let w2 = wallet::ensure(pool, &second_wallet_addr(), "mainnet")
        .await
        .expect("wallet 2");

    let hash = BlockHash::from_bytes([9_u8; 32]);
    let block_id = block::insert(
        pool,
        hash,
        w1.id,
        wk.id,
        DaaScore::new(1),
        0,
        CorrelationId::new_v4(),
    )
    .await
    .expect("block");
    block::mark_submitted(pool, hash).await.expect("submit");
    block::mark_confirmed_blue(pool, hash, Some(1))
        .await
        .expect("confirm");
    block::mark_matured(pool, hash, 5_000_000_000)
        .await
        .expect("mature");
    let (reward_id, _) = coinbase_reward::ensure(pool, &[9_u8; 32], 0, 5_000_000_000, 1)
        .await
        .expect("coinbase reward");

    let rows = vec![
        share_allocation::NewAllocation {
            wallet_id: w1.id,
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
    share_allocation::insert_batch(pool, reward_id, &rows)
        .await
        .expect("allocations");

    (block_id, w1.id, w2.id)
}

#[tokio::test]
async fn kas_eligible_wallets_respects_threshold_and_confirmed_paid() {
    let (pool, _ctr) = fresh_pool().await;
    let (_block_id, w1, w2) = seed_two_wallet_allocations(&pool).await;

    let eligible = payout::list_kas_eligible_wallets(&pool, DEFAULT_KAS_PAYOUT_THRESHOLD_SOMPI)
        .await
        .expect("eligible");
    assert_eq!(eligible.len(), 2);
    assert_eq!(eligible[0].wallet_id, w1);
    assert_eq!(eligible[0].payable_sompi, 2_977_500_000);
    assert_eq!(eligible[1].wallet_id, w2);
    assert_eq!(eligible[1].payable_sompi, 1_985_000_000);

    let cycle = payout::create_cycle(&pool, PayoutKind::Kas, DaaScore::new(1), DaaScore::new(2))
        .await
        .expect("cycle");
    let p1 = payout::insert_payout(&pool, cycle.id, w1, 1_000_000_000)
        .await
        .expect("partial payout");
    payout::mark_payout_submitted(&pool, p1.id, BlockHash::from_bytes([11_u8; 32]))
        .await
        .expect("submit");
    payout::mark_payout_confirmed(&pool, p1.id)
        .await
        .expect("confirm");

    let after = payout::list_kas_eligible_wallets(&pool, DEFAULT_KAS_PAYOUT_THRESHOLD_SOMPI)
        .await
        .expect("eligible after partial pay");
    assert_eq!(after.len(), 2);
    let w1_row = after.iter().find(|r| r.wallet_id == w1).expect("w1");
    assert_eq!(w1_row.confirmed_paid_sompi, 1_000_000_000);
    assert_eq!(w1_row.payable_sompi, 2_977_500_000 - 1_000_000_000);

    let high_bar = payout::list_kas_eligible_wallets(&pool, 3_000_000_000)
        .await
        .expect("high threshold");
    assert!(
        high_bar.is_empty(),
        "w1 payable is 1_977_500_000 after partial confirm — below 3 KAS bar"
    );

    let mid_bar = payout::list_kas_eligible_wallets(&pool, 1_980_000_000)
        .await
        .expect("mid threshold");
    assert_eq!(mid_bar.len(), 1);
    assert_eq!(mid_bar[0].wallet_id, w2);
}

#[tokio::test]
async fn plan_kas_cycle_is_idempotent_and_sets_totals() {
    let (pool, _ctr) = fresh_pool().await;
    let (_block_id, w1, _w2) = seed_two_wallet_allocations(&pool).await;

    let params = PlanKasCycleParams {
        daa_start: DaaScore::new(1_000),
        daa_end: DaaScore::new(2_000),
        threshold_sompi: DEFAULT_KAS_PAYOUT_THRESHOLD_SOMPI,
    };

    let first = plan_kas_cycle(&pool, params).await.expect("plan");
    assert_eq!(first.cycle.idempotency_key, "kas-1000-2000");
    assert_eq!(first.cycle.total_recipients, 2);
    assert_eq!(
        first.cycle.total_sompi,
        2_977_500_000_i64 + 1_985_000_000_i64
    );
    assert_eq!(first.payouts.len(), 2);
    assert!(
        first
            .payouts
            .iter()
            .all(|p| p.status == PayoutStatus::Planned)
    );

    let second = plan_kas_cycle(&pool, params).await.expect("replay");
    assert_eq!(second.cycle.id, first.cycle.id);
    assert_eq!(second.payouts.len(), 2);

    let w1_payout = second
        .payouts
        .iter()
        .find(|p| p.wallet_id == w1)
        .expect("w1 payout");
    assert_eq!(w1_payout.amount_sompi, 2_977_500_000);
}

#[tokio::test]
async fn plan_kas_cycle_excludes_wallet_below_threshold() {
    let (pool, _ctr) = fresh_pool().await;
    let (_block_id, w1, _w2) = seed_two_wallet_allocations(&pool).await;

    let params = PlanKasCycleParams {
        daa_start: DaaScore::new(10),
        daa_end: DaaScore::new(20),
        threshold_sompi: 2_900_000_000,
    };
    let result = plan_kas_cycle(&pool, params).await.expect("plan");
    assert_eq!(result.cycle.total_recipients, 1);
    assert_eq!(result.payouts.len(), 1);
    assert_eq!(result.payouts[0].wallet_id, w1);
}

// ---- M4.4: restart-safe cycle state machine -------------------------

#[tokio::test]
async fn resume_after_crash_before_broadcast_never_double_pays() {
    let (pool, _ctr) = fresh_pool().await;
    let (_block_id, _w1, _w2) = seed_two_wallet_allocations(&pool).await;

    let params = PlanKasCycleParams {
        daa_start: DaaScore::new(5_000),
        daa_end: DaaScore::new(6_000),
        threshold_sompi: DEFAULT_KAS_PAYOUT_THRESHOLD_SOMPI,
    };

    // First pass plans the cycle and records idempotent rows BEFORE any sign.
    let planned = resume_or_plan_kas_cycle(&pool, params)
        .await
        .expect("plan pass");
    assert_eq!(planned.cycle.status, PayoutCycleStatus::Planned);
    assert_eq!(planned.payouts.len(), 2);
    assert_eq!(planned.pending().len(), 2, "all recipients are signable");
    assert_eq!(planned.derived_status(), PayoutCycleStatus::Planned);
    let original_ids: Vec<i64> = planned.payouts.iter().map(|p| p.id).collect();

    // Simulate a crash after plan, before broadcast: the process restarts and
    // re-enters through the same idempotent entry point.
    let resumed = resume_or_plan_kas_cycle(&pool, params)
        .await
        .expect("resume pass");
    assert_eq!(
        resumed.cycle.id, planned.cycle.id,
        "same cycle, not a new one"
    );
    assert_eq!(resumed.cycle.total_sompi, planned.cycle.total_sompi);
    assert_eq!(resumed.payouts.len(), 2, "no duplicate recipient rows");
    let resumed_ids: Vec<i64> = resumed.payouts.iter().map(|p| p.id).collect();
    assert_eq!(resumed_ids, original_ids, "identical payout rows on resume");
    assert_eq!(
        resumed.pending().len(),
        2,
        "nothing was paid, all still signable"
    );

    // Reconcile on an untouched cycle keeps it Planned and writes an audit row.
    let derived = reconcile_cycle_status(&pool, resumed.cycle.id)
        .await
        .expect("reconcile");
    assert_eq!(derived, PayoutCycleStatus::Planned);
    let reloaded = payout::get_cycle(&pool, resumed.cycle.id)
        .await
        .expect("reload cycle");
    assert_eq!(reloaded.status, PayoutCycleStatus::Planned);
    assert!(reloaded.broadcast_at.is_none());

    let trail = audit::list_for_subject(&pool, "payout_cycle", resumed.cycle.id, 50)
        .await
        .expect("audit trail");
    let actions: Vec<&str> = trail.iter().map(|e| e.action.as_str()).collect();
    assert!(actions.contains(&"cycle.plan"), "plan audited: {actions:?}");
    assert!(
        actions.contains(&"cycle.resume"),
        "resume audited: {actions:?}"
    );
    assert!(
        actions.contains(&"cycle.reconcile"),
        "reconcile audited: {actions:?}"
    );
}

#[tokio::test]
async fn reconcile_folds_partial_then_full_settlement() {
    let (pool, _ctr) = fresh_pool().await;
    let (_block_id, _w1, _w2) = seed_two_wallet_allocations(&pool).await;

    let params = PlanKasCycleParams {
        daa_start: DaaScore::new(7_000),
        daa_end: DaaScore::new(8_000),
        threshold_sompi: DEFAULT_KAS_PAYOUT_THRESHOLD_SOMPI,
    };
    let state = resume_or_plan_kas_cycle(&pool, params).await.expect("plan");
    let first = state.payouts[0].id;
    let second = state.payouts[1].id;

    // Confirm only the first recipient.
    payout::mark_payout_submitted(&pool, first, BlockHash::from_bytes([21_u8; 32]))
        .await
        .expect("submit first");
    payout::mark_payout_confirmed(&pool, first)
        .await
        .expect("confirm first");

    let derived = reconcile_cycle_status(&pool, state.cycle.id)
        .await
        .expect("reconcile partial");
    assert_eq!(derived, PayoutCycleStatus::PartiallySettled);
    let mid = payout::get_cycle(&pool, state.cycle.id)
        .await
        .expect("reload mid");
    assert_eq!(mid.status, PayoutCycleStatus::PartiallySettled);
    assert!(mid.broadcast_at.is_some(), "broadcast timestamp stamped");

    // The remaining recipient is still the only signable one.
    let mid_state = resume_or_plan_kas_cycle(&pool, params)
        .await
        .expect("resume mid");
    let pending: Vec<i64> = mid_state.pending().iter().map(|p| p.id).collect();
    assert_eq!(pending, vec![second]);

    // Confirm the second recipient; cycle settles.
    payout::mark_payout_submitted(&pool, second, BlockHash::from_bytes([22_u8; 32]))
        .await
        .expect("submit second");
    payout::mark_payout_confirmed(&pool, second)
        .await
        .expect("confirm second");

    let derived = reconcile_cycle_status(&pool, state.cycle.id)
        .await
        .expect("reconcile full");
    assert_eq!(derived, PayoutCycleStatus::Settled);
    let settled = payout::get_cycle(&pool, state.cycle.id)
        .await
        .expect("reload settled");
    assert_eq!(settled.status, PayoutCycleStatus::Settled);
    assert!(settled.settled_at.is_some());

    let final_state = resume_or_plan_kas_cycle(&pool, params)
        .await
        .expect("resume settled");
    assert!(final_state.pending().is_empty(), "nothing left to sign");
}
