//! Orchestration tests (M4.6): drive a planned cycle through sign → submit →
//! confirm against an in-memory mock kaspad and a real testcontainer Postgres.
//!
//! These pin the crash-safe, idempotent contract deterministically: rows are
//! marked `submitted` with the on-chain txid before broadcast, a re-run never
//! re-pays, and confirmation only advances on a positive chain signal.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::too_many_lines
)]

use std::collections::HashSet;
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use kaspa_addresses::{Address, Prefix, Version};
use kaspa_consensus_core::tx::{Transaction, TransactionId, TransactionOutpoint, UtxoEntry};
use kaspa_txscript::pay_to_address_script;
use katpool_db::repo::payout::{self, PayoutCycleStatus, PayoutStatus};
use katpool_db::repo::{block, coinbase_reward, share_allocation, wallet, worker};
use katpool_db::{PoolConfig, build_pool, migrate};
use katpool_domain::{BlockHash, CorrelationId, DaaScore, WalletAddress, WorkerName};
use katpool_secrets::{TreasurySecret, from_hex};
use payout_kas::{
    DEFAULT_KAS_PAYOUT_THRESHOLD_SOMPI, ExecutionMode, KaspadClient, KaspadError,
    PlanKasCycleParams, TreasuryUtxoSnapshot, broadcast_cycle, confirm_cycle,
    reconcile_cycle_status, resume_or_plan_kas_cycle,
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
        application_name: "payout-kas-execute-test".to_owned(),
    };
    let pool = build_pool(&cfg).await.expect("pool");
    migrate::run(&pool).await.expect("migrate");
    (pool, container)
}

fn keypair(hex: &str) -> Keypair {
    let secret = from_hex(hex).expect("valid key");
    Keypair::from_seckey_slice(secp256k1::SECP256K1, secret.expose_secret()).expect("kp")
}

/// A valid, checksummed mainnet address derived from a known key, so it
/// round-trips through `kaspa_addresses::Address::try_from` in the executor.
fn mainnet_address_string(hex: &str) -> String {
    Address::new(
        Prefix::Mainnet,
        Version::PubKey,
        &keypair(hex).x_only_public_key().0.serialize(),
    )
    .to_string()
}

async fn seed_two_wallet_allocations(
    pool: &sqlx::PgPool,
) -> (katpool_db::repo::WalletId, katpool_db::repo::WalletId) {
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
    (w1.id, w2.id)
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

// ---- mock kaspad ----------------------------------------------------

#[derive(Default)]
struct MockKaspad {
    virtual_daa: Mutex<u64>,
    utxos: Mutex<Vec<TreasuryUtxoSnapshot>>,
    submitted: Mutex<Vec<Transaction>>,
    mempool: Mutex<HashSet<[u8; 32]>>,
}

impl MockKaspad {
    fn set_virtual_daa(&self, daa: u64) {
        *self.virtual_daa.lock().unwrap() = daa;
    }
    fn set_utxos(&self, utxos: Vec<TreasuryUtxoSnapshot>) {
        *self.utxos.lock().unwrap() = utxos;
    }
    fn submitted_count(&self) -> usize {
        self.submitted.lock().unwrap().len()
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

fn funding(
    amount: u64,
    script: &kaspa_consensus_core::tx::ScriptPublicKey,
) -> TreasuryUtxoSnapshot {
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

fn change_for(
    txid_bytes: [u8; 32],
    daa: u64,
    script: &kaspa_consensus_core::tx::ScriptPublicKey,
) -> TreasuryUtxoSnapshot {
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

// ---- tests ----------------------------------------------------------

#[tokio::test]
async fn full_lifecycle_sign_submit_confirm_settles_and_is_idempotent() {
    let (pool, _ctr) = fresh_pool().await;
    seed_two_wallet_allocations(&pool).await;
    let (secret, treasury_addr) = treasury();
    let treasury_script = pay_to_address_script(&treasury_addr);

    let mock = MockKaspad::default();
    mock.set_virtual_daa(1_000);
    mock.set_utxos(vec![funding(10_000_000_000, &treasury_script)]);

    let params = PlanKasCycleParams {
        daa_start: DaaScore::new(1_000),
        daa_end: DaaScore::new(2_000),
        threshold_sompi: DEFAULT_KAS_PAYOUT_THRESHOLD_SOMPI,
    };

    // --- broadcast: one batch funds both recipients ---
    let state = resume_or_plan_kas_cycle(&pool, params).await.expect("plan");
    let report = broadcast_cycle(
        &pool,
        &mock,
        &secret,
        &treasury_addr,
        &state,
        ExecutionMode::Live,
    )
    .await
    .expect("broadcast");
    assert_eq!(report.planned_batches, 1);
    assert_eq!(
        report.submitted_payouts, 2,
        "both rows recorded before broadcast"
    );
    assert_eq!(report.submitted_txids.len(), 1);
    assert!(report.submit_errors.is_empty());
    assert_eq!(mock.submitted_count(), 1, "exactly one tx hit the wire");

    // Rows are submitted with the deterministic txid.
    let after = resume_or_plan_kas_cycle(&pool, params)
        .await
        .expect("reload");
    assert!(
        after
            .payouts
            .iter()
            .all(|p| p.status == PayoutStatus::Submitted)
    );
    let txid_bytes: [u8; 32] = after.payouts[0]
        .tx_hash
        .clone()
        .expect("tx_hash")
        .try_into()
        .expect("32 bytes");
    assert!(
        after
            .payouts
            .iter()
            .all(|p| p.tx_hash.as_deref() == Some(txid_bytes.as_slice())),
        "all payouts in the batch share one txid"
    );

    // --- idempotent re-run: nothing pending, no new tx ---
    let rerun = broadcast_cycle(
        &pool,
        &mock,
        &secret,
        &treasury_addr,
        &after,
        ExecutionMode::Live,
    )
    .await
    .expect("rerun");
    assert_eq!(rerun.submitted_payouts, 0, "no planned rows remain");
    assert_eq!(mock.submitted_count(), 1, "no double broadcast");

    // --- confirm pass 1: still only in mempool ⇒ pending ---
    let r1 = confirm_cycle(&pool, &mock, &treasury_addr, &after)
        .await
        .expect("confirm pending");
    assert_eq!(r1.pending, 2);
    assert_eq!(r1.accepted, 0);

    // --- confirm pass 2: change coin on chain below depth ⇒ accepted ---
    mock.set_utxos(vec![change_for(txid_bytes, 1_000, &treasury_script)]);
    mock.set_virtual_daa(1_000);
    let state = resume_or_plan_kas_cycle(&pool, params)
        .await
        .expect("reload");
    let r2 = confirm_cycle(&pool, &mock, &treasury_addr, &state)
        .await
        .expect("confirm accepted");
    assert_eq!(r2.accepted, 2);
    let state = resume_or_plan_kas_cycle(&pool, params)
        .await
        .expect("reload");
    assert!(
        state
            .payouts
            .iter()
            .all(|p| p.status == PayoutStatus::Accepted)
    );

    // --- confirm pass 3: matured past depth ⇒ confirmed ---
    mock.set_virtual_daa(1_000 + payout_kas::KAS_PAYOUT_CONFIRMATION_DAA);
    let r3 = confirm_cycle(&pool, &mock, &treasury_addr, &state)
        .await
        .expect("confirm confirmed");
    assert_eq!(r3.confirmed, 2);

    // --- reconcile: cycle settles ---
    let derived = reconcile_cycle_status(&pool, state.cycle.id)
        .await
        .expect("reconcile");
    assert_eq!(derived, PayoutCycleStatus::Settled);
    let settled = payout::get_cycle(&pool, state.cycle.id)
        .await
        .expect("reload cycle");
    assert_eq!(settled.status, PayoutCycleStatus::Settled);

    // Confirmed payouts zero out payable balance ⇒ no longer eligible.
    let eligible = payout::list_kas_eligible_wallets(&pool, DEFAULT_KAS_PAYOUT_THRESHOLD_SOMPI)
        .await
        .expect("eligible");
    assert!(eligible.is_empty(), "all balances paid: {eligible:?}");
}

#[tokio::test]
async fn dry_run_signs_but_neither_records_nor_broadcasts() {
    let (pool, _ctr) = fresh_pool().await;
    seed_two_wallet_allocations(&pool).await;
    let (secret, treasury_addr) = treasury();
    let treasury_script = pay_to_address_script(&treasury_addr);

    let mock = MockKaspad::default();
    mock.set_virtual_daa(1_000);
    mock.set_utxos(vec![funding(10_000_000_000, &treasury_script)]);

    let params = PlanKasCycleParams {
        daa_start: DaaScore::new(3_000),
        daa_end: DaaScore::new(4_000),
        threshold_sompi: DEFAULT_KAS_PAYOUT_THRESHOLD_SOMPI,
    };
    let state = resume_or_plan_kas_cycle(&pool, params).await.expect("plan");

    let report = broadcast_cycle(
        &pool,
        &mock,
        &secret,
        &treasury_addr,
        &state,
        ExecutionMode::DryRun,
    )
    .await
    .expect("dry run");
    assert_eq!(report.planned_batches, 1);
    assert_eq!(
        report.submitted_txids.len(),
        1,
        "txid computed for rehearsal"
    );
    assert_eq!(report.submitted_payouts, 0, "dry-run records nothing");
    assert_eq!(mock.submitted_count(), 0, "dry-run broadcasts nothing");

    let reloaded = resume_or_plan_kas_cycle(&pool, params)
        .await
        .expect("reload");
    assert!(
        reloaded
            .payouts
            .iter()
            .all(|p| p.status == PayoutStatus::Planned),
        "rows untouched by dry-run"
    );
    assert_eq!(reloaded.pending().len(), 2);
}
