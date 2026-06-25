//! Engine integration tests (M4.7): single-leader periodic loop over a real
//! testcontainer Postgres and an in-memory mock kaspad.
//!
//! Covers a full multi-tick settlement, leader-lock contention (a non-leader
//! tick does no work), and clean `run_loop` shutdown.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::too_many_lines
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
use katpool_db::repo::payout::{self, PayoutCycleStatus};
use katpool_db::repo::{block, coinbase_reward, share_allocation, wallet, worker};
use katpool_db::{PoolConfig, build_pool, migrate};
use katpool_domain::{BlockHash, CorrelationId, DaaScore, WalletAddress, WorkerName};
use katpool_idempotency::{AdvisoryLock, advisory_key};
use katpool_secrets::{TreasurySecret, from_hex};
use payout_kas::{
    DEFAULT_KAS_PAYOUT_THRESHOLD_SOMPI, ExecutionMode, KAS_PAYOUT_CONFIRMATION_DAA, KaspadClient,
    KaspadError, PayoutEngine, PayoutEngineConfig, TickOutcome, TreasuryUtxoSnapshot,
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
        max_connections: 8,
        acquire_timeout: Duration::from_secs(10),
        idle_timeout: Duration::from_secs(60),
        max_lifetime: Duration::from_secs(300),
        statement_timeout: Duration::from_secs(30),
        application_name: "payout-kas-engine-test".to_owned(),
    };
    let pool = build_pool(&cfg).await.expect("pool");
    migrate::run(&pool).await.expect("migrate");
    (pool, container)
}

fn keypair(hex: &str) -> Keypair {
    let secret = from_hex(hex).expect("valid key");
    Keypair::from_seckey_slice(secp256k1::SECP256K1, secret.expose_secret()).expect("kp")
}

fn mainnet_address_string(hex: &str) -> String {
    Address::new(
        Prefix::Mainnet,
        Version::PubKey,
        &keypair(hex).x_only_public_key().0.serialize(),
    )
    .to_string()
}

async fn seed_two_wallet_allocations(pool: &sqlx::PgPool) {
    let a1 = WalletAddress::new(mainnet_address_string(
        "2222222222222222222222222222222222222222222222222222222222222222",
    ))
    .expect("addr1");
    let a2 = WalletAddress::new(mainnet_address_string(
        "3333333333333333333333333333333333333333333333333333333333333333",
    ))
    .expect("addr2");
    let w1 = wallet::ensure(pool, &a1, "mainnet")
        .await
        .expect("wallet 1");
    let wk = worker::ensure(pool, w1.id, &WorkerName::new("rig-01").expect("wk"))
        .await
        .expect("worker");
    let w2 = wallet::ensure(pool, &a2, "mainnet")
        .await
        .expect("wallet 2");

    let hash = BlockHash::from_bytes([9_u8; 32]);
    let _block_id = block::insert(
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
}

fn treasury() -> (TreasurySecret, Address, ScriptPublicKey) {
    let secret = from_hex(TREASURY_HEX).expect("valid key");
    let addr = Address::new(
        Prefix::Mainnet,
        Version::PubKey,
        &keypair(TREASURY_HEX).x_only_public_key().0.serialize(),
    );
    let script = pay_to_address_script(&addr);
    (secret, addr, script)
}

fn engine_config(namespace: &str, mode: ExecutionMode) -> PayoutEngineConfig {
    PayoutEngineConfig {
        instance_id: "test".to_owned(),
        poll_interval: Duration::from_secs(60),
        cycle_span_daa: 10_000,
        threshold_sompi: DEFAULT_KAS_PAYOUT_THRESHOLD_SOMPI,
        max_payout_sompi_per_cycle: None,
        mode,
        lock_namespace: namespace.to_owned(),
    }
}

// ---- mock kaspad (shareable handle) ---------------------------------

#[derive(Clone, Default)]
struct MockKaspad {
    inner: Arc<MockInner>,
}

#[derive(Default)]
struct MockInner {
    virtual_daa: Mutex<u64>,
    utxos: Mutex<Vec<TreasuryUtxoSnapshot>>,
    submitted: Mutex<Vec<Transaction>>,
    mempool: Mutex<HashSet<[u8; 32]>>,
}

impl MockKaspad {
    fn set_virtual_daa(&self, daa: u64) {
        *self.inner.virtual_daa.lock().unwrap() = daa;
    }
    fn set_utxos(&self, utxos: Vec<TreasuryUtxoSnapshot>) {
        *self.inner.utxos.lock().unwrap() = utxos;
    }
    fn submitted_count(&self) -> usize {
        self.inner.submitted.lock().unwrap().len()
    }
}

#[async_trait]
impl KaspadClient for MockKaspad {
    async fn virtual_daa_score(&self) -> Result<u64, KaspadError> {
        Ok(*self.inner.virtual_daa.lock().unwrap())
    }
    async fn treasury_utxos(&self, _: &Address) -> Result<Vec<TreasuryUtxoSnapshot>, KaspadError> {
        Ok(self.inner.utxos.lock().unwrap().clone())
    }
    async fn submit_transaction(
        &self,
        tx: &Transaction,
        _allow_orphan: bool,
    ) -> Result<TransactionId, KaspadError> {
        let id = tx.id();
        self.inner.submitted.lock().unwrap().push(tx.clone());
        self.inner.mempool.lock().unwrap().insert(id.as_bytes());
        Ok(id)
    }
    async fn transaction_in_mempool(&self, txid: TransactionId) -> Result<bool, KaspadError> {
        Ok(self
            .inner
            .mempool
            .lock()
            .unwrap()
            .contains(&txid.as_bytes()))
    }
    async fn fee_estimate_sompi_per_gram(&self) -> Result<f64, KaspadError> {
        Ok(1.0)
    }
}

fn funding(amount: u64, script: &ScriptPublicKey) -> TreasuryUtxoSnapshot {
    TreasuryUtxoSnapshot {
        outpoint: TransactionOutpoint {
            transaction_id: TransactionId::from_bytes([7_u8; 32]),
            index: 0,
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

fn change_for(txid_bytes: [u8; 32], daa: u64, script: &ScriptPublicKey) -> TreasuryUtxoSnapshot {
    TreasuryUtxoSnapshot {
        outpoint: TransactionOutpoint {
            transaction_id: TransactionId::from_bytes(txid_bytes),
            index: 2,
        },
        entry: UtxoEntry {
            amount: 4_000_000_000,
            script_public_key: script.clone(),
            block_daa_score: daa,
            is_coinbase: false,
            covenant_id: None,
        },
    }
}

fn cycle_id_of(outcome: &TickOutcome) -> i64 {
    match outcome {
        TickOutcome::Ran(r) => r.cycle_id,
        TickOutcome::SkippedNotLeader => panic!("expected a leader tick"),
    }
}

// ---- tests ----------------------------------------------------------

#[tokio::test]
async fn engine_ticks_drive_cycle_to_settlement() {
    let (pool, _ctr) = fresh_pool().await;
    seed_two_wallet_allocations(&pool).await;
    let (secret, treasury_addr, treasury_script) = treasury();

    let mock = MockKaspad::default();
    mock.set_virtual_daa(5_000);
    mock.set_utxos(vec![funding(10_000_000_000, &treasury_script)]);

    let engine = PayoutEngine::new(
        pool.clone(),
        mock.clone(),
        secret,
        treasury_addr,
        engine_config("test:engine-settle", ExecutionMode::Live),
    )
    .expect("engine");

    // Tick 1: plan + broadcast; rows submitted, still pending in mempool.
    let t1 = engine.run_once().await.expect("tick1");
    let cycle_id = cycle_id_of(&t1);
    assert_eq!(mock.submitted_count(), 1, "one tx broadcast");
    let mid = payout::get_cycle(&pool, cycle_id).await.expect("cycle");
    assert_eq!(mid.status, PayoutCycleStatus::Broadcasting);

    let rows = payout::list_for_cycle(&pool, cycle_id).await.expect("rows");
    let txid_bytes: [u8; 32] = rows[0]
        .tx_hash
        .clone()
        .expect("tx_hash")
        .try_into()
        .expect("32 bytes");

    // Tick 2: change coin appears on chain (below depth) ⇒ accepted, no re-pay.
    mock.set_utxos(vec![change_for(txid_bytes, 5_000, &treasury_script)]);
    let _t2 = engine.run_once().await.expect("tick2");
    assert_eq!(mock.submitted_count(), 1, "no double broadcast on resume");

    // Tick 3: matured past depth ⇒ confirmed ⇒ cycle settles.
    mock.set_virtual_daa(5_000 + KAS_PAYOUT_CONFIRMATION_DAA);
    let _t3 = engine.run_once().await.expect("tick3");
    let settled = payout::get_cycle(&pool, cycle_id)
        .await
        .expect("cycle settled");
    assert_eq!(settled.status, PayoutCycleStatus::Settled);

    let eligible = payout::list_kas_eligible_wallets(&pool, DEFAULT_KAS_PAYOUT_THRESHOLD_SOMPI)
        .await
        .expect("eligible");
    assert!(eligible.is_empty(), "balances fully paid");
}

#[tokio::test]
async fn non_leader_tick_does_no_work() {
    let (pool, _ctr) = fresh_pool().await;
    seed_two_wallet_allocations(&pool).await;
    let (secret, treasury_addr, treasury_script) = treasury();

    let mock = MockKaspad::default();
    mock.set_virtual_daa(5_000);
    mock.set_utxos(vec![funding(10_000_000_000, &treasury_script)]);

    let namespace = "test:engine-contend";
    // Another instance already holds the leader lock.
    let held = AdvisoryLock::try_acquire(&pool, advisory_key(namespace))
        .await
        .expect("acquire")
        .expect("free");

    let engine = PayoutEngine::new(
        pool.clone(),
        mock.clone(),
        secret,
        treasury_addr,
        engine_config(namespace, ExecutionMode::Live),
    )
    .expect("engine");

    let skipped = engine.run_once().await.expect("contended tick");
    assert!(matches!(skipped, TickOutcome::SkippedNotLeader));
    assert_eq!(mock.submitted_count(), 0, "non-leader broadcasts nothing");

    // Once leadership frees up, the engine runs.
    held.release().await.expect("release");
    let ran = engine.run_once().await.expect("leader tick");
    assert!(matches!(ran, TickOutcome::Ran(_)));
    assert_eq!(mock.submitted_count(), 1);
}

#[tokio::test]
async fn run_loop_exits_cleanly_on_shutdown() {
    let (pool, _ctr) = fresh_pool().await;
    seed_two_wallet_allocations(&pool).await;
    let (secret, treasury_addr, treasury_script) = treasury();

    let mock = MockKaspad::default();
    mock.set_virtual_daa(5_000);
    mock.set_utxos(vec![funding(10_000_000_000, &treasury_script)]);

    let mut config = engine_config("test:engine-loop", ExecutionMode::DryRun);
    config.poll_interval = Duration::from_millis(50);
    let engine =
        PayoutEngine::new(pool.clone(), mock, secret, treasury_addr, config).expect("engine");

    let (tx, rx) = tokio::sync::watch::channel(false);
    let handle = tokio::spawn(engine.run_loop(rx));
    tokio::time::sleep(Duration::from_millis(180)).await;
    tx.send(true).expect("signal shutdown");

    let result = tokio::time::timeout(Duration::from_secs(2), handle).await;
    assert!(result.is_ok(), "run_loop did not exit promptly");
    assert!(result.unwrap().is_ok(), "join ok");
}
