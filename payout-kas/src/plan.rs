//! KAS payout cycle planning (database only; no chain interaction).

use katpool_db::DbError;
use katpool_db::repo::payout::{self, Payout, PayoutCycle, PayoutKind};
use katpool_domain::DaaScore;
use sqlx::PgPool;

use crate::error::PayoutKasError;

/// Parameters for [`plan_kas_cycle`].
#[derive(Debug, Clone, Copy)]
pub struct PlanKasCycleParams {
    /// Half-open DAA range start (inclusive).
    pub daa_start: DaaScore,
    /// Half-open DAA range end (exclusive).
    pub daa_end: DaaScore,
    /// Minimum payable sompi per wallet (typically 5 KAS).
    pub threshold_sompi: i64,
}

/// Outcome of a successful planning pass.
#[derive(Debug, Clone)]
pub struct PlanKasCycleResult {
    /// The cycle row (`planned` status).
    pub cycle: PayoutCycle,
    /// One `payout` row per eligible wallet.
    pub payouts: Vec<Payout>,
}

/// Create (or resume) a KAS payout cycle and insert planned payout rows.
///
/// Idempotent on retry:
/// - `create_cycle` is keyed by `idempotency_key`
/// - `ensure_payout` is keyed by `(cycle_id, wallet_id)`
/// - totals are recomputed from the final recipient set
///
/// Does not touch kaspad or sign transactions (M4.6+).
pub async fn plan_kas_cycle(
    pool: &PgPool,
    params: PlanKasCycleParams,
) -> Result<PlanKasCycleResult, PayoutKasError> {
    let mut tx = pool.begin().await.map_err(DbError::from)?;

    let cycle =
        payout::create_cycle(&mut *tx, PayoutKind::Kas, params.daa_start, params.daa_end).await?;

    let eligible = payout::list_kas_eligible_wallets(&mut *tx, params.threshold_sompi).await?;

    let mut total_sompi = 0_i64;
    let mut total_recipients = 0_i32;
    for wallet in &eligible {
        payout::ensure_payout(&mut *tx, cycle.id, wallet.wallet_id, wallet.payable_sompi).await?;
        total_sompi = total_sompi.saturating_add(wallet.payable_sompi);
        total_recipients = total_recipients.saturating_add(1);
    }

    payout::set_cycle_totals(&mut *tx, cycle.id, total_sompi, total_recipients).await?;

    tx.commit().await.map_err(DbError::from)?;

    let payouts = payout::list_for_cycle(pool, cycle.id).await?;
    let cycle = payout::get_cycle(pool, cycle.id).await?;

    Ok(PlanKasCycleResult { cycle, payouts })
}
