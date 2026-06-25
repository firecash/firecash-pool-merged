//! Tests for `accountant::MaturityTracker` against an in-memory
//! `KaspadClient` fake.
//!
//! The tracker drives two independent concerns each sweep:
//!   * block-lifecycle telemetry — resolve `submitted_to_node` blocks to
//!     `confirmed_blue` / `orphaned` by GHOSTDAG colour; and
//!   * coinbase-reward allocation — record matured coinbase UTXOs and
//!     hand them to the allocation engine.
//!
//! The kaspad client is a trait, so we drive the tracker by manipulating
//! a `FakeKaspad` from the outside — entirely deterministic, no network.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_arithmetic,
    clippy::similar_names,
    clippy::cast_possible_wrap,
    // Test-only helper constants and synthetic byte casts; the
    // values are bounded to 0..5 so the u8 cast is exact.
    clippy::missing_const_for_fn,
    clippy::cast_possible_truncation
)]

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use katpool_db::repo::block::{self, BlockStatus};
use katpool_db::repo::{coinbase_reward, share, wallet, worker};
use katpool_domain::{
    BlockHash, CorrelationId, DaaScore, ShareDifficulty, WalletAddress, WorkerName,
};
use tokio::sync::{Mutex, watch};

use accountant::{
    AllocationEngine, BlockColor, CoinbaseUtxo, FeeConfig, KaspadClient, KaspadError,
    MaturityConfig, MaturityTracker, StaticTierClassifier,
};

mod common;
use common::{HASH_A, HASH_B, MINER_A, setup};

const NETWORK: &str = "mainnet";

/// In-memory `KaspadClient` whose state is driven from tests via
/// the `set_*` mutators.
#[derive(Debug, Default)]
struct FakeKaspad {
    state: Mutex<FakeState>,
}

#[derive(Debug, Default)]
struct FakeState {
    virtual_daa_score: u64,
    /// Hashes kaspad knows the colour of. A missing hash models
    /// `MergerNotFound` → `BlockColor::NotYetMerged`.
    colors: HashMap<BlockHash, BlockColor>,
    /// Coinbase UTXOs credited to the pool address.
    utxos: Vec<CoinbaseUtxo>,
    fail_next_color: bool,
    fail_next_virtual: bool,
}

impl FakeKaspad {
    fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    async fn set_virtual_daa_score(&self, v: u64) {
        self.state.lock().await.virtual_daa_score = v;
    }

    async fn set_color(&self, hash: BlockHash, color: BlockColor) {
        self.state.lock().await.colors.insert(hash, color);
    }

    async fn add_utxo(&self, utxo: CoinbaseUtxo) {
        self.state.lock().await.utxos.push(utxo);
    }
}

#[async_trait]
impl KaspadClient for FakeKaspad {
    async fn get_virtual_daa_score(&self) -> Result<u64, KaspadError> {
        let mut s = self.state.lock().await;
        if s.fail_next_virtual {
            s.fail_next_virtual = false;
            return Err(KaspadError::Transport("test-injected".to_owned()));
        }
        Ok(s.virtual_daa_score)
    }

    async fn get_block_color(&self, hash: BlockHash) -> Result<BlockColor, KaspadError> {
        let mut s = self.state.lock().await;
        if s.fail_next_color {
            s.fail_next_color = false;
            return Err(KaspadError::Transport("test-injected".to_owned()));
        }
        Ok(s.colors
            .get(&hash)
            .copied()
            .unwrap_or(BlockColor::NotYetMerged))
    }

    async fn get_pool_coinbase_utxos(&self) -> Result<Vec<CoinbaseUtxo>, KaspadError> {
        Ok(self.state.lock().await.utxos.clone())
    }
}

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

/// Insert a block in `submitted_to_node` state (mirrors the
/// consumer's post-BlockAccepted state).
async fn insert_submitted(
    db: &sqlx::PgPool,
    hash: BlockHash,
    finder_w: katpool_db::repo::WalletId,
    finder_wk: katpool_db::repo::WorkerId,
    daa: u64,
) {
    let _ = block::ensure(
        db,
        hash,
        finder_w,
        finder_wk,
        DaaScore::new(daa),
        0,
        CorrelationId::new_v4(),
    )
    .await
    .unwrap();
    block::mark_submitted(db, hash).await.unwrap();
}

fn build_tracker(
    db: sqlx::PgPool,
    kaspad: Arc<FakeKaspad>,
    cfg: MaturityConfig,
) -> MaturityTracker {
    let fee = FeeConfig::new(75).unwrap();
    let engine = Arc::new(AllocationEngine::new(
        db.clone(),
        fee,
        Arc::new(StaticTierClassifier::standard()),
        "test".to_owned(),
    ));
    MaturityTracker::new(db, kaspad as _, engine, cfg, "test".to_owned())
}

fn default_cfg() -> MaturityConfig {
    MaturityConfig {
        poll_interval: Duration::from_millis(50),
        coinbase_maturity: 1000,
        window_daa_span: 600,
        batch_size: 200,
        coinbase_min_daa_score: 0,
    }
}

// ---------- block-lifecycle telemetry ------------------------------

#[tokio::test]
async fn submitted_to_node_transitions_to_confirmed_blue_when_blue() {
    let env = setup().await;
    let (w, wk) = ensure_wallet_worker(&env.db, MINER_A, "rig-01").await;
    let h = BlockHash::from_hex(HASH_A).unwrap();
    insert_submitted(&env.db, h, w, wk, 1_000_000).await;

    let kaspad = FakeKaspad::new();
    kaspad.set_virtual_daa_score(1_000_500).await;
    kaspad.set_color(h, BlockColor::Blue).await;

    let tracker = build_tracker(env.db.clone(), kaspad, default_cfg());
    let stats = tracker.run_once().await.unwrap();
    assert_eq!(stats.confirmed_blue, 1);
    assert_eq!(stats.orphaned, 0);
    assert_eq!(stats.blocks_waiting, 0);

    let blk = block::find_by_hash(&env.db, h).await.unwrap().unwrap();
    assert_eq!(blk.status, BlockStatus::ConfirmedBlue);
}

#[tokio::test]
async fn submitted_to_node_stays_when_not_yet_merged_within_depth() {
    let env = setup().await;
    let (w, wk) = ensure_wallet_worker(&env.db, MINER_A, "rig-01").await;
    let h = BlockHash::from_hex(HASH_A).unwrap();
    insert_submitted(&env.db, h, w, wk, 1_000_000).await;

    let kaspad = FakeKaspad::new();
    // Within coinbase_maturity (1000) of the block's daa → still waiting.
    kaspad.set_virtual_daa_score(1_000_500).await;
    // No colour set → NotYetMerged.

    let tracker = build_tracker(env.db.clone(), kaspad, default_cfg());
    let stats = tracker.run_once().await.unwrap();
    assert_eq!(stats.blocks_waiting, 1);
    let blk = block::find_by_hash(&env.db, h).await.unwrap().unwrap();
    assert_eq!(blk.status, BlockStatus::SubmittedToNode);
}

#[tokio::test]
async fn submitted_to_node_orphans_when_merged_red() {
    let env = setup().await;
    let (w, wk) = ensure_wallet_worker(&env.db, MINER_A, "rig-01").await;
    let h = BlockHash::from_hex(HASH_A).unwrap();
    insert_submitted(&env.db, h, w, wk, 1_000_000).await;

    let kaspad = FakeKaspad::new();
    kaspad.set_virtual_daa_score(1_000_500).await;
    kaspad.set_color(h, BlockColor::Red).await;

    let tracker = build_tracker(env.db.clone(), kaspad, default_cfg());
    let stats = tracker.run_once().await.unwrap();
    assert_eq!(stats.orphaned, 1);
    let blk = block::find_by_hash(&env.db, h).await.unwrap().unwrap();
    assert_eq!(blk.status, BlockStatus::Orphaned);
}

#[tokio::test]
async fn submitted_to_node_ages_out_to_orphan_past_maturity_depth() {
    let env = setup().await;
    let (w, wk) = ensure_wallet_worker(&env.db, MINER_A, "rig-01").await;
    let h = BlockHash::from_hex(HASH_A).unwrap();
    insert_submitted(&env.db, h, w, wk, 1_000_000).await;

    let kaspad = FakeKaspad::new();
    // Beyond coinbase_maturity (1000) and still NotYetMerged → orphan.
    kaspad.set_virtual_daa_score(1_001_001).await;
    // No colour set → NotYetMerged.

    let tracker = build_tracker(env.db.clone(), kaspad, default_cfg());
    let stats = tracker.run_once().await.unwrap();
    assert_eq!(stats.orphaned, 1);
    assert_eq!(stats.blocks_waiting, 0);
    let blk = block::find_by_hash(&env.db, h).await.unwrap().unwrap();
    assert_eq!(blk.status, BlockStatus::Orphaned);
}

// ---------- coinbase-reward allocation -----------------------------

#[tokio::test]
async fn matured_coinbase_utxo_is_recorded_and_allocated() {
    let env = setup().await;
    let (w, wk) = ensure_wallet_worker(&env.db, MINER_A, "rig-01").await;
    // Seed shares inside the PROP window ending at the UTXO's daa.
    for i in 0..5 {
        seed_share(&env.db, w, wk, 1024.0, 1_000_000 + i).await;
    }

    let kaspad = FakeKaspad::new();
    // virtual_daa - utxo_daa = 1000 == coinbase_maturity → mature.
    kaspad.set_virtual_daa_score(1_001_010).await;
    kaspad
        .add_utxo(CoinbaseUtxo {
            transaction_id: [0xa1; 32],
            index: 0,
            amount_sompi: 500_000_000,
            block_daa_score: 1_000_010,
        })
        .await;

    let tracker = build_tracker(env.db.clone(), kaspad, default_cfg());
    let stats = tracker.run_once().await.unwrap();
    assert_eq!(stats.rewards_discovered, 1);
    assert_eq!(stats.rewards_allocated, 1);

    let reward = coinbase_reward::find_by_outpoint(&env.db, &[0xa1; 32], 0)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(reward.amount_sompi, 500_000_000);
    assert!(reward.allocated_at.is_some());

    let n: i64 = sqlx::query_scalar(
        "SELECT count(*)::bigint FROM share_allocation WHERE coinbase_reward_id = $1",
    )
    .bind(reward.id.0)
    .fetch_one(&env.db)
    .await
    .unwrap();
    assert_eq!(n, 1, "exactly one wallet contributed → one allocation");
}

#[tokio::test]
async fn coinbase_utxo_below_cutover_daa_floor_is_skipped() {
    // A matured coinbase whose block predates the cutover floor must be
    // ignored entirely — never recorded, never allocated — even with
    // contributing shares present (it was paid by the prior pool).
    let env = setup().await;
    let (w, wk) = ensure_wallet_worker(&env.db, MINER_A, "rig-01").await;
    for i in 0..5 {
        seed_share(&env.db, w, wk, 1024.0, 1_000_000 + i).await;
    }

    let kaspad = FakeKaspad::new();
    kaspad.set_virtual_daa_score(1_001_010).await;
    kaspad
        .add_utxo(CoinbaseUtxo {
            transaction_id: [0xb2; 32],
            index: 0,
            amount_sompi: 500_000_000,
            // Matured (depth 1000) but below the floor below.
            block_daa_score: 1_000_010,
        })
        .await;

    let cfg = MaturityConfig {
        coinbase_min_daa_score: 1_000_011, // floor just above the UTXO's daa
        ..default_cfg()
    };
    let tracker = build_tracker(env.db.clone(), kaspad, cfg);
    let stats = tracker.run_once().await.unwrap();
    assert_eq!(stats.rewards_discovered, 0);
    assert_eq!(stats.rewards_allocated, 0);
    assert_eq!(stats.rewards_skipped_below_floor, 1);

    assert!(
        coinbase_reward::find_by_outpoint(&env.db, &[0xb2; 32], 0)
            .await
            .unwrap()
            .is_none(),
        "a below-floor coinbase must not be recorded"
    );
}

#[tokio::test]
async fn immature_coinbase_utxo_is_not_recorded() {
    let env = setup().await;
    let kaspad = FakeKaspad::new();
    // depth = 999 < coinbase_maturity (1000) → immature.
    kaspad.set_virtual_daa_score(1_001_009).await;
    kaspad
        .add_utxo(CoinbaseUtxo {
            transaction_id: [0xa1; 32],
            index: 0,
            amount_sompi: 500_000_000,
            block_daa_score: 1_000_010,
        })
        .await;

    let tracker = build_tracker(env.db.clone(), kaspad, default_cfg());
    let stats = tracker.run_once().await.unwrap();
    assert_eq!(stats.rewards_discovered, 0);
    assert_eq!(stats.rewards_allocated, 0);
    assert!(
        coinbase_reward::find_by_outpoint(&env.db, &[0xa1; 32], 0)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn reward_discovery_and_allocation_are_idempotent() {
    let env = setup().await;
    let (w, wk) = ensure_wallet_worker(&env.db, MINER_A, "rig-01").await;
    seed_share(&env.db, w, wk, 1024.0, 1_000_000).await;

    let kaspad = FakeKaspad::new();
    kaspad.set_virtual_daa_score(1_001_010).await;
    kaspad
        .add_utxo(CoinbaseUtxo {
            transaction_id: [0xa1; 32],
            index: 0,
            amount_sompi: 500_000_000,
            block_daa_score: 1_000_010,
        })
        .await;
    let tracker = build_tracker(env.db.clone(), kaspad, default_cfg());

    let first = tracker.run_once().await.unwrap();
    assert_eq!(first.rewards_discovered, 1);
    assert_eq!(first.rewards_allocated, 1);

    // Second sweep: UTXO still present, but already recorded + allocated.
    let second = tracker.run_once().await.unwrap();
    assert_eq!(second.rewards_discovered, 0);
    assert_eq!(second.rewards_allocated, 0);

    let count: i64 = sqlx::query_scalar("SELECT count(*)::bigint FROM share_allocation")
        .fetch_one(&env.db)
        .await
        .unwrap();
    assert_eq!(count, 1, "allocation must not be duplicated across sweeps");
}

#[tokio::test]
async fn matured_utxo_with_no_shares_is_finalised_empty() {
    let env = setup().await;
    // No shares seeded → empty window → reward retained by pool.
    let kaspad = FakeKaspad::new();
    kaspad.set_virtual_daa_score(1_001_010).await;
    kaspad
        .add_utxo(CoinbaseUtxo {
            transaction_id: [0xa1; 32],
            index: 0,
            amount_sompi: 500_000_000,
            block_daa_score: 1_000_010,
        })
        .await;

    let tracker = build_tracker(env.db.clone(), kaspad, default_cfg());
    let stats = tracker.run_once().await.unwrap();
    assert_eq!(stats.rewards_discovered, 1);
    assert_eq!(stats.rewards_empty, 1);
    assert_eq!(stats.rewards_allocated, 0);

    let reward = coinbase_reward::find_by_outpoint(&env.db, &[0xa1; 32], 0)
        .await
        .unwrap()
        .unwrap();
    assert!(reward.allocated_at.is_some());
}

// ---------- error isolation ----------------------------------------

#[tokio::test]
async fn whole_sweep_fails_when_virtual_daa_query_errors() {
    let env = setup().await;
    let kaspad = FakeKaspad::new();
    {
        let mut s = kaspad.state.lock().await;
        s.fail_next_virtual = true;
    }
    let tracker = build_tracker(env.db.clone(), kaspad, default_cfg());
    let err = tracker
        .run_once()
        .await
        .expect_err("transport error → sweep failure");
    let msg = format!("{err}");
    assert!(msg.contains("kaspad") || msg.contains("transport"), "{msg}");
}

#[tokio::test]
async fn per_block_color_error_is_isolated() {
    let env = setup().await;
    let (w, wk) = ensure_wallet_worker(&env.db, MINER_A, "rig-01").await;
    let h_a = BlockHash::from_hex(HASH_A).unwrap();
    let h_b = BlockHash::from_hex(HASH_B).unwrap();
    insert_submitted(&env.db, h_a, w, wk, 1_000_000).await;
    insert_submitted(&env.db, h_b, w, wk, 1_000_010).await;

    let kaspad = FakeKaspad::new();
    kaspad.set_virtual_daa_score(1_000_500).await;
    kaspad.set_color(h_b, BlockColor::Blue).await;
    // Inject a one-shot error on the first colour query.
    {
        let mut s = kaspad.state.lock().await;
        s.fail_next_color = true;
    }
    let tracker = build_tracker(env.db.clone(), kaspad, default_cfg());
    let stats = tracker.run_once().await.unwrap();
    assert_eq!(stats.errors, 1, "first block sees the injected error");
    // The other block proceeded normally.
    assert!(stats.confirmed_blue + stats.blocks_waiting >= 1);
}

// ---------- batch limit --------------------------------------------

#[tokio::test]
async fn batch_size_limits_blocks_processed_per_sweep() {
    let env = setup().await;
    let (w, wk) = ensure_wallet_worker(&env.db, MINER_A, "rig-01").await;
    let kaspad = FakeKaspad::new();
    kaspad.set_virtual_daa_score(1_000_500).await;

    // Seed 5 submitted_to_node blocks. None known to kaspad → waiting.
    for i in 0..5u64 {
        let mut bytes = [0u8; 32];
        bytes[31] = i as u8 + 1;
        let h = BlockHash::from_bytes(bytes);
        insert_submitted(&env.db, h, w, wk, 1_000_000 + i).await;
    }

    let cfg = MaturityConfig {
        batch_size: 2,
        ..default_cfg()
    };
    let tracker = build_tracker(env.db.clone(), kaspad, cfg);
    let stats = tracker.run_once().await.unwrap();
    assert_eq!(
        stats.blocks_waiting + stats.confirmed_blue + stats.orphaned + stats.errors,
        2,
        "exactly batch_size blocks processed per sweep"
    );
}

// ---------- run_loop shutdown --------------------------------------

#[tokio::test]
async fn run_loop_exits_cleanly_on_shutdown_signal() {
    let env = setup().await;
    let kaspad = FakeKaspad::new();
    kaspad.set_virtual_daa_score(0).await;

    let cfg = MaturityConfig {
        poll_interval: Duration::from_millis(50),
        ..default_cfg()
    };
    let tracker = build_tracker(env.db.clone(), kaspad, cfg);

    let (tx, rx) = watch::channel(false);
    let handle = tokio::spawn(tracker.run_loop(rx));

    // Let at least one tick elapse.
    tokio::time::sleep(Duration::from_millis(150)).await;
    tx.send(true).unwrap();

    let result = tokio::time::timeout(Duration::from_secs(1), handle).await;
    assert!(result.is_ok(), "loop didn't shut down within 1s");
    assert!(result.unwrap().unwrap().is_ok(), "loop returned an error");
}
