//! Restart-safe KRC-20 NACHO payout cycle state machine (database only).
//!
//! Mirrors the KAS cycle state machine ([`payout_kas`]'s `cycle`), adapted to
//! the NACHO rebate model:
//!
//! - **Plan.** [`plan_krc20_cycle`] quotes the floor price once, selects
//!   eligible wallets (`payout::list_krc20_eligible_wallets`), converts each
//!   pending KAS-sompi balance to NACHO base units at that price (M5.2,
//!   ADR-0016), applies the dust gate, and writes one `payout` +
//!   `krc20_pending_transfer` row per payable recipient — the P2SH commit
//!   address bound to the treasury key and the recipient inscription (M5.1).
//!   All in one transaction; idempotent via the cycle idempotency key and the
//!   one-to-one `ensure_krc20_pending` upsert.
//! - **Resume.** [`resume_or_plan_krc20_cycle`] loads an existing cycle for a
//!   DAA window **without recomputing**, so a cycle's recipient set and
//!   amounts never shift under an in-flight commit/reveal.
//! - **Credit.** [`credit_completed_transfers`] turns a confirmed reveal into
//!   a rebate payment: it confirms the payout and increments
//!   `nacho_rebate.paid_sompi` atomically, **exactly once**
//!   (`confirm_krc20_payout_once` is the guard). Eligibility nets out
//!   non-terminal payouts, so a credited (or in-flight) balance is never
//!   re-selected; a failed transfer's balance is refunded for a later cycle.
//! - **Reconcile.** [`reconcile_krc20_cycle_status`] folds the transfer
//!   statuses into the cycle status via the shared pure
//!   [`payout_kas::derive_cycle_status`].

use kaspa_addresses::Prefix;
use katpool_db::DbError;
use katpool_db::repo::audit::{self, NewEntry};
use katpool_db::repo::nacho_rebate;
use katpool_db::repo::payout::{
    self, Krc20PendingTransfer, Krc20TransferStatus, PayoutCycle, PayoutCycleStatus, PayoutKind,
};
use katpool_domain::DaaScore;
use payout_kas::{PayoutStatusCounts, derive_cycle_status};
use serde_json::json;
use sqlx::PgPool;

use crate::inscription::{
    InscriptionError, Krc20Transfer, build_transfer_inscription, commit_address,
};
use crate::quote::{DEFAULT_QUOTE_TICKER, FloorPriceSource, QuoteError};
use crate::rebate::{
    DEFAULT_MIN_NACHO_BASE_UNITS, DEFAULT_MIN_PENDING_SOMPI, RebateError, is_payable,
    nacho_base_units,
};

/// Audit-log actor for every KRC-20 cycle-lifecycle entry this crate writes.
const AUDIT_ACTOR: &str = "payout-krc20";

/// Default cap on recipients selected per cycle.
pub const DEFAULT_CYCLE_LIMIT: i64 = 1_000;

/// Errors from the KRC-20 cycle state machine.
#[derive(Debug, thiserror::Error)]
pub enum Krc20CycleError {
    /// Database failure.
    #[error(transparent)]
    Db(#[from] DbError),

    /// Floor-price quote failure (incl. a fail-closed circuit breaker).
    #[error(transparent)]
    Quote(#[from] QuoteError),

    /// KAS-sompi → NACHO conversion failure.
    #[error(transparent)]
    Rebate(#[from] RebateError),

    /// Inscription / P2SH-address derivation failure.
    #[error(transparent)]
    Inscription(#[from] InscriptionError),

    /// A converted NACHO amount does not fit the `BIGINT` column.
    #[error("nacho amount {nacho} for wallet {wallet_id} exceeds i64")]
    AmountTooLarge {
        /// Offending wallet id.
        wallet_id: i64,
        /// The over-large NACHO base-unit amount.
        nacho: u128,
    },

    /// The configured commit lock amount does not fit the `BIGINT` column.
    #[error("commit amount {0} sompi exceeds i64")]
    CommitAmount(u64),
}

/// Parameters for planning a KRC-20 NACHO payout cycle.
#[derive(Debug, Clone)]
pub struct Krc20CycleParams {
    /// Half-open DAA window start (inclusive) — the cycle identity.
    pub daa_start: DaaScore,
    /// Half-open DAA window end (exclusive).
    pub daa_end: DaaScore,
    /// Minimum pending KAS-sompi for a wallet to be selected (coarse filter).
    pub min_pending_sompi: i64,
    /// Minimum converted NACHO base units worth a reveal (dust gate).
    pub min_nacho_base_units: u128,
    /// Token ticker to quote and inscribe.
    pub ticker: String,
    /// KAS-sompi locked into each commit P2SH output (`sompi_to_miner`).
    pub commit_amount_sompi: u64,
    /// Maximum recipients per cycle.
    pub limit: i64,
}

impl Krc20CycleParams {
    /// Parameters for a DAA window with operator defaults from M5.2/M5.3.
    #[must_use]
    pub fn new(daa_start: DaaScore, daa_end: DaaScore) -> Self {
        Self {
            daa_start,
            daa_end,
            min_pending_sompi: DEFAULT_MIN_PENDING_SOMPI,
            min_nacho_base_units: DEFAULT_MIN_NACHO_BASE_UNITS,
            ticker: DEFAULT_QUOTE_TICKER.to_owned(),
            commit_amount_sompi: crate::plan::DEFAULT_COMMIT_AMOUNT_SOMPI,
            limit: DEFAULT_CYCLE_LIMIT,
        }
    }
}

/// In-memory snapshot of a KRC-20 cycle and its planned/in-flight transfers.
#[derive(Debug, Clone)]
pub struct Krc20CycleState {
    /// The cycle row.
    pub cycle: PayoutCycle,
    /// Every KRC-20 transfer in the cycle, oldest first.
    pub transfers: Vec<Krc20PendingTransfer>,
}

impl Krc20CycleState {
    /// Transfer status tally, mapped onto the shared cycle-status counts.
    #[must_use]
    pub fn counts(&self) -> PayoutStatusCounts {
        counts_from_transfers(&self.transfers)
    }

    /// Cycle status implied by the current transfer statuses (does not touch
    /// the database — see [`reconcile_krc20_cycle_status`] to persist).
    #[must_use]
    pub fn derived_status(&self) -> PayoutCycleStatus {
        derive_cycle_status(self.counts())
    }
}

/// Map KRC-20 transfer statuses onto the shared [`PayoutStatusCounts`]:
/// commit/reveal in flight → `in_flight`, `completed` → `confirmed`.
fn counts_from_transfers(transfers: &[Krc20PendingTransfer]) -> PayoutStatusCounts {
    let mut counts = PayoutStatusCounts {
        total: transfers.len(),
        ..PayoutStatusCounts::default()
    };
    for transfer in transfers {
        match transfer.status {
            Krc20TransferStatus::Pending => counts.pending += 1,
            Krc20TransferStatus::CommitSubmitted | Krc20TransferStatus::RevealSubmitted => {
                counts.in_flight += 1;
            }
            Krc20TransferStatus::Completed => counts.confirmed += 1,
            Krc20TransferStatus::Failed => counts.failed += 1,
        }
    }
    counts
}

/// Restart-safe entry point: resume an existing KRC-20 cycle or plan a new one.
///
/// An existing cycle for `(daa_start, daa_end)` is loaded as-is (amounts and
/// recipients frozen); otherwise a new cycle is planned via
/// [`plan_krc20_cycle`].
///
/// # Errors
///
/// See [`Krc20CycleError`].
pub async fn resume_or_plan_krc20_cycle<Q>(
    pool: &PgPool,
    quote: &Q,
    treasury_xonly: &[u8; 32],
    prefix: Prefix,
    params: &Krc20CycleParams,
) -> Result<Krc20CycleState, Krc20CycleError>
where
    Q: FloorPriceSource + ?Sized,
{
    let key = payout::idempotency_key(PayoutKind::Krc20Nacho, params.daa_start, params.daa_end);
    if let Some(cycle) = payout::find_cycle_by_idempotency_key(pool, &key).await? {
        let transfers = payout::list_krc20_for_cycle(pool, cycle.id).await?;
        audit::append(
            pool,
            NewEntry::new(AUDIT_ACTOR, "krc20_cycle.resume")
                .subject("payout_cycle", cycle.id)
                .payload(json!({
                    "idempotency_key": cycle.idempotency_key,
                    "status": format!("{:?}", cycle.status),
                    "transfers": transfers.len(),
                })),
        )
        .await?;
        return Ok(Krc20CycleState { cycle, transfers });
    }
    plan_krc20_cycle(pool, quote, treasury_xonly, prefix, params).await
}

/// Plan a new KRC-20 cycle: quote the floor price, select + convert eligible
/// wallets, and write the `payout` + `krc20_pending_transfer` rows.
///
/// The floor price is fetched **before** the transaction opens (the only
/// network call); everything else is one atomic, idempotent DB transaction.
///
/// # Errors
///
/// See [`Krc20CycleError`].
pub async fn plan_krc20_cycle<Q>(
    pool: &PgPool,
    quote: &Q,
    treasury_xonly: &[u8; 32],
    prefix: Prefix,
    params: &Krc20CycleParams,
) -> Result<Krc20CycleState, Krc20CycleError>
where
    Q: FloorPriceSource + ?Sized,
{
    let price = quote.floor_price(&params.ticker).await?;
    let commit_amount_sompi = i64::try_from(params.commit_amount_sompi)
        .map_err(|_| Krc20CycleError::CommitAmount(params.commit_amount_sompi))?;

    let mut tx = pool.begin().await.map_err(DbError::from)?;
    let cycle = payout::create_cycle(
        &mut *tx,
        PayoutKind::Krc20Nacho,
        params.daa_start,
        params.daa_end,
    )
    .await?;
    let eligible =
        payout::list_krc20_eligible_wallets(&mut *tx, params.min_pending_sompi, params.limit)
            .await?;

    let mut total_sompi = 0_i64;
    let mut recipients = 0_i32;
    let mut planned = 0_usize;
    for wallet in &eligible {
        let nacho = nacho_base_units(wallet.pending_sompi, &price)?;
        if !is_payable(nacho, params.min_nacho_base_units) {
            continue;
        }
        let nacho_amount = i64::try_from(nacho).map_err(|_| Krc20CycleError::AmountTooLarge {
            wallet_id: wallet.wallet_id.0,
            nacho,
        })?;

        let transfer = Krc20Transfer::new(
            params.ticker.clone(),
            nacho_amount.to_string(),
            wallet.address.clone(),
        );
        let redeem = build_transfer_inscription(treasury_xonly, &transfer)?;
        let p2sh = commit_address(&redeem, prefix)?;

        let payout =
            payout::ensure_payout(&mut *tx, cycle.id, wallet.wallet_id, wallet.pending_sompi)
                .await?;
        payout::ensure_krc20_pending(
            &mut *tx,
            payout.id,
            commit_amount_sompi,
            nacho_amount,
            &p2sh.to_string(),
        )
        .await?;

        total_sompi = total_sompi.saturating_add(wallet.pending_sompi);
        recipients = recipients.saturating_add(1);
        planned += 1;
    }

    payout::set_cycle_totals(&mut *tx, cycle.id, total_sompi, recipients).await?;
    audit::append(
        &mut *tx,
        NewEntry::new(AUDIT_ACTOR, "krc20_cycle.plan")
            .subject("payout_cycle", cycle.id)
            .payload(json!({
                "idempotency_key": cycle.idempotency_key,
                "eligible": eligible.len(),
                "planned": planned,
                "total_sompi": total_sompi,
                "floor_price_mantissa": price.mantissa().to_string(),
                "floor_price_scale": price.scale(),
            })),
    )
    .await?;
    tx.commit().await.map_err(DbError::from)?;

    let cycle = payout::get_cycle(pool, cycle.id).await?;
    let transfers = payout::list_krc20_for_cycle(pool, cycle.id).await?;
    Ok(Krc20CycleState { cycle, transfers })
}

/// Outcome of a [`credit_completed_transfers`] pass.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CreditReport {
    /// Transfers credited to `nacho_rebate.paid_sompi` this pass.
    pub credited: usize,
    /// Completed transfers whose payout was already confirmed (no-op).
    pub already_credited: usize,
    /// Total KAS-sompi credited this pass.
    pub paid_sompi: i64,
}

/// Credit every `completed` transfer whose payout is not yet confirmed:
/// confirm the payout and increment `nacho_rebate.paid_sompi` in one
/// transaction, **exactly once** per transfer.
///
/// # Errors
///
/// See [`Krc20CycleError`]. A `paid > accrued` CHECK violation surfaces as a
/// [`DbError`] rather than silently over-crediting.
pub async fn credit_completed_transfers(
    pool: &PgPool,
    limit: i64,
) -> Result<CreditReport, Krc20CycleError> {
    let completed =
        payout::list_krc20_by_status(pool, &[Krc20TransferStatus::Completed], limit).await?;

    let mut report = CreditReport::default();
    for transfer in &completed {
        let payout_row = payout::get_payout(pool, transfer.payout_id).await?;
        let mut tx = pool.begin().await.map_err(DbError::from)?;
        if payout::confirm_krc20_payout_once(&mut *tx, transfer.payout_id).await? {
            nacho_rebate::mark_paid(&mut *tx, payout_row.wallet_id, payout_row.amount_sompi)
                .await?;
            audit::append(
                &mut *tx,
                NewEntry::new(AUDIT_ACTOR, "krc20.credit")
                    .subject("payout", transfer.payout_id)
                    .payload(json!({
                        "wallet_id": payout_row.wallet_id.0,
                        "paid_sompi": payout_row.amount_sompi,
                        "nacho_amount": transfer.nacho_amount,
                    })),
            )
            .await?;
            tx.commit().await.map_err(DbError::from)?;
            report.credited += 1;
            report.paid_sompi = report.paid_sompi.saturating_add(payout_row.amount_sompi);
        } else {
            tx.rollback().await.map_err(DbError::from)?;
            report.already_credited += 1;
        }
    }
    Ok(report)
}

/// Escalate a transfer to terminal failure: mark both the
/// `krc20_pending_transfer` and its parent `payout` failed in one
/// transaction.
///
/// Because eligibility nets out only non-terminal payouts, marking the payout
/// `failed` **refunds** its KAS-sompi balance back to the wallet's pending
/// total for a later cycle (it was never credited). The *policy* for when to
/// give up on a stuck transfer lives in the engine (M5.5b); this is the DB
/// primitive it calls.
///
/// # Errors
///
/// [`Krc20CycleError::Db`] on failure.
pub async fn fail_krc20_transfer(
    pool: &PgPool,
    payout_id: i64,
    reason: &str,
) -> Result<(), Krc20CycleError> {
    let mut tx = pool.begin().await.map_err(DbError::from)?;
    payout::mark_krc20_failed(&mut *tx, payout_id).await?;
    payout::mark_payout_failed(&mut *tx, payout_id, reason).await?;
    audit::append(
        &mut *tx,
        NewEntry::new(AUDIT_ACTOR, "krc20.fail")
            .subject("payout", payout_id)
            .payload(json!({ "reason": reason })),
    )
    .await?;
    tx.commit().await.map_err(DbError::from)?;
    Ok(())
}

/// Recompute the KRC-20 cycle status from its transfers and persist the
/// transition, writing a `krc20_cycle.reconcile` audit entry.
///
/// Idempotent. Returns the derived status.
///
/// # Errors
///
/// See [`Krc20CycleError`].
pub async fn reconcile_krc20_cycle_status(
    pool: &PgPool,
    cycle_id: i64,
) -> Result<PayoutCycleStatus, Krc20CycleError> {
    let transfers = payout::list_krc20_for_cycle(pool, cycle_id).await?;
    let counts = counts_from_transfers(&transfers);
    let derived = derive_cycle_status(counts);

    let mut tx = pool.begin().await.map_err(DbError::from)?;
    match derived {
        PayoutCycleStatus::Planned => {}
        PayoutCycleStatus::Broadcasting => {
            payout::mark_cycle_broadcasting(&mut *tx, cycle_id).await?;
        }
        PayoutCycleStatus::PartiallySettled => {
            payout::mark_cycle_broadcasting(&mut *tx, cycle_id).await?;
            payout::mark_cycle_partially_settled(&mut *tx, cycle_id).await?;
        }
        PayoutCycleStatus::Settled => {
            payout::mark_cycle_broadcasting(&mut *tx, cycle_id).await?;
            payout::mark_cycle_settled(&mut *tx, cycle_id).await?;
        }
        PayoutCycleStatus::Failed => {
            payout::mark_cycle_failed(&mut *tx, cycle_id).await?;
        }
    }
    audit::append(
        &mut *tx,
        NewEntry::new(AUDIT_ACTOR, "krc20_cycle.reconcile")
            .subject("payout_cycle", cycle_id)
            .payload(json!({
                "total": counts.total,
                "pending": counts.pending,
                "in_flight": counts.in_flight,
                "confirmed": counts.confirmed,
                "failed": counts.failed,
                "derived": format!("{derived:?}"),
            })),
    )
    .await?;
    tx.commit().await.map_err(DbError::from)?;

    Ok(derived)
}
