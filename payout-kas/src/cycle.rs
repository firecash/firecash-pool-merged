//! Restart-safe KAS payout cycle state machine (database only).
//!
//! The cycle's durability story rests entirely on natural database keys,
//! not a side-channel idempotency table:
//!
//! - `payout_cycle.idempotency_key` (`kas-<daa_start>-<daa_end>`, UNIQUE)
//!   identifies a cycle exactly once for a given DAA window.
//! - `payout UNIQUE (cycle_id, wallet_id)` records one planned row per
//!   recipient **before any transaction is signed** (M4.3 `ensure_payout`).
//!
//! Together these mean a crash at any point is recoverable by re-deriving
//! state from the rows already committed:
//!
//! 1. [`resume_or_plan_kas_cycle`] is the single entry point. On first call
//!    it plans the cycle; on every subsequent call (after a restart) it
//!    loads the existing cycle **without recomputing eligibility**, so an
//!    in-flight cycle's amounts and recipient set never shift underneath a
//!    partially-broadcast batch.
//! 2. [`CycleState::pending`] is the set of recipients still safe to sign —
//!    rows in `planned` status only. Anything `submitted`/`accepted`/
//!    `confirmed` is excluded, so a resumed cycle can never double-pay.
//! 3. [`reconcile_cycle_status`] folds per-recipient statuses into the
//!    cycle status via the pure [`derive_cycle_status`] function and
//!    persists the transition with an audit-log entry.

use katpool_db::DbError;
use katpool_db::repo::audit::{self, NewEntry};
use katpool_db::repo::payout::{
    self, Payout, PayoutCycle, PayoutCycleStatus, PayoutKind, PayoutStatus,
};
use serde_json::json;
use sqlx::PgPool;

use crate::error::PayoutKasError;
use crate::plan::{PlanKasCycleParams, plan_kas_cycle};

/// Audit-log actor for every cycle-lifecycle entry this crate writes.
const AUDIT_ACTOR: &str = "payout-kas";

/// Tally of recipient statuses within one cycle. Drives the pure
/// [`derive_cycle_status`] folding function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PayoutStatusCounts {
    /// Total payout rows in the cycle.
    pub total: usize,
    /// Rows in `planned` status (not yet signed/broadcast).
    pub pending: usize,
    /// Rows `submitted` or `accepted` (on the wire, not yet confirmed).
    pub in_flight: usize,
    /// Rows `confirmed` past the maturity window.
    pub confirmed: usize,
    /// Rows `failed`.
    pub failed: usize,
}

impl PayoutStatusCounts {
    /// Fold a recipient slice into status counts.
    #[must_use]
    pub fn from_payouts(payouts: &[Payout]) -> Self {
        let mut counts = Self {
            total: payouts.len(),
            ..Self::default()
        };
        for payout in payouts {
            match payout.status {
                PayoutStatus::Planned => counts.pending += 1,
                PayoutStatus::Submitted | PayoutStatus::Accepted => counts.in_flight += 1,
                PayoutStatus::Confirmed => counts.confirmed += 1,
                PayoutStatus::Failed => counts.failed += 1,
            }
        }
        counts
    }
}

/// Fold recipient status counts into the cycle status.
///
/// Pure and total — the single source of truth for cycle progression,
/// kept side-effect-free so it can be exhaustively unit/property tested:
///
/// - no recipients, or all still `planned` → `Planned`
/// - all `confirmed` → `Settled`
/// - all terminal (`confirmed` + `failed`) with ≥1 confirmed → `PartiallySettled`
/// - all terminal with 0 confirmed → `Failed`
/// - some confirmed, work remaining → `PartiallySettled`
/// - work started (in-flight or a failure) but nothing confirmed → `Broadcasting`
#[must_use]
pub const fn derive_cycle_status(counts: PayoutStatusCounts) -> PayoutCycleStatus {
    if counts.total == 0 {
        return PayoutCycleStatus::Planned;
    }
    if counts.confirmed == counts.total {
        return PayoutCycleStatus::Settled;
    }
    let all_terminal = counts.confirmed + counts.failed == counts.total;
    if all_terminal {
        // Not all confirmed (handled above) ⇒ at least one failure.
        return if counts.confirmed > 0 {
            PayoutCycleStatus::PartiallySettled
        } else {
            PayoutCycleStatus::Failed
        };
    }
    if counts.confirmed > 0 {
        return PayoutCycleStatus::PartiallySettled;
    }
    if counts.in_flight > 0 || counts.failed > 0 {
        return PayoutCycleStatus::Broadcasting;
    }
    PayoutCycleStatus::Planned
}

/// In-memory snapshot of a cycle and its recipients, loaded atomically
/// enough for restart-safe decision-making.
#[derive(Debug, Clone)]
pub struct CycleState {
    /// The cycle row.
    pub cycle: PayoutCycle,
    /// Every recipient row, ordered by amount then id (see
    /// [`payout::list_for_cycle`]).
    pub payouts: Vec<Payout>,
}

impl CycleState {
    /// Recipients still safe to sign and broadcast: `planned` only.
    ///
    /// Excludes anything already on the wire or settled, guaranteeing a
    /// resumed cycle never re-pays a recipient.
    #[must_use]
    pub fn pending(&self) -> Vec<&Payout> {
        self.payouts
            .iter()
            .filter(|p| p.status == PayoutStatus::Planned)
            .collect()
    }

    /// Recipient status tally.
    #[must_use]
    pub fn counts(&self) -> PayoutStatusCounts {
        PayoutStatusCounts::from_payouts(&self.payouts)
    }

    /// Cycle status implied by the current recipient statuses (does not
    /// touch the database — see [`reconcile_cycle_status`] to persist).
    #[must_use]
    pub fn derived_status(&self) -> PayoutCycleStatus {
        derive_cycle_status(self.counts())
    }
}

/// Restart-safe entry point: resume an existing KAS cycle or plan a new one.
///
/// If a cycle already exists for the `(daa_start, daa_end)` window it is
/// loaded as-is — eligibility is **not** recomputed, so amounts and the
/// recipient set stay frozen for the life of the cycle. Otherwise the cycle
/// is planned via [`plan_kas_cycle`]. Either way an audit-log entry records
/// the entry path.
pub async fn resume_or_plan_kas_cycle(
    pool: &PgPool,
    params: PlanKasCycleParams,
) -> Result<CycleState, PayoutKasError> {
    let key = payout::idempotency_key(PayoutKind::Kas, params.daa_start, params.daa_end);

    if let Some(cycle) = payout::find_cycle_by_idempotency_key(pool, &key).await? {
        let payouts = payout::list_for_cycle(pool, cycle.id).await?;
        audit::append(
            pool,
            NewEntry::new(AUDIT_ACTOR, "cycle.resume")
                .subject("payout_cycle", cycle.id)
                .payload(json!({
                    "idempotency_key": cycle.idempotency_key,
                    "status": format!("{:?}", cycle.status),
                    "recipients": payouts.len(),
                })),
        )
        .await?;
        return Ok(CycleState { cycle, payouts });
    }

    let planned = plan_kas_cycle(pool, params).await?;
    audit::append(
        pool,
        NewEntry::new(AUDIT_ACTOR, "cycle.plan")
            .subject("payout_cycle", planned.cycle.id)
            .payload(json!({
                "idempotency_key": planned.cycle.idempotency_key,
                "recipients": planned.payouts.len(),
                "total_sompi": planned.cycle.total_sompi,
            })),
    )
    .await?;
    Ok(CycleState {
        cycle: planned.cycle,
        payouts: planned.payouts,
    })
}

/// Recompute the cycle status from its recipients and persist the
/// transition, writing a `cycle.reconcile` audit entry.
///
/// Idempotent: re-running on an unchanged cycle re-applies the same
/// terminal status without side effects. Returns the derived status.
pub async fn reconcile_cycle_status(
    pool: &PgPool,
    cycle_id: i64,
) -> Result<PayoutCycleStatus, PayoutKasError> {
    let payouts = payout::list_for_cycle(pool, cycle_id).await?;
    let counts = PayoutStatusCounts::from_payouts(&payouts);
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
        NewEntry::new(AUDIT_ACTOR, "cycle.reconcile")
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

#[cfg(test)]
mod tests {
    use super::{PayoutStatusCounts, derive_cycle_status};
    use katpool_db::repo::payout::PayoutCycleStatus;

    fn counts(
        pending: usize,
        in_flight: usize,
        confirmed: usize,
        failed: usize,
    ) -> PayoutStatusCounts {
        PayoutStatusCounts {
            total: pending + in_flight + confirmed + failed,
            pending,
            in_flight,
            confirmed,
            failed,
        }
    }

    #[test]
    fn empty_cycle_is_planned() {
        assert_eq!(
            derive_cycle_status(counts(0, 0, 0, 0)),
            PayoutCycleStatus::Planned
        );
    }

    #[test]
    fn all_pending_is_planned() {
        assert_eq!(
            derive_cycle_status(counts(3, 0, 0, 0)),
            PayoutCycleStatus::Planned
        );
    }

    #[test]
    fn all_confirmed_is_settled() {
        assert_eq!(
            derive_cycle_status(counts(0, 0, 4, 0)),
            PayoutCycleStatus::Settled
        );
    }

    #[test]
    fn all_failed_is_failed() {
        assert_eq!(
            derive_cycle_status(counts(0, 0, 0, 2)),
            PayoutCycleStatus::Failed
        );
    }

    #[test]
    fn terminal_mix_is_partially_settled() {
        assert_eq!(
            derive_cycle_status(counts(0, 0, 2, 1)),
            PayoutCycleStatus::PartiallySettled
        );
    }

    #[test]
    fn confirmed_with_work_remaining_is_partially_settled() {
        assert_eq!(
            derive_cycle_status(counts(1, 1, 2, 0)),
            PayoutCycleStatus::PartiallySettled
        );
    }

    #[test]
    fn in_flight_without_confirmation_is_broadcasting() {
        assert_eq!(
            derive_cycle_status(counts(1, 2, 0, 0)),
            PayoutCycleStatus::Broadcasting
        );
    }

    #[test]
    fn failure_with_pending_remainder_is_broadcasting() {
        assert_eq!(
            derive_cycle_status(counts(2, 0, 0, 1)),
            PayoutCycleStatus::Broadcasting
        );
    }
}
