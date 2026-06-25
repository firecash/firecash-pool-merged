//! Engine integration tests (M5.5b): the single-leader periodic loop over a
//! real testcontainer Postgres, an address-keyed in-memory mock kaspad, and a
//! fixed mock floor-price source.
//!
//! Covers a full multi-tick settlement (plan → commit → reveal → complete →
//! credit → settle), leader-lock mutual exclusion (a non-leader tick does no
//! work), and clean `run_loop` shutdown.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::too_many_lines
)]

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use kaspa_addresses::{Address, Prefix, Version};
use kaspa_consensus_core::tx::{
    ScriptPublicKey, Transaction, TransactionId, TransactionOutpoint, UtxoEntry,
};
use kaspa_txscript::pay_to_address_script;
use katpool_db::repo::payout::{self, PayoutCycleStatus, PayoutStatus};
use katpool_db::repo::{nacho_rebate, wallet};
use katpool_db::{PoolConfig, build_pool, migrate};
use katpool_domain::WalletAddress;
use katpool_idempotency::{AdvisoryLock, advisory_key};
use katpool_secrets::{TreasurySecret, from_hex};
use payout_kas::{
    ExecutionMode, KAS_PAYOUT_CONFIRMATION_DAA, KaspadClient, KaspadError, TreasuryUtxoSnapshot,
};
use payout_krc20::{
    DEFAULT_COMMIT_AMOUNT_SOMPI, FloorPrice, FloorPriceSource, Krc20PayoutEngine,
    Krc20PayoutEngineConfig, Krc20TickOutcome, Krc20Transfer, QuoteError,
    build_transfer_inscription, commit_address, commit_script_public_key,
};
use secp256k1::Keypair;
use testcontainers::ContainerAsync;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;

const TREASURY_HEX: &str = "1111111111111111111111111111111111111111111111111111111111111111";
const RECIPIENT_HEX: &str = "2222222222222222222222222222222222222222222222222222222222222222";
const PENDING: i64 = 200_000_000;

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
        application_name: "payout-krc20-engine-test".to_owned(),
    };
    let pool = build_pool(&cfg).await.expect("pool");
    migrate::run(&pool).await.expect("migrate");
    (pool, container)
}

fn keypair(hex: &str) -> Keypair {
    let secret = from_hex(hex).expect("valid key");
    Keypair::from_seckey_slice(secp256k1::SECP256K1, secret.expose_secret()).expect("kp")
}

fn xonly(hex: &str) -> [u8; 32] {
    keypair(hex).x_only_public_key().0.serialize()
}

fn mainnet_address(hex: &str) -> Address {
    Address::new(Prefix::Mainnet, Version::PubKey, &xonly(hex))
}

fn treasury() -> (TreasurySecret, Address) {
    (
        from_hex(TREASURY_HEX).expect("key"),
        mainnet_address(TREASURY_HEX),
    )
}

/// Commit P2SH script + address the engine will derive for this recipient at
/// the identity floor price (`nacho == PENDING`).
fn commit_p2sh(recipient: &Address) -> (ScriptPublicKey, Address) {
    let transfer = Krc20Transfer::new("NACHO", PENDING.to_string(), recipient.to_string());
    let redeem = build_transfer_inscription(&xonly(TREASURY_HEX), &transfer).expect("redeem");
    (
        commit_script_public_key(&redeem),
        commit_address(&redeem, Prefix::Mainnet).expect("p2sh"),
    )
}

async fn seed_eligible_wallet(pool: &sqlx::PgPool) -> Address {
    let recipient = mainnet_address(RECIPIENT_HEX);
    let wallet_addr = WalletAddress::new(recipient.to_string()).expect("wallet addr");
    let w = wallet::ensure(pool, &wallet_addr, "mainnet")
        .await
        .expect("wallet");
    nacho_rebate::accrue(pool, w.id, PENDING)
        .await
        .expect("accrue");
    recipient
}

async fn only_payout_id(pool: &sqlx::PgPool, cycle_id: i64) -> i64 {
    let transfers = payout::list_krc20_for_cycle(pool, cycle_id)
        .await
        .expect("transfers");
    assert_eq!(transfers.len(), 1, "exactly one planned transfer");
    transfers[0].payout_id
}

async fn recorded_hash(pool: &sqlx::PgPool, payout_id: i64, commit: bool) -> TransactionId {
    let row = payout::get_payout(pool, payout_id).await.expect("payout");
    let bytes: [u8; 32] = if commit {
        row.krc20_commit_hash
    } else {
        row.krc20_reveal_hash
    }
    .expect("hash recorded")
    .try_into()
    .expect("32 bytes");
    TransactionId::from_bytes(bytes)
}

fn engine_config(namespace: &str, mode: ExecutionMode) -> Krc20PayoutEngineConfig {
    Krc20PayoutEngineConfig {
        instance_id: "test".to_owned(),
        poll_interval: Duration::from_secs(60),
        // No bounded retry in tests: the lock-held case returns immediately.
        lock_acquire_wait: Duration::ZERO,
        cycle_span_daa: 1_000_000,
        mode,
        lock_namespace: namespace.to_owned(),
        min_pending_sompi: 1,
        min_nacho_base_units: 100_000_000,
        ticker: "NACHO".to_owned(),
        commit_amount_sompi: DEFAULT_COMMIT_AMOUNT_SOMPI,
        batch_limit: 100,
        max_nacho_base_units_per_cycle: None,
    }
}

// ---- mock floor price (identity: nacho == pending) ------------------

struct FixedFloor(FloorPrice);

#[async_trait]
impl FloorPriceSource for FixedFloor {
    async fn floor_price(&self, _ticker: &str) -> Result<FloorPrice, QuoteError> {
        Ok(self.0)
    }
}

fn identity_quote() -> FixedFloor {
    FixedFloor(FloorPrice::from_decimal_str("1").expect("price"))
}

// ---- mock kaspad (cloneable, address-keyed UTXOs) -------------------

#[derive(Clone, Default)]
struct MockKaspad {
    inner: Arc<MockInner>,
}

#[derive(Default)]
struct MockInner {
    virtual_daa: Mutex<u64>,
    by_address: Mutex<HashMap<String, Vec<TreasuryUtxoSnapshot>>>,
    submitted: Mutex<Vec<Transaction>>,
    mempool: Mutex<HashSet<[u8; 32]>>,
}

impl MockKaspad {
    fn set_daa(&self, daa: u64) {
        *self.inner.virtual_daa.lock().unwrap() = daa;
    }
    fn set_utxos(&self, address: &Address, utxos: Vec<TreasuryUtxoSnapshot>) {
        self.inner
            .by_address
            .lock()
            .unwrap()
            .insert(address.to_string(), utxos);
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
    async fn treasury_utxos(
        &self,
        address: &Address,
    ) -> Result<Vec<TreasuryUtxoSnapshot>, KaspadError> {
        Ok(self
            .inner
            .by_address
            .lock()
            .unwrap()
            .get(&address.to_string())
            .cloned()
            .unwrap_or_default())
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

fn coin_for(
    txid: TransactionId,
    index: u32,
    daa: u64,
    script: &ScriptPublicKey,
) -> TreasuryUtxoSnapshot {
    TreasuryUtxoSnapshot {
        outpoint: TransactionOutpoint {
            transaction_id: txid,
            index,
        },
        entry: UtxoEntry {
            amount: DEFAULT_COMMIT_AMOUNT_SOMPI,
            script_public_key: script.clone(),
            block_daa_score: daa,
            is_coinbase: false,
            covenant_id: None,
        },
    }
}

fn cycle_id_of(outcome: &Krc20TickOutcome) -> i64 {
    match outcome {
        Krc20TickOutcome::Ran(r) => r.cycle_id,
        Krc20TickOutcome::SkippedNotLeader => panic!("expected a leader tick"),
    }
}

// ---- tests ----------------------------------------------------------

#[tokio::test]
async fn engine_ticks_drive_cycle_to_settlement() {
    let (pool, _ctr) = fresh_pool().await;
    let recipient = seed_eligible_wallet(&pool).await;
    let (secret, treasury_addr) = treasury();
    let treasury_script = pay_to_address_script(&treasury_addr);
    let (commit_spk, commit_addr) = commit_p2sh(&recipient);

    let mock = MockKaspad::default();
    mock.set_daa(1_000);
    mock.set_utxos(
        &treasury_addr,
        vec![funding(10_000_000_000, &treasury_script)],
    );

    let engine = Krc20PayoutEngine::new(
        pool.clone(),
        mock.clone(),
        secret,
        treasury_addr.clone(),
        identity_quote(),
        engine_config("test:krc20-engine-settle", ExecutionMode::Live),
    )
    .expect("engine");

    // Tick 1: plan the cycle + broadcast the commit.
    let t1 = engine.run_once().await.expect("tick1");
    let cycle_id = cycle_id_of(&t1);
    let payout_id = only_payout_id(&pool, cycle_id).await;
    assert_eq!(mock.submitted_count(), 1, "commit broadcast");
    let commit_id = recorded_hash(&pool, payout_id, true).await;
    assert_eq!(
        payout::get_cycle(&pool, cycle_id)
            .await
            .expect("cycle")
            .status,
        PayoutCycleStatus::Broadcasting
    );

    // Tick 2: commit output on chain ⇒ broadcast the reveal.
    mock.set_utxos(&commit_addr, vec![coin_for(commit_id, 0, 0, &commit_spk)]);
    let _t2 = engine.run_once().await.expect("tick2");
    assert_eq!(mock.submitted_count(), 2, "commit + reveal");
    let reveal_id = recorded_hash(&pool, payout_id, false).await;

    // Tick 3: reveal matured past depth ⇒ completed + credited ⇒ cycle settles.
    let reveal_daa = 5_000;
    mock.set_utxos(
        &treasury_addr,
        vec![coin_for(reveal_id, 0, reveal_daa, &treasury_script)],
    );
    mock.set_daa(reveal_daa + KAS_PAYOUT_CONFIRMATION_DAA);
    let t3 = engine.run_once().await.expect("tick3");
    assert_eq!(mock.submitted_count(), 2, "no re-broadcast once confirmed");
    match t3 {
        Krc20TickOutcome::Ran(r) => {
            assert_eq!(r.settle.completed, 1);
            assert_eq!(r.credit.credited, 1);
            assert_eq!(r.credit.paid_sompi, PENDING);
            assert_eq!(r.status, PayoutCycleStatus::Settled);
        }
        Krc20TickOutcome::SkippedNotLeader => panic!("leader tick expected"),
    }

    // Rebate credited exactly, payout confirmed, wallet no longer eligible.
    assert_eq!(
        payout::get_payout(&pool, payout_id)
            .await
            .expect("payout")
            .status,
        PayoutStatus::Confirmed
    );
    let eligible = payout::list_krc20_eligible_wallets(&pool, 1, 100)
        .await
        .expect("eligible");
    assert!(eligible.is_empty(), "fully paid wallet drops out");
}

#[tokio::test]
async fn non_leader_tick_does_no_work() {
    let (pool, _ctr) = fresh_pool().await;
    seed_eligible_wallet(&pool).await;
    let (secret, treasury_addr) = treasury();
    let treasury_script = pay_to_address_script(&treasury_addr);

    let mock = MockKaspad::default();
    mock.set_daa(1_000);
    mock.set_utxos(
        &treasury_addr,
        vec![funding(10_000_000_000, &treasury_script)],
    );

    let namespace = "test:krc20-engine-contend";
    // Another instance already holds the leader lock.
    let held = AdvisoryLock::try_acquire(&pool, advisory_key(namespace))
        .await
        .expect("acquire")
        .expect("free");

    let engine = Krc20PayoutEngine::new(
        pool.clone(),
        mock.clone(),
        secret,
        treasury_addr,
        identity_quote(),
        engine_config(namespace, ExecutionMode::Live),
    )
    .expect("engine");

    let skipped = engine.run_once().await.expect("contended tick");
    assert!(matches!(skipped, Krc20TickOutcome::SkippedNotLeader));
    assert_eq!(mock.submitted_count(), 0, "non-leader broadcasts nothing");
    assert!(
        payout::list_krc20_by_status(
            &pool,
            &[katpool_db::repo::payout::Krc20TransferStatus::Pending],
            100
        )
        .await
        .expect("transfers")
        .is_empty(),
        "non-leader plans nothing"
    );

    // Once leadership frees up, the engine runs.
    held.release().await.expect("release");
    let ran = engine.run_once().await.expect("leader tick");
    assert!(matches!(ran, Krc20TickOutcome::Ran(_)));
    assert_eq!(mock.submitted_count(), 1, "commit broadcast once leader");
}

#[tokio::test]
async fn run_loop_exits_cleanly_on_shutdown() {
    let (pool, _ctr) = fresh_pool().await;
    seed_eligible_wallet(&pool).await;
    let (secret, treasury_addr) = treasury();
    let treasury_script = pay_to_address_script(&treasury_addr);

    let mock = MockKaspad::default();
    mock.set_daa(1_000);
    mock.set_utxos(
        &treasury_addr,
        vec![funding(10_000_000_000, &treasury_script)],
    );

    let mut config = engine_config("test:krc20-engine-loop", ExecutionMode::DryRun);
    config.poll_interval = Duration::from_millis(50);
    let engine = Krc20PayoutEngine::new(
        pool.clone(),
        mock,
        secret,
        treasury_addr,
        identity_quote(),
        config,
    )
    .expect("engine");

    let (tx, rx) = tokio::sync::watch::channel(false);
    let handle = tokio::spawn(engine.run_loop(rx));
    tokio::time::sleep(Duration::from_millis(180)).await;
    tx.send(true).expect("signal shutdown");

    let result = tokio::time::timeout(Duration::from_secs(2), handle).await;
    assert!(result.is_ok(), "run_loop did not exit promptly");
    assert!(result.unwrap().is_ok(), "join ok");
}
