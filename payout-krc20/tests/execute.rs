//! Orchestration tests (M5.4b): drive a KRC-20 transfer through the
//! `pending → commit_submitted → reveal_submitted → completed` state machine
//! against an in-memory, address-keyed mock kaspad and a real testcontainer
//! Postgres.
//!
//! These pin the crash-safe, idempotent contract deterministically: the txid
//! is recorded before broadcast, a crash-before-broadcast re-broadcasts the
//! *same* commit (never a second distinct spend), UTXO drift is refused, and a
//! dry-run touches neither the database nor the wire.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::too_many_lines
)]

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use kaspa_addresses::{Address, Prefix, Version};
use kaspa_consensus_core::tx::{
    ScriptPublicKey, Transaction, TransactionId, TransactionOutpoint, UtxoEntry,
};
use kaspa_txscript::pay_to_address_script;
use katpool_db::repo::payout::{self, Krc20PendingTransfer, Krc20TransferStatus, PayoutKind};
use katpool_db::repo::wallet;
use katpool_db::{PoolConfig, build_pool, migrate};
use katpool_domain::{DaaScore, WalletAddress};
use katpool_secrets::{TreasurySecret, from_hex};
use katpool_storagemass::FeeRate;
use payout_kas::{
    ExecutionMode, KAS_PAYOUT_CONFIRMATION_DAA, KaspadClient, KaspadError, TreasuryUtxoSnapshot,
};
use payout_krc20::{
    DEFAULT_COMMIT_AMOUNT_SOMPI, Krc20Transfer, TransferStep, advance_transfer,
    build_transfer_inscription, commit_address, commit_script_public_key, settle_pending,
};
use secp256k1::Keypair;
use testcontainers::ContainerAsync;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;

const TREASURY_HEX: &str = "1111111111111111111111111111111111111111111111111111111111111111";
const RECIPIENT_HEX: &str = "2222222222222222222222222222222222222222222222222222222222222222";
const RECIPIENT2_HEX: &str = "3333333333333333333333333333333333333333333333333333333333333333";
const NACHO_AMOUNT: i64 = 123_456;

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
        application_name: "payout-krc20-execute-test".to_owned(),
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

/// The inscription the executor will rebuild for this transfer, and its P2SH
/// commit address — derived identically to how the executor derives them.
fn inscription_and_p2sh(recipient: &Address) -> (Krc20Transfer, ScriptPublicKey, Address) {
    let transfer = Krc20Transfer::new("NACHO", NACHO_AMOUNT.to_string(), recipient.to_string());
    let redeem = build_transfer_inscription(&xonly(TREASURY_HEX), &transfer).expect("redeem");
    let spk = commit_script_public_key(&redeem);
    let addr = commit_address(&redeem, Prefix::Mainnet).expect("p2sh addr");
    (transfer, spk, addr)
}

/// Seed wallet + KRC-20 cycle + payout + pending transfer; return its id and
/// the recipient address.
async fn seed_transfer(pool: &sqlx::PgPool) -> (i64, Address) {
    let recipient = mainnet_address(RECIPIENT_HEX);
    let wallet_addr = WalletAddress::new(recipient.to_string()).expect("wallet addr");
    let w = wallet::ensure(pool, &wallet_addr, "mainnet")
        .await
        .expect("wallet");

    let cycle = payout::create_cycle(
        pool,
        PayoutKind::Krc20Nacho,
        DaaScore::new(1_000),
        DaaScore::new(2_000),
    )
    .await
    .expect("cycle");
    let p = payout::insert_payout(pool, cycle.id, w.id, 500_000_000)
        .await
        .expect("payout");

    let (_, _, p2sh) = inscription_and_p2sh(&recipient);
    payout::insert_krc20_pending(
        pool,
        p.id,
        i64::try_from(DEFAULT_COMMIT_AMOUNT_SOMPI).expect("fits"),
        NACHO_AMOUNT,
        &p2sh.to_string(),
    )
    .await
    .expect("pending");
    (p.id, recipient)
}

async fn reload(pool: &sqlx::PgPool, payout_id: i64) -> Krc20PendingTransfer {
    payout::list_krc20_by_status(
        pool,
        &[
            Krc20TransferStatus::Pending,
            Krc20TransferStatus::CommitSubmitted,
            Krc20TransferStatus::RevealSubmitted,
            Krc20TransferStatus::Completed,
            Krc20TransferStatus::Failed,
        ],
        100,
    )
    .await
    .expect("list")
    .into_iter()
    .find(|t| t.payout_id == payout_id)
    .expect("transfer present")
}

async fn recorded_commit(pool: &sqlx::PgPool, payout_id: i64) -> TransactionId {
    let row = payout::get_payout(pool, payout_id).await.expect("payout");
    let bytes: [u8; 32] = row
        .krc20_commit_hash
        .expect("commit hash recorded")
        .try_into()
        .expect("32 bytes");
    TransactionId::from_bytes(bytes)
}

async fn recorded_reveal(pool: &sqlx::PgPool, payout_id: i64) -> TransactionId {
    let row = payout::get_payout(pool, payout_id).await.expect("payout");
    let bytes: [u8; 32] = row
        .krc20_reveal_hash
        .expect("reveal hash recorded")
        .try_into()
        .expect("32 bytes");
    TransactionId::from_bytes(bytes)
}

// ---- mock kaspad (address-keyed UTXOs) ------------------------------

#[derive(Default)]
struct MockKaspad {
    virtual_daa: Mutex<u64>,
    by_address: Mutex<HashMap<String, Vec<TreasuryUtxoSnapshot>>>,
    submitted: Mutex<Vec<Transaction>>,
    mempool: Mutex<HashSet<[u8; 32]>>,
    fail_next_submit: Mutex<bool>,
}

impl MockKaspad {
    fn set_daa(&self, daa: u64) {
        *self.virtual_daa.lock().unwrap() = daa;
    }
    fn set_utxos(&self, address: &Address, utxos: Vec<TreasuryUtxoSnapshot>) {
        self.by_address
            .lock()
            .unwrap()
            .insert(address.to_string(), utxos);
    }
    fn clear_mempool(&self) {
        self.mempool.lock().unwrap().clear();
    }
    fn arm_submit_failure(&self) {
        *self.fail_next_submit.lock().unwrap() = true;
    }
    fn submitted_ids(&self) -> Vec<[u8; 32]> {
        self.submitted
            .lock()
            .unwrap()
            .iter()
            .map(|t| t.id().as_bytes())
            .collect()
    }
}

#[async_trait]
impl KaspadClient for MockKaspad {
    async fn virtual_daa_score(&self) -> Result<u64, KaspadError> {
        Ok(*self.virtual_daa.lock().unwrap())
    }
    async fn treasury_utxos(
        &self,
        address: &Address,
    ) -> Result<Vec<TreasuryUtxoSnapshot>, KaspadError> {
        Ok(self
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
        if std::mem::replace(&mut *self.fail_next_submit.lock().unwrap(), false) {
            return Err(KaspadError::Rpc("injected submit failure".to_owned()));
        }
        let id = tx.id();
        self.submitted.lock().unwrap().push(tx.clone());
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

/// Relay-floor fee policy for the first (pending) plan; the resolved fees are
/// then frozen onto the row and replayed on every later step.
fn fee_rate() -> FeeRate {
    FeeRate::from_feerate(0.0)
}

// ---- tests ----------------------------------------------------------

#[tokio::test]
async fn drives_pending_through_completed_and_is_idempotent() {
    let (pool, _ctr) = fresh_pool().await;
    let (payout_id, recipient) = seed_transfer(&pool).await;
    let (_, commit_spk, commit_addr) = inscription_and_p2sh(&recipient);
    let (secret, treasury_addr) = treasury();
    let treasury_script = pay_to_address_script(&treasury_addr);

    let mock = MockKaspad::default();
    mock.set_daa(1_000);
    mock.set_utxos(
        &treasury_addr,
        vec![funding(10_000_000_000, &treasury_script)],
    );

    // --- pending → commit_submitted: records intent, broadcasts commit ---
    let t = reload(&pool, payout_id).await;
    let step = advance_transfer(
        &pool,
        &mock,
        &secret,
        &treasury_addr,
        &fee_rate(),
        &t,
        ExecutionMode::Live,
    )
    .await
    .expect("advance pending");
    assert_eq!(step, TransferStep::CommitBroadcast);
    assert_eq!(
        reload(&pool, payout_id).await.status,
        Krc20TransferStatus::CommitSubmitted
    );
    let commit_id = recorded_commit(&pool, payout_id).await;
    assert_eq!(
        mock.submitted_ids(),
        vec![commit_id.as_bytes()],
        "exactly the commit hit the wire"
    );

    // --- idempotent: commit only in mempool ⇒ still pending, no re-broadcast ---
    let t = reload(&pool, payout_id).await;
    let step = advance_transfer(
        &pool,
        &mock,
        &secret,
        &treasury_addr,
        &fee_rate(),
        &t,
        ExecutionMode::Live,
    )
    .await
    .expect("advance commit (mempool)");
    assert_eq!(step, TransferStep::CommitPending);
    assert_eq!(mock.submitted_ids().len(), 1, "no double broadcast");

    // --- commit confirmed on chain (P2SH output spendable) → reveal ---
    mock.set_utxos(&commit_addr, vec![coin_for(commit_id, 0, 0, &commit_spk)]);
    let t = reload(&pool, payout_id).await;
    let step = advance_transfer(
        &pool,
        &mock,
        &secret,
        &treasury_addr,
        &fee_rate(),
        &t,
        ExecutionMode::Live,
    )
    .await
    .expect("advance commit (spendable)");
    assert_eq!(step, TransferStep::RevealBroadcast);
    assert_eq!(
        reload(&pool, payout_id).await.status,
        Krc20TransferStatus::RevealSubmitted
    );
    let reveal_id = recorded_reveal(&pool, payout_id).await;
    assert_eq!(mock.submitted_ids().len(), 2, "commit + reveal");
    assert!(mock.submitted_ids().contains(&reveal_id.as_bytes()));

    // --- reveal in mempool only ⇒ still pending ---
    let t = reload(&pool, payout_id).await;
    let step = advance_transfer(
        &pool,
        &mock,
        &secret,
        &treasury_addr,
        &fee_rate(),
        &t,
        ExecutionMode::Live,
    )
    .await
    .expect("advance reveal (mempool)");
    assert_eq!(step, TransferStep::RevealPending);

    // --- reveal on chain, below depth ⇒ accepted but still pending ---
    let reveal_daa = 5_000;
    mock.set_utxos(
        &treasury_addr,
        vec![coin_for(reveal_id, 0, reveal_daa, &treasury_script)],
    );
    mock.set_daa(reveal_daa + KAS_PAYOUT_CONFIRMATION_DAA - 1);
    let t = reload(&pool, payout_id).await;
    let step = advance_transfer(
        &pool,
        &mock,
        &secret,
        &treasury_addr,
        &fee_rate(),
        &t,
        ExecutionMode::Live,
    )
    .await
    .expect("advance reveal (accepted)");
    assert_eq!(step, TransferStep::RevealPending);
    assert_eq!(
        reload(&pool, payout_id).await.status,
        Krc20TransferStatus::RevealSubmitted
    );

    // --- reveal matured past depth ⇒ completed ---
    mock.set_daa(reveal_daa + KAS_PAYOUT_CONFIRMATION_DAA);
    let t = reload(&pool, payout_id).await;
    let step = advance_transfer(
        &pool,
        &mock,
        &secret,
        &treasury_addr,
        &fee_rate(),
        &t,
        ExecutionMode::Live,
    )
    .await
    .expect("advance reveal (confirmed)");
    assert_eq!(step, TransferStep::Completed);
    assert_eq!(
        reload(&pool, payout_id).await.status,
        Krc20TransferStatus::Completed
    );

    // --- terminal: re-run is a no-op ---
    let t = reload(&pool, payout_id).await;
    let step = advance_transfer(
        &pool,
        &mock,
        &secret,
        &treasury_addr,
        &fee_rate(),
        &t,
        ExecutionMode::Live,
    )
    .await
    .expect("advance completed");
    assert_eq!(step, TransferStep::NoChange);
    assert_eq!(
        mock.submitted_ids().len(),
        2,
        "completed transfer broadcasts nothing"
    );
}

#[tokio::test]
async fn crash_before_broadcast_rebroadcasts_the_same_commit() {
    let (pool, _ctr) = fresh_pool().await;
    let (payout_id, _recipient) = seed_transfer(&pool).await;
    let (secret, treasury_addr) = treasury();
    let treasury_script = pay_to_address_script(&treasury_addr);

    let mock = MockKaspad::default();
    mock.set_daa(1_000);
    mock.set_utxos(
        &treasury_addr,
        vec![funding(10_000_000_000, &treasury_script)],
    );

    // Intent is recorded, then the broadcast fails (process "crashes").
    mock.arm_submit_failure();
    let t = reload(&pool, payout_id).await;
    let err = advance_transfer(
        &pool,
        &mock,
        &secret,
        &treasury_addr,
        &fee_rate(),
        &t,
        ExecutionMode::Live,
    )
    .await
    .expect_err("submit failure surfaces");
    assert!(matches!(err, payout_krc20::Krc20ExecuteError::Kaspad(_)));

    // But the row already advanced with the deterministic hash, and nothing
    // reached the wire.
    assert_eq!(
        reload(&pool, payout_id).await.status,
        Krc20TransferStatus::CommitSubmitted
    );
    let recorded = recorded_commit(&pool, payout_id).await;
    assert!(
        mock.submitted_ids().is_empty(),
        "crash left the wire untouched"
    );

    // Resume: commit is neither on chain nor in mempool ⇒ re-broadcast the
    // *same* commit (same inputs ⇒ same txid), never a second distinct spend.
    let t = reload(&pool, payout_id).await;
    let step = advance_transfer(
        &pool,
        &mock,
        &secret,
        &treasury_addr,
        &fee_rate(),
        &t,
        ExecutionMode::Live,
    )
    .await
    .expect("resume rebroadcast");
    assert_eq!(step, TransferStep::CommitRebroadcast);
    assert_eq!(
        mock.submitted_ids(),
        vec![recorded.as_bytes()],
        "exactly one distinct commit, equal to the recorded txid"
    );
}

#[tokio::test]
async fn utxo_drift_refuses_to_broadcast_a_divergent_commit() {
    let (pool, _ctr) = fresh_pool().await;
    let (payout_id, _recipient) = seed_transfer(&pool).await;
    let (secret, treasury_addr) = treasury();
    let treasury_script = pay_to_address_script(&treasury_addr);

    let mock = MockKaspad::default();
    mock.set_daa(1_000);
    mock.set_utxos(
        &treasury_addr,
        vec![funding(10_000_000_000, &treasury_script)],
    );

    // Commit recorded + broadcast.
    let t = reload(&pool, payout_id).await;
    advance_transfer(
        &pool,
        &mock,
        &secret,
        &treasury_addr,
        &fee_rate(),
        &t,
        ExecutionMode::Live,
    )
    .await
    .expect("commit");

    // The commit drops from the mempool and the treasury UTXO set changes out
    // from under us (a different coin) before the P2SH output ever appears.
    mock.clear_mempool();
    let mut drifted = funding(10_000_000_000, &treasury_script);
    drifted.outpoint.transaction_id = TransactionId::from_bytes([8_u8; 32]);
    mock.set_utxos(&treasury_addr, vec![drifted]);

    let t = reload(&pool, payout_id).await;
    let err = advance_transfer(
        &pool,
        &mock,
        &secret,
        &treasury_addr,
        &fee_rate(),
        &t,
        ExecutionMode::Live,
    )
    .await
    .expect_err("drift is refused");
    assert!(matches!(
        err,
        payout_krc20::Krc20ExecuteError::CommitDrift { .. }
    ));
    assert_eq!(
        mock.submitted_ids().len(),
        1,
        "no divergent commit broadcast"
    );
}

#[tokio::test]
async fn dry_run_plans_but_records_nothing_and_broadcasts_nothing() {
    let (pool, _ctr) = fresh_pool().await;
    let (payout_id, _recipient) = seed_transfer(&pool).await;
    let (secret, treasury_addr) = treasury();
    let treasury_script = pay_to_address_script(&treasury_addr);

    let mock = MockKaspad::default();
    mock.set_daa(1_000);
    mock.set_utxos(
        &treasury_addr,
        vec![funding(10_000_000_000, &treasury_script)],
    );

    let t = reload(&pool, payout_id).await;
    let step = advance_transfer(
        &pool,
        &mock,
        &secret,
        &treasury_addr,
        &fee_rate(),
        &t,
        ExecutionMode::DryRun,
    )
    .await
    .expect("dry run");
    assert_eq!(step, TransferStep::NoChange);

    let row = payout::get_payout(&pool, payout_id).await.expect("payout");
    assert!(row.krc20_commit_hash.is_none(), "dry-run records no hash");
    assert_eq!(
        reload(&pool, payout_id).await.status,
        Krc20TransferStatus::Pending
    );
    assert!(
        mock.submitted_ids().is_empty(),
        "dry-run broadcasts nothing"
    );
}

/// Seed a wallet + KRC-20 cycle + payout + pending transfer for an arbitrary
/// recipient key, in a caller-chosen DAA window (so two transfers can live in
/// distinct cycles yet be swept together).
async fn seed_transfer_for(
    pool: &sqlx::PgPool,
    recipient_hex: &str,
    daa_start: u64,
    daa_end: u64,
) -> (i64, Address) {
    let recipient = mainnet_address(recipient_hex);
    let wallet_addr = WalletAddress::new(recipient.to_string()).expect("wallet addr");
    let w = wallet::ensure(pool, &wallet_addr, "mainnet")
        .await
        .expect("wallet");
    let cycle = payout::create_cycle(
        pool,
        PayoutKind::Krc20Nacho,
        DaaScore::new(daa_start),
        DaaScore::new(daa_end),
    )
    .await
    .expect("cycle");
    let p = payout::insert_payout(pool, cycle.id, w.id, 500_000_000)
        .await
        .expect("payout");
    let transfer = Krc20Transfer::new("NACHO", NACHO_AMOUNT.to_string(), recipient.to_string());
    let redeem = build_transfer_inscription(&xonly(TREASURY_HEX), &transfer).expect("redeem");
    let p2sh = commit_address(&redeem, Prefix::Mainnet).expect("p2sh addr");
    payout::insert_krc20_pending(
        pool,
        p.id,
        i64::try_from(DEFAULT_COMMIT_AMOUNT_SOMPI).expect("fits"),
        NACHO_AMOUNT,
        &p2sh.to_string(),
    )
    .await
    .expect("pending");
    (p.id, recipient)
}

/// Two pending transfers swept together must fund from *disjoint* coins even
/// when only one treasury UTXO exists: the second commit chains off the first
/// commit's change output instead of re-selecting the same (still-confirmed)
/// coin, which kaspad would reject as a double-spend.
#[tokio::test]
async fn sweep_chains_sibling_commits_off_a_single_coin() {
    let (pool, _ctr) = fresh_pool().await;
    let (payout_a, _ra) = seed_transfer_for(&pool, RECIPIENT_HEX, 1_000, 2_000).await;
    let (payout_b, _rb) = seed_transfer_for(&pool, RECIPIENT2_HEX, 3_000, 4_000).await;
    let (secret, treasury_addr) = treasury();
    let treasury_script = pay_to_address_script(&treasury_addr);

    // A single dominant treasury coin funds the whole sweep.
    let mock = MockKaspad::default();
    mock.set_daa(1_000);
    mock.set_utxos(
        &treasury_addr,
        vec![funding(10_000_000_000, &treasury_script)],
    );

    let report = settle_pending(
        &pool,
        &mock,
        &secret,
        &treasury_addr,
        100,
        ExecutionMode::Live,
    )
    .await
    .expect("sweep");

    assert_eq!(report.commits_broadcast, 2, "both commits broadcast");
    assert!(report.errors.is_empty(), "no double-spend rejection");

    // Submission order follows transfer id, so [0] is the first commit.
    let submitted = mock.submitted.lock().unwrap().clone();
    assert_eq!(submitted.len(), 2, "exactly the two commits hit the wire");
    let commit_a = &submitted[0];
    let commit_b = &submitted[1];

    let a_inputs: HashSet<TransactionOutpoint> = commit_a
        .inputs
        .iter()
        .map(|i| i.previous_outpoint)
        .collect();
    let b_inputs: HashSet<TransactionOutpoint> = commit_b
        .inputs
        .iter()
        .map(|i| i.previous_outpoint)
        .collect();
    assert!(
        a_inputs.is_disjoint(&b_inputs),
        "sibling commits must not share an input (no double-spend)"
    );

    // The first commit consumes the original coin and emits change at index 1;
    // the second commit spends exactly that change output.
    let original = TransactionOutpoint {
        transaction_id: TransactionId::from_bytes([7_u8; 32]),
        index: 0,
    };
    assert!(a_inputs.contains(&original), "first commit spends the coin");
    let chained = TransactionOutpoint {
        transaction_id: commit_a.id(),
        index: 1,
    };
    assert!(
        b_inputs.contains(&chained),
        "second commit chains off the first commit's change output"
    );

    let _ = (payout_a, payout_b);
}
