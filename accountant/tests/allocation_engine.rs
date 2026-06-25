//! Integration tests for `AllocationEngine` against ephemeral
//! Postgres.
//!
//! Allocation is anchored on a matured `coinbase_reward` row (the unit
//! of realised pool income), not on a found block. The block table is
//! pure lifecycle telemetry and plays no part here — see ADR-0014.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_arithmetic,
    clippy::similar_names,
    clippy::cast_possible_wrap,
    // Test-local stub types are defined inside test functions for
    // colocation with the assertion they support.
    clippy::items_after_statements
)]

use std::sync::Arc;

use chrono::Utc;
use katpool_db::repo::share_allocation::DbWalletTier;
use katpool_db::repo::{CoinbaseRewardId, coinbase_reward, nacho_rebate, share, wallet, worker};
use katpool_domain::{CorrelationId, DaaScore, ShareDifficulty, WalletAddress, WorkerName};

use accountant::{
    AllocationEngine, AllocationOutcome, FeeConfig, StaticTierClassifier, TierClassifier,
    WalletTier,
};

mod common;
use common::{MINER_A, MINER_B, setup};

const NETWORK: &str = "mainnet";

/// Distinct coinbase-outpoint transaction ids for the fixtures.
const TXID_A: [u8; 32] = [0xa1; 32];
const TXID_B: [u8; 32] = [0xb2; 32];

async fn ensure_wallet_worker(
    db: &sqlx::PgPool,
    wallet_str: &str,
    worker_str: &str,
) -> (katpool_db::repo::WalletId, katpool_db::repo::WorkerId) {
    let mut tx = db.begin().await.unwrap();
    let w = wallet::ensure(
        &mut *tx,
        &WalletAddress::new(wallet_str.to_owned()).unwrap(),
        NETWORK,
    )
    .await
    .unwrap();
    let wk = worker::ensure(
        &mut *tx,
        w.id,
        &WorkerName::new(worker_str.to_owned()).unwrap(),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    (w.id, wk.id)
}

async fn seed_share(
    db: &sqlx::PgPool,
    w_id: katpool_db::repo::WalletId,
    wk_id: katpool_db::repo::WorkerId,
    difficulty: f64,
    daa: u64,
) {
    share::insert_credited(
        db,
        w_id,
        wk_id,
        None,
        ShareDifficulty::new(difficulty).unwrap(),
        DaaScore::new(daa),
        CorrelationId::new_v4(),
    )
    .await
    .unwrap();
}

/// Record a matured coinbase UTXO and return its anchor id. This is the
/// unit the tracker hands to the engine.
async fn insert_reward(
    db: &sqlx::PgPool,
    txid: &[u8; 32],
    amount_sompi: i64,
    block_daa_score: u64,
) -> CoinbaseRewardId {
    let (id, _) = coinbase_reward::ensure(db, txid, 0, amount_sompi, block_daa_score)
        .await
        .unwrap();
    id
}

fn engine_with(
    db: sqlx::PgPool,
    topline_bps: u16,
    classifier: Arc<dyn TierClassifier>,
) -> AllocationEngine {
    let fee = FeeConfig::new(topline_bps).unwrap();
    AllocationEngine::new(db, fee, classifier, "test".to_owned())
}

// ---------- happy-path / sum invariants -----------------------------

#[tokio::test]
async fn happy_path_two_wallets_standard_tier() {
    let env = setup().await;
    let (w_a, wk_a) = ensure_wallet_worker(&env.db, MINER_A, "rig-01").await;
    let (w_b, wk_b) = ensure_wallet_worker(&env.db, MINER_B, "rig-02").await;

    // 60/40 weight split.
    seed_share(&env.db, w_a, wk_a, 600.0, 1_000_000).await;
    seed_share(&env.db, w_b, wk_b, 400.0, 1_000_001).await;

    let reward_id = insert_reward(&env.db, &TXID_A, 1_000_000_000, 1_000_010).await;

    let engine = engine_with(
        env.db.clone(),
        75,
        Arc::new(StaticTierClassifier::standard()),
    );
    let outcome = engine
        .allocate_coinbase_reward(
            reward_id,
            1_000_000_000,
            DaaScore::new(1_000_000),
            DaaScore::new(1_000_020),
        )
        .await
        .expect("allocate");

    let AllocationOutcome::Allocated {
        wallet_count,
        total_pool_fee_sompi,
        total_nacho_accrual_sompi,
        total_net_payout_sompi,
        rounding_residue_sompi,
    } = outcome
    else {
        panic!("expected Allocated, got {outcome:?}");
    };
    assert_eq!(wallet_count, 2);
    // Sum invariant: every sompi accounted for.
    assert_eq!(
        total_pool_fee_sompi + total_nacho_accrual_sompi + total_net_payout_sompi,
        1_000_000_000,
        "sum invariant must hold across all wallets"
    );
    // Residue is bounded by N-1 = 1.
    assert!(
        rounding_residue_sompi <= 1,
        "residue = {rounding_residue_sompi}"
    );
}

#[tokio::test]
async fn mixed_tier_elite_dominates_in_nacho_rebate() {
    let env = setup().await;
    let (w_a, wk_a) = ensure_wallet_worker(&env.db, MINER_A, "rig-01").await;
    let (w_b, wk_b) = ensure_wallet_worker(&env.db, MINER_B, "rig-02").await;
    seed_share(&env.db, w_a, wk_a, 500.0, 1_000_000).await;
    seed_share(&env.db, w_b, wk_b, 500.0, 1_000_001).await;

    let reward_id = insert_reward(&env.db, &TXID_A, 1_000_000_000, 1_000_010).await;

    // Custom classifier: wallet A elite, wallet B standard.
    struct PerWalletStub;
    #[async_trait::async_trait]
    impl TierClassifier for PerWalletStub {
        async fn classify(
            &self,
            w: &WalletAddress,
        ) -> Result<WalletTier, accountant::ClassifierError> {
            Ok(if w.as_str() == MINER_A {
                WalletTier::Elite
            } else {
                WalletTier::Standard
            })
        }
    }

    let engine = engine_with(env.db.clone(), 75, Arc::new(PerWalletStub));
    let _ = engine
        .allocate_coinbase_reward(
            reward_id,
            1_000_000_000,
            DaaScore::new(1_000_000),
            DaaScore::new(1_000_020),
        )
        .await
        .unwrap();

    // Pull allocations for the reward.
    let allocs = katpool_db::repo::share_allocation::list_for_reward(&env.db, reward_id)
        .await
        .unwrap();
    assert_eq!(allocs.len(), 2);

    let a_alloc = allocs.iter().find(|a| a.wallet_id == w_a).unwrap();
    let b_alloc = allocs.iter().find(|a| a.wallet_id == w_b).unwrap();
    assert_eq!(a_alloc.applied_tier, DbWalletTier::Elite);
    assert_eq!(b_alloc.applied_tier, DbWalletTier::Standard);
    // Elite has higher NACHO accrual at identical gross.
    // Note: A gets the residue (smallest wallet_id) so its gross
    // may be 1 sompi higher than B's. The NACHO comparison is
    // dominated by the rebate ratio (10000 vs 3300), not the
    // residue, so the assertion still holds with strict >.
    assert!(
        a_alloc.nacho_accrual_sompi > b_alloc.nacho_accrual_sompi,
        "elite ({}) should accrue more NACHO than standard ({})",
        a_alloc.nacho_accrual_sompi,
        b_alloc.nacho_accrual_sompi
    );
    // Elite has zero pool_fee (100% rebate).
    assert_eq!(a_alloc.pool_fee_sompi, 0);
    // Standard has non-zero pool_fee (67% of fee_share retained).
    assert!(b_alloc.pool_fee_sompi > 0);
}

#[tokio::test]
async fn allocate_marks_reward_allocated() {
    let env = setup().await;
    let (w_a, wk_a) = ensure_wallet_worker(&env.db, MINER_A, "rig-01").await;
    seed_share(&env.db, w_a, wk_a, 1024.0, 1_000_000).await;

    let reward_id = insert_reward(&env.db, &TXID_A, 500_000_000, 1_000_010).await;

    let engine = engine_with(
        env.db.clone(),
        75,
        Arc::new(StaticTierClassifier::standard()),
    );
    let _ = engine
        .allocate_coinbase_reward(
            reward_id,
            500_000_000,
            DaaScore::new(1_000_000),
            DaaScore::new(1_000_020),
        )
        .await
        .unwrap();

    let reward = coinbase_reward::find_by_outpoint(&env.db, &TXID_A, 0)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(reward.amount_sompi, 500_000_000);
    assert!(
        reward.allocated_at.is_some(),
        "reward must be stamped allocated"
    );
}

#[tokio::test]
async fn nacho_rebate_accrues_additively() {
    let env = setup().await;
    let (w_a, wk_a) = ensure_wallet_worker(&env.db, MINER_A, "rig-01").await;
    seed_share(&env.db, w_a, wk_a, 1024.0, 1_000_000).await;

    let reward_a = insert_reward(&env.db, &TXID_A, 1_000_000_000, 1_000_010).await;

    // First allocation.
    let engine = engine_with(
        env.db.clone(),
        75,
        Arc::new(StaticTierClassifier::new(WalletTier::Elite)),
    );
    let _ = engine
        .allocate_coinbase_reward(
            reward_a,
            1_000_000_000,
            DaaScore::new(1_000_000),
            DaaScore::new(1_000_020),
        )
        .await
        .unwrap();
    let r1 = nacho_rebate::get(&env.db, w_a).await.unwrap().unwrap();
    // Elite rebate at 75bps of 1B = 7_500_000.
    assert_eq!(r1.accrued_sompi, 7_500_000);

    // Second reward, second allocation — must ADD, not replace.
    seed_share(&env.db, w_a, wk_a, 2048.0, 1_000_021).await;
    let reward_b = insert_reward(&env.db, &TXID_B, 1_000_000_000, 1_000_030).await;
    let _ = engine
        .allocate_coinbase_reward(
            reward_b,
            1_000_000_000,
            DaaScore::new(1_000_020),
            DaaScore::new(1_000_040),
        )
        .await
        .unwrap();
    let r2 = nacho_rebate::get(&env.db, w_a).await.unwrap().unwrap();
    assert_eq!(
        r2.accrued_sompi, 15_000_000,
        "second reward must accrue additively"
    );
}

// ---------- idempotency ---------------------------------------------

#[tokio::test]
async fn replaying_allocate_is_noop() {
    let env = setup().await;
    let (w_a, wk_a) = ensure_wallet_worker(&env.db, MINER_A, "rig-01").await;
    seed_share(&env.db, w_a, wk_a, 1024.0, 1_000_000).await;
    let reward_id = insert_reward(&env.db, &TXID_A, 500_000_000, 1_000_010).await;

    let engine = engine_with(
        env.db.clone(),
        75,
        Arc::new(StaticTierClassifier::standard()),
    );
    let _ = engine
        .allocate_coinbase_reward(
            reward_id,
            500_000_000,
            DaaScore::new(1_000_000),
            DaaScore::new(1_000_020),
        )
        .await
        .unwrap();
    let r_first = nacho_rebate::get(&env.db, w_a).await.unwrap();

    // Second call: idempotent no-op (reward already allocated).
    let outcome2 = engine
        .allocate_coinbase_reward(
            reward_id,
            500_000_000,
            DaaScore::new(1_000_000),
            DaaScore::new(1_000_020),
        )
        .await
        .unwrap();
    assert_eq!(outcome2, AllocationOutcome::AlreadyAllocated);

    // Rebate hasn't moved.
    let r_second = nacho_rebate::get(&env.db, w_a).await.unwrap();
    assert_eq!(
        r_first.map(|r| r.accrued_sompi),
        r_second.map(|r| r.accrued_sompi),
    );

    // Allocation row count unchanged.
    let count: i64 = sqlx::query_scalar("SELECT count(*)::bigint FROM share_allocation")
        .fetch_one(&env.db)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

// ---------- edge cases ----------------------------------------------

#[tokio::test]
async fn empty_window_no_allocations() {
    let env = setup().await;
    let reward_id = insert_reward(&env.db, &TXID_A, 500_000_000, 1_000_010).await;

    // No shares seeded.

    let engine = engine_with(
        env.db.clone(),
        75,
        Arc::new(StaticTierClassifier::standard()),
    );
    let outcome = engine
        .allocate_coinbase_reward(
            reward_id,
            500_000_000,
            DaaScore::new(1_000_000),
            DaaScore::new(1_000_020),
        )
        .await
        .unwrap();
    assert_eq!(
        outcome,
        AllocationOutcome::NoContributingWallets {
            retained_reward_sompi: 500_000_000
        }
    );

    let reward = coinbase_reward::find_by_outpoint(&env.db, &TXID_A, 0)
        .await
        .unwrap()
        .unwrap();
    assert!(
        reward.allocated_at.is_some(),
        "empty-window reward must still be stamped allocated"
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*)::bigint FROM share_allocation")
        .fetch_one(&env.db)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn rejects_unknown_reward_id() {
    let env = setup().await;
    let engine = engine_with(
        env.db.clone(),
        75,
        Arc::new(StaticTierClassifier::standard()),
    );
    let err = engine
        .allocate_coinbase_reward(
            CoinbaseRewardId(999_999),
            1_000_000_000,
            DaaScore::new(1_000_000),
            DaaScore::new(1_000_020),
        )
        .await
        .expect_err("unknown reward id should error");
    let msg = format!("{err}");
    assert!(msg.contains("unknown coinbase reward"), "{msg}");
}

#[tokio::test]
async fn rejects_negative_reward() {
    let env = setup().await;
    let reward_id = insert_reward(&env.db, &TXID_A, 0, 1_000_010).await;
    let engine = engine_with(
        env.db.clone(),
        75,
        Arc::new(StaticTierClassifier::standard()),
    );
    let err = engine
        .allocate_coinbase_reward(reward_id, -1, DaaScore::new(0), DaaScore::new(2_000_000))
        .await
        .expect_err("negative reward should error");
    let _ = Utc::now();
    let _ = err;
}

#[tokio::test]
async fn audit_log_records_allocation_event() {
    let env = setup().await;
    let (w_a, wk_a) = ensure_wallet_worker(&env.db, MINER_A, "rig-01").await;
    seed_share(&env.db, w_a, wk_a, 1024.0, 1_000_000).await;
    let reward_id = insert_reward(&env.db, &TXID_A, 500_000_000, 1_000_010).await;

    let engine = engine_with(
        env.db.clone(),
        75,
        Arc::new(StaticTierClassifier::standard()),
    );
    let _ = engine
        .allocate_coinbase_reward(
            reward_id,
            500_000_000,
            DaaScore::new(1_000_000),
            DaaScore::new(1_000_020),
        )
        .await
        .unwrap();

    let entries =
        katpool_db::repo::audit::list_for_subject(&env.db, "coinbase_reward", reward_id.0, 10)
            .await
            .unwrap();
    let allocation_entry = entries
        .iter()
        .find(|e| e.action == "coinbase_reward.allocated")
        .expect("audit entry for coinbase_reward.allocated must exist");
    assert_eq!(allocation_entry.actor, "test");
    let payload = &allocation_entry.payload;
    assert_eq!(payload["reward_sompi"], 500_000_000);
    assert_eq!(payload["wallet_count"], 1);
    assert_eq!(payload["applied_topline_bps"], 75);
}
