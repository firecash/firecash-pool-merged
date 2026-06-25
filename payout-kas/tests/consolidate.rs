//! Consolidation-engine tests: drive ticks against an in-memory mock kaspad
//! and a real testcontainer Postgres.
//!
//! These pin the engine contract deterministically: a snapshot (with the
//! spendable UTXO count) is recorded every tick; hysteresis starts a sweep only
//! above the trigger and compounds down to the target floor; dry-run plans +
//! signs but never broadcasts; live broadcasts every planned batch and writes an
//! audit entry; and a tick skips entirely when another treasury spender holds
//! the shared advisory lock.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use kaspa_addresses::{Address, Prefix, Version};
use kaspa_consensus_core::tx::{
    ScriptPublicKey, Transaction, TransactionId, TransactionOutpoint, UtxoEntry,
};
use kaspa_txscript::pay_to_address_script;
use katpool_db::repo::{audit, treasury};
use katpool_db::{PoolConfig, build_pool, migrate};
use katpool_idempotency::{AdvisoryLock, advisory_key};
use katpool_secrets::{TreasurySecret, from_hex};
use payout_kas::{
    ConsolidationEngine, ConsolidationEngineConfig, ConsolidationTickOutcome, ExecutionMode,
    KaspadClient, KaspadError, TREASURY_SPEND_LOCK_NAMESPACE, TreasuryUtxoSnapshot,
};
use secp256k1::Keypair;
use testcontainers::ContainerAsync;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;

const TREASURY_HEX: &str = "1111111111111111111111111111111111111111111111111111111111111111";

// ---- harness --------------------------------------------------------

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
        application_name: "payout-kas-consolidate-test".to_owned(),
    };
    let pool = build_pool(&cfg).await.expect("pool");
    migrate::run(&pool).await.expect("migrate");
    (pool, container)
}

fn treasury() -> (TreasurySecret, Address) {
    let secret = from_hex(TREASURY_HEX).expect("valid key");
    let kp = Keypair::from_seckey_slice(secp256k1::SECP256K1, secret.expose_secret()).expect("kp");
    let addr = Address::new(
        Prefix::Mainnet,
        Version::PubKey,
        &kp.x_only_public_key().0.serialize(),
    );
    (secret, addr)
}

fn spendable(index: u32, amount: u64, script: &ScriptPublicKey) -> TreasuryUtxoSnapshot {
    TreasuryUtxoSnapshot {
        outpoint: TransactionOutpoint {
            transaction_id: TransactionId::from_bytes([7_u8; 32]),
            index,
        },
        entry: UtxoEntry {
            amount,
            script_public_key: script.clone(),
            block_daa_score: 0,
            is_coinbase: false,
            covenant_id: None,
        },
    }
}

/// `count` fragmented ~3.9-KAS treasury coins with distinct outpoints.
fn fragmented(count: u32, script: &ScriptPublicKey) -> Vec<TreasuryUtxoSnapshot> {
    (0..count)
        .map(|i| spendable(i, 390_000_000 + u64::from(i), script))
        .collect()
}

// ---- mock kaspad ----------------------------------------------------

// Shared interior state so a test can mutate the UTXO set across ticks after
// the mock has been moved into the engine (clone shares the same `Arc`s).
#[derive(Default, Clone)]
struct MockKaspad {
    virtual_daa: Arc<Mutex<u64>>,
    utxos: Arc<Mutex<Vec<TreasuryUtxoSnapshot>>>,
    mempool: Arc<Mutex<HashSet<[u8; 32]>>>,
}

impl MockKaspad {
    fn set_virtual_daa(&self, daa: u64) {
        *self.virtual_daa.lock().unwrap() = daa;
    }
    fn set_utxos(&self, utxos: Vec<TreasuryUtxoSnapshot>) {
        *self.utxos.lock().unwrap() = utxos;
    }
}

#[async_trait]
impl KaspadClient for MockKaspad {
    async fn virtual_daa_score(&self) -> Result<u64, KaspadError> {
        Ok(*self.virtual_daa.lock().unwrap())
    }
    async fn treasury_utxos(&self, _: &Address) -> Result<Vec<TreasuryUtxoSnapshot>, KaspadError> {
        Ok(self.utxos.lock().unwrap().clone())
    }
    async fn submit_transaction(
        &self,
        tx: &Transaction,
        _allow_orphan: bool,
    ) -> Result<TransactionId, KaspadError> {
        let id = tx.id();
        self.mempool.lock().unwrap().insert(id.as_bytes());
        Ok(id)
    }
    async fn transaction_in_mempool(&self, txid: TransactionId) -> Result<bool, KaspadError> {
        Ok(self.mempool.lock().unwrap().contains(&txid.as_bytes()))
    }
    async fn fee_estimate_sompi_per_gram(&self) -> Result<f64, KaspadError> {
        Ok(1.0)
    }
}

fn engine_config(mode: ExecutionMode, trigger: usize, target: usize) -> ConsolidationEngineConfig {
    ConsolidationEngineConfig {
        instance_id: "test-consolidator".to_owned(),
        poll_interval: Duration::from_secs(60),
        tick_timeout: Duration::from_secs(60),
        // No bounded retry in tests: the lock-held case returns immediately.
        lock_acquire_wait: Duration::ZERO,
        mode,
        trigger_utxo_count: trigger,
        target_utxo_count: target,
        max_inputs_per_tx: 5,
        max_txs_per_tick: 10,
        lock_namespace: TREASURY_SPEND_LOCK_NAMESPACE.to_owned(),
    }
}

// ---- tests ----------------------------------------------------------

#[tokio::test]
async fn below_ceiling_records_snapshot_without_planning() {
    let (pool, _ctr) = fresh_pool().await;
    let (secret, addr) = treasury();
    let script = pay_to_address_script(&addr);

    let mock = MockKaspad::default();
    mock.set_virtual_daa(2_000);
    mock.set_utxos(fragmented(3, &script)); // 3 below trigger 5 → idle, no campaign

    let engine = ConsolidationEngine::new(
        pool.clone(),
        mock,
        secret,
        addr,
        engine_config(ExecutionMode::Live, 5, 2),
    );
    let outcome = engine.run_once().await.expect("tick");
    let ConsolidationTickOutcome::Ran(report) = outcome else {
        panic!("expected a leader tick");
    };
    assert!(report.below_ceiling);
    assert!(!report.campaign_active);
    assert_eq!(report.planned_batches, 0);
    assert!(report.submitted_txids.is_empty());

    let snap = treasury::latest(&pool)
        .await
        .expect("latest")
        .expect("snapshot");
    assert_eq!(snap.utxo_count, Some(3));
    assert_eq!(snap.daa_score, 2_000);
}

#[tokio::test]
async fn dry_run_plans_but_does_not_broadcast() {
    let (pool, _ctr) = fresh_pool().await;
    let (secret, addr) = treasury();
    let script = pay_to_address_script(&addr);

    let mock = MockKaspad::default();
    mock.set_virtual_daa(2_000);
    mock.set_utxos(fragmented(10, &script)); // 10 > trigger 5 → sweep active

    let engine = ConsolidationEngine::new(
        pool.clone(),
        mock,
        secret,
        addr,
        engine_config(ExecutionMode::DryRun, 5, 2),
    );
    let outcome = engine.run_once().await.expect("tick");
    let ConsolidationTickOutcome::Ran(report) = outcome else {
        panic!("expected a leader tick");
    };
    assert!(!report.below_ceiling);
    assert!(report.campaign_active);
    // 10 inputs, cap 5 → two 5-input batches.
    assert_eq!(report.planned_batches, 2);
    assert_eq!(
        report.submitted_txids.len(),
        2,
        "dry-run still computes txids"
    );
    assert!(report.submit_errors.is_empty());

    // Snapshot recorded; no audit entries because nothing was broadcast.
    let snap = treasury::latest(&pool)
        .await
        .expect("latest")
        .expect("snapshot");
    assert_eq!(snap.utxo_count, Some(10));
    let entries = audit::list_for_subject(&pool, "treasury_snapshot", snap.id, 100)
        .await
        .expect("audit");
    assert!(entries.is_empty(), "dry-run writes no consolidation audit");
}

#[tokio::test]
async fn live_broadcasts_every_planned_batch_and_audits() {
    let (pool, _ctr) = fresh_pool().await;
    let (secret, addr) = treasury();
    let script = pay_to_address_script(&addr);

    let mock = MockKaspad::default();
    mock.set_virtual_daa(2_000);
    mock.set_utxos(fragmented(10, &script)); // 10 > trigger 5 → sweep active

    let engine = ConsolidationEngine::new(
        pool.clone(),
        mock,
        secret,
        addr,
        engine_config(ExecutionMode::Live, 5, 2),
    );
    let outcome = engine.run_once().await.expect("tick");
    let ConsolidationTickOutcome::Ran(report) = outcome else {
        panic!("expected a leader tick");
    };
    assert_eq!(report.planned_batches, 2);
    assert_eq!(report.submitted_txids.len(), 2);
    assert!(report.submit_errors.is_empty());

    let snap = treasury::latest(&pool)
        .await
        .expect("latest")
        .expect("snapshot");
    assert_eq!(snap.utxo_count, Some(10));
    let entries = audit::list_for_subject(&pool, "treasury_snapshot", snap.id, 100)
        .await
        .expect("audit");
    assert_eq!(entries.len(), 2, "one audit row per broadcast batch");
    assert!(entries.iter().all(|e| e.actor == "treasury-consolidation"));
    assert!(entries.iter().all(|e| e.action == "treasury.consolidate"));
}

#[tokio::test]
async fn skips_when_treasury_lock_held_elsewhere() {
    let (pool, _ctr) = fresh_pool().await;
    let (secret, addr) = treasury();
    let script = pay_to_address_script(&addr);

    // Hold the shared treasury-spend lock from "another spender".
    let held = AdvisoryLock::try_acquire(&pool, advisory_key(TREASURY_SPEND_LOCK_NAMESPACE))
        .await
        .expect("acquire")
        .expect("lock free");

    let mock = MockKaspad::default();
    mock.set_virtual_daa(2_000);
    mock.set_utxos(fragmented(10, &script));

    let engine = ConsolidationEngine::new(
        pool.clone(),
        mock,
        secret,
        addr,
        engine_config(ExecutionMode::Live, 5, 2),
    );
    let outcome = engine.run_once().await.expect("tick");
    assert!(matches!(
        outcome,
        ConsolidationTickOutcome::SkippedNotLeader
    ));

    // A skipped tick does no work: no snapshot row was written.
    assert!(treasury::latest(&pool).await.expect("latest").is_none());

    held.release().await.expect("release");
}

#[tokio::test]
async fn hysteresis_starts_above_trigger_and_sweeps_down_to_target() {
    let (pool, _ctr) = fresh_pool().await;
    let (secret, addr) = treasury();
    let script = pay_to_address_script(&addr);

    let mock = MockKaspad::default();
    mock.set_virtual_daa(2_000);
    // Between target(2) and trigger(20): no campaign yet.
    mock.set_utxos(fragmented(10, &script));

    // Clone shares the mock's interior state so we can drive the UTXO count.
    let engine = ConsolidationEngine::new(
        pool.clone(),
        mock.clone(),
        secret,
        addr,
        engine_config(ExecutionMode::DryRun, 20, 2),
    );

    // Tick 1: count in (target, trigger] and no active campaign → idle.
    let ConsolidationTickOutcome::Ran(r1) = engine.run_once().await.expect("tick1") else {
        panic!("leader");
    };
    assert!(
        r1.below_ceiling && !r1.campaign_active,
        "below trigger ⇒ idle"
    );
    assert_eq!(r1.planned_batches, 0);

    // Tick 2: count crosses the trigger → campaign activates and plans.
    mock.set_utxos(fragmented(30, &script));
    let ConsolidationTickOutcome::Ran(r2) = engine.run_once().await.expect("tick2") else {
        panic!("leader");
    };
    assert!(
        r2.campaign_active && !r2.below_ceiling,
        "above trigger ⇒ sweeping"
    );
    assert_eq!(r2.planned_batches, 6, "30 inputs / cap 5 = 6 batches");

    // Tick 3: count drops back below the trigger but the latch keeps sweeping.
    mock.set_utxos(fragmented(8, &script));
    let ConsolidationTickOutcome::Ran(r3) = engine.run_once().await.expect("tick3") else {
        panic!("leader");
    };
    assert!(r3.campaign_active, "latched: keep sweeping below trigger");
    assert_eq!(r3.planned_batches, 2, "8 inputs / cap 5 = [5,3]");

    // Tick 4: count reaches the target floor → campaign ends, engine idles.
    mock.set_utxos(fragmented(2, &script));
    let ConsolidationTickOutcome::Ran(r4) = engine.run_once().await.expect("tick4") else {
        panic!("leader");
    };
    assert!(
        !r4.campaign_active && r4.below_ceiling,
        "at floor ⇒ campaign ends"
    );
    assert_eq!(r4.planned_batches, 0);
}

#[tokio::test]
async fn does_not_sweep_unconfirmed_payout_change_outputs() {
    let (pool, _ctr) = fresh_pool().await;
    let (secret, addr) = treasury();
    let script = pay_to_address_script(&addr);

    // Record an in-flight (submitted) payout whose transaction produced the
    // treasury coins under test — every `fragmented` coin shares txid [7u8; 32].
    // Its change output must not be swept until the payout confirms, or the
    // payout's own confirmation (which detects the change coin) would strand.
    let wallet_id: i64 =
        sqlx::query_scalar("INSERT INTO wallet (address, network) VALUES ($1, $2) RETURNING id")
            .bind("kaspatest:qq5fysv96t636u4slda59daza6tn5j5p5x5953hs6dstajuw0u6l6ez5wz3gd")
            .bind("testnet-10")
            .fetch_one(&pool)
            .await
            .expect("wallet");
    let cycle_id: i64 = sqlx::query_scalar(
        "INSERT INTO payout_cycle (kind, daa_start, daa_end, idempotency_key)
         VALUES ('kas', 0, 1, 'kas-protect-test') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .expect("cycle");
    sqlx::query(
        "INSERT INTO payout (cycle_id, wallet_id, amount_sompi, status, tx_hash, submitted_at)
         VALUES ($1, $2, 100, 'submitted', $3, now())",
    )
    .bind(cycle_id)
    .bind(wallet_id)
    .bind(vec![7_u8; 32])
    .execute(&pool)
    .await
    .expect("payout");

    let mock = MockKaspad::default();
    mock.set_virtual_daa(2_000);
    mock.set_utxos(fragmented(10, &script)); // 10 > trigger 5, but all protected

    let engine = ConsolidationEngine::new(
        pool.clone(),
        mock.clone(),
        secret,
        addr,
        engine_config(ExecutionMode::Live, 5, 2),
    );

    // Every spendable coin is a protected change output ⇒ nothing to sweep.
    let ConsolidationTickOutcome::Ran(report) = engine.run_once().await.expect("tick") else {
        panic!("leader");
    };
    assert_eq!(report.planned_batches, 0, "protected coins are not swept");
    assert!(report.submitted_txids.is_empty());
    let snap = treasury::latest(&pool)
        .await
        .expect("latest")
        .expect("snap");
    assert_eq!(
        snap.utxo_count,
        Some(0),
        "protected change outputs are excluded from the spendable set"
    );

    // Once the payout confirms, its change output is releasable and sweeps.
    sqlx::query("UPDATE payout SET status = 'confirmed', confirmed_at = now() WHERE cycle_id = $1")
        .bind(cycle_id)
        .execute(&pool)
        .await
        .expect("confirm");
    let ConsolidationTickOutcome::Ran(report2) = engine.run_once().await.expect("tick2") else {
        panic!("leader");
    };
    assert!(
        report2.planned_batches > 0,
        "confirmed-payout change is now eligible for consolidation"
    );
}
