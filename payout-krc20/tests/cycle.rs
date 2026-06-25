//! Cycle state-machine tests (M5.5a): plan → resume → credit → fail →
//! reconcile for the KRC-20 NACHO payout cycle, against a real testcontainer
//! Postgres and a fixed (mock) floor-price source. No kaspad — this layer is
//! database-only.
//!
//! These pin the cross-cycle safety contract deterministically: planning
//! gates dust and freezes on resume, crediting a confirmed reveal is
//! exactly-once, a failed transfer refunds its balance for a later cycle, and
//! the cycle status folds from the transfer states.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::time::Duration;

use async_trait::async_trait;
use kaspa_addresses::{Address, Prefix, Version};
use katpool_db::repo::WalletId;
use katpool_db::repo::payout::{self, Krc20TransferStatus, PayoutCycleStatus, PayoutStatus};
use katpool_db::repo::{nacho_rebate, wallet};
use katpool_db::{PoolConfig, build_pool, migrate};
use katpool_domain::{DaaScore, WalletAddress};
use katpool_secrets::from_hex;
use payout_krc20::{
    DEFAULT_COMMIT_AMOUNT_SOMPI, FloorPrice, FloorPriceSource, Krc20CycleParams, Krc20Transfer,
    QuoteError, build_transfer_inscription, commit_address, credit_completed_transfers,
    fail_krc20_transfer, reconcile_krc20_cycle_status, resume_or_plan_krc20_cycle,
};
use secp256k1::Keypair;
use testcontainers::ContainerAsync;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;

const TREASURY_HEX: &str = "1111111111111111111111111111111111111111111111111111111111111111";
const WALLET_A_HEX: &str = "2222222222222222222222222222222222222222222222222222222222222222";
const WALLET_B_HEX: &str = "3333333333333333333333333333333333333333333333333333333333333333";

const DUST_GATE: u128 = 100_000_000;
const PENDING_A: i64 = 200_000_000;
const PENDING_B: i64 = 50_000_000;

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
        application_name: "payout-krc20-cycle-test".to_owned(),
    };
    let pool = build_pool(&cfg).await.expect("pool");
    migrate::run(&pool).await.expect("migrate");
    (pool, container)
}

fn xonly(hex: &str) -> [u8; 32] {
    let secret = from_hex(hex).expect("key");
    Keypair::from_seckey_slice(secp256k1::SECP256K1, secret.expose_secret())
        .expect("kp")
        .x_only_public_key()
        .0
        .serialize()
}

fn address(hex: &str) -> Address {
    Address::new(Prefix::Mainnet, Version::PubKey, &xonly(hex))
}

async fn seed_wallet(pool: &sqlx::PgPool, hex: &str, accrued: i64) -> (WalletId, Address) {
    let addr = address(hex);
    let wallet_addr = WalletAddress::new(addr.to_string()).expect("wallet addr");
    let w = wallet::ensure(pool, &wallet_addr, "mainnet")
        .await
        .expect("wallet");
    nacho_rebate::accrue(pool, w.id, accrued)
        .await
        .expect("accrue");
    (w.id, addr)
}

/// A floor-price source that always returns a fixed price; `"1"` makes the
/// conversion an identity (`nacho_base_units == pending_sompi`) for
/// easy-to-reason assertions.
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

fn params(daa_start: u64, daa_end: u64, min_nacho_base_units: u128) -> Krc20CycleParams {
    Krc20CycleParams {
        daa_start: DaaScore::new(daa_start),
        daa_end: DaaScore::new(daa_end),
        min_pending_sompi: 1,
        min_nacho_base_units,
        ticker: "NACHO".to_owned(),
        commit_amount_sompi: DEFAULT_COMMIT_AMOUNT_SOMPI,
        limit: 100,
    }
}

fn treasury_xonly() -> [u8; 32] {
    xonly(TREASURY_HEX)
}

async fn complete_transfer(pool: &sqlx::PgPool, payout_id: i64) {
    payout::mark_krc20_commit_submitted(pool, payout_id)
        .await
        .expect("commit");
    payout::mark_krc20_reveal_submitted(pool, payout_id)
        .await
        .expect("reveal");
    payout::mark_krc20_completed(pool, payout_id)
        .await
        .expect("complete");
}

// ---- tests ----------------------------------------------------------

#[tokio::test]
async fn plan_selects_payable_and_gates_dust() {
    let (pool, _ctr) = fresh_pool().await;
    let (_a, addr_a) = seed_wallet(&pool, WALLET_A_HEX, PENDING_A).await;
    seed_wallet(&pool, WALLET_B_HEX, PENDING_B).await;

    let state = resume_or_plan_krc20_cycle(
        &pool,
        &identity_quote(),
        &treasury_xonly(),
        Prefix::Mainnet,
        &params(1_000, 2_000, DUST_GATE),
    )
    .await
    .expect("plan");

    // B (pending 50M → 5e7 NACHO) is below the dust gate; only A is planned.
    assert_eq!(state.transfers.len(), 1);
    let t = &state.transfers[0];
    assert_eq!(
        t.nacho_amount, PENDING_A,
        "identity price ⇒ nacho == pending"
    );
    assert_eq!(
        t.sompi_to_miner,
        i64::try_from(DEFAULT_COMMIT_AMOUNT_SOMPI).unwrap()
    );
    assert_eq!(t.status, Krc20TransferStatus::Pending);

    // P2SH binds the treasury key to A's inscription.
    let transfer = Krc20Transfer::new("NACHO", PENDING_A.to_string(), addr_a.to_string());
    let redeem = build_transfer_inscription(&treasury_xonly(), &transfer).unwrap();
    let expected_p2sh = commit_address(&redeem, Prefix::Mainnet)
        .unwrap()
        .to_string();
    assert_eq!(t.p2sh_address, expected_p2sh);

    assert_eq!(state.cycle.total_recipients, 1);
    assert_eq!(state.cycle.total_sompi, PENDING_A);
    assert_eq!(state.derived_status(), PayoutCycleStatus::Planned);
}

#[tokio::test]
async fn resume_freezes_recipients_and_amounts() {
    let (pool, _ctr) = fresh_pool().await;
    let (a, _addr_a) = seed_wallet(&pool, WALLET_A_HEX, PENDING_A).await;

    let first = resume_or_plan_krc20_cycle(
        &pool,
        &identity_quote(),
        &treasury_xonly(),
        Prefix::Mainnet,
        &params(1_000, 2_000, DUST_GATE),
    )
    .await
    .expect("plan");
    assert_eq!(first.transfers.len(), 1);
    assert_eq!(first.transfers[0].nacho_amount, PENDING_A);

    // More accrues mid-cycle; resume must not re-price or add recipients.
    nacho_rebate::accrue(&pool, a, 500_000_000)
        .await
        .expect("accrue more");
    let resumed = resume_or_plan_krc20_cycle(
        &pool,
        &identity_quote(),
        &treasury_xonly(),
        Prefix::Mainnet,
        &params(1_000, 2_000, DUST_GATE),
    )
    .await
    .expect("resume");

    assert_eq!(resumed.cycle.id, first.cycle.id);
    assert_eq!(resumed.transfers.len(), 1);
    assert_eq!(
        resumed.transfers[0].nacho_amount, PENDING_A,
        "frozen at plan-time amount"
    );
}

#[tokio::test]
async fn credit_is_exactly_once() {
    let (pool, _ctr) = fresh_pool().await;
    let (a, _addr_a) = seed_wallet(&pool, WALLET_A_HEX, PENDING_A).await;

    let state = resume_or_plan_krc20_cycle(
        &pool,
        &identity_quote(),
        &treasury_xonly(),
        Prefix::Mainnet,
        &params(1_000, 2_000, DUST_GATE),
    )
    .await
    .expect("plan");
    let payout_id = state.transfers[0].payout_id;
    complete_transfer(&pool, payout_id).await;

    let first = credit_completed_transfers(&pool, 100)
        .await
        .expect("credit");
    assert_eq!(first.credited, 1);
    assert_eq!(first.paid_sompi, PENDING_A);
    assert_eq!(first.already_credited, 0);

    let rebate = nacho_rebate::get(&pool, a).await.expect("get").unwrap();
    assert_eq!(rebate.paid_sompi, PENDING_A);
    assert_eq!(rebate.pending_sompi(), 0);
    let payout_row = payout::get_payout(&pool, payout_id).await.expect("payout");
    assert_eq!(payout_row.status, PayoutStatus::Confirmed);

    // Re-run credits nothing.
    let second = credit_completed_transfers(&pool, 100)
        .await
        .expect("re-credit");
    assert_eq!(second.credited, 0);
    assert_eq!(second.already_credited, 1);
    let rebate = nacho_rebate::get(&pool, a).await.expect("get").unwrap();
    assert_eq!(rebate.paid_sompi, PENDING_A, "no double credit");

    // The wallet is no longer eligible (balance fully paid).
    let eligible = payout::list_krc20_eligible_wallets(&pool, 1, 100)
        .await
        .expect("eligible");
    assert!(eligible.iter().all(|w| w.wallet_id != a));
}

#[tokio::test]
async fn in_flight_is_unselectable_and_failure_refunds() {
    let (pool, _ctr) = fresh_pool().await;
    let (a, _addr_a) = seed_wallet(&pool, WALLET_A_HEX, PENDING_A).await;

    let state = resume_or_plan_krc20_cycle(
        &pool,
        &identity_quote(),
        &treasury_xonly(),
        Prefix::Mainnet,
        &params(1_000, 2_000, DUST_GATE),
    )
    .await
    .expect("plan");
    let payout_id = state.transfers[0].payout_id;

    // While the payout is planned/in-flight the balance is netted out.
    payout::mark_krc20_commit_submitted(&pool, payout_id)
        .await
        .expect("commit");
    let eligible = payout::list_krc20_eligible_wallets(&pool, 1, 100)
        .await
        .expect("eligible (in flight)");
    assert!(
        eligible.iter().all(|w| w.wallet_id != a),
        "in-flight balance must not be re-selectable"
    );

    // Terminal failure refunds the balance for a future cycle.
    fail_krc20_transfer(&pool, payout_id, "test giving up")
        .await
        .expect("fail");
    let eligible = payout::list_krc20_eligible_wallets(&pool, 1, 100)
        .await
        .expect("eligible (after fail)");
    let refunded = eligible
        .iter()
        .find(|w| w.wallet_id == a)
        .expect("refunded wallet present");
    assert_eq!(refunded.pending_sompi, PENDING_A, "full balance refunded");

    assert_eq!(
        reconcile_krc20_cycle_status(&pool, state.cycle.id)
            .await
            .expect("reconcile"),
        PayoutCycleStatus::Failed
    );
}

#[tokio::test]
async fn reconcile_folds_transfer_states_into_cycle_status() {
    let (pool, _ctr) = fresh_pool().await;
    let (_a, _) = seed_wallet(&pool, WALLET_A_HEX, PENDING_A).await;
    let (_b, _) = seed_wallet(&pool, WALLET_B_HEX, PENDING_A).await;

    let state = resume_or_plan_krc20_cycle(
        &pool,
        &identity_quote(),
        &treasury_xonly(),
        Prefix::Mainnet,
        &params(1_000, 2_000, DUST_GATE),
    )
    .await
    .expect("plan");
    assert_eq!(state.transfers.len(), 2);
    let cycle_id = state.cycle.id;

    assert_eq!(
        reconcile_krc20_cycle_status(&pool, cycle_id)
            .await
            .expect("reconcile planned"),
        PayoutCycleStatus::Planned
    );

    // One completes, one still pending ⇒ partially settled.
    complete_transfer(&pool, state.transfers[0].payout_id).await;
    assert_eq!(
        reconcile_krc20_cycle_status(&pool, cycle_id)
            .await
            .expect("reconcile partial"),
        PayoutCycleStatus::PartiallySettled
    );

    // Both complete ⇒ settled.
    complete_transfer(&pool, state.transfers[1].payout_id).await;
    assert_eq!(
        reconcile_krc20_cycle_status(&pool, cycle_id)
            .await
            .expect("reconcile settled"),
        PayoutCycleStatus::Settled
    );
}
