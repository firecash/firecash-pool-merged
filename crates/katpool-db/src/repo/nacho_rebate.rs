//! Nacho-rebate aggregate — running NACHO rebate balance per wallet.
//!
//! Two numbers per wallet: cumulative `accrued_sompi` and cumulative
//! `paid_sompi`. The schema's `nacho_rebate_paid_le_accrued` CHECK
//! constraint enforces `paid <= accrued`, so pending balance
//! (`accrued - paid`) is always non-negative.

use chrono::{DateTime, Utc};
use sqlx::PgExecutor;

use crate::DbError;
use crate::repo::WalletId;

/// One row of the `nacho_rebate_accrual` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct NachoRebate {
    /// FK to `wallet.id` (which is also this table's primary key).
    pub wallet_id: WalletId,
    /// Cumulative sompi accrued via the 33% NACHO accrual rule.
    pub accrued_sompi: i64,
    /// Cumulative sompi already paid out via NACHO payout cycles.
    pub paid_sompi: i64,
    /// Last mutation timestamp.
    pub updated_at: DateTime<Utc>,
}

impl NachoRebate {
    /// Pending (unpaid) balance. Always non-negative thanks to the
    /// `paid <= accrued` CHECK constraint.
    #[must_use]
    pub const fn pending_sompi(&self) -> i64 {
        self.accrued_sompi - self.paid_sompi
    }
}

/// Read one wallet's rebate state. Returns `Ok(None)` if the wallet
/// has never accrued.
pub async fn get<'e, E: PgExecutor<'e>>(
    executor: E,
    wallet_id: WalletId,
) -> Result<Option<NachoRebate>, DbError> {
    sqlx::query_as::<_, NachoRebate>(
        "SELECT wallet_id, accrued_sompi, paid_sompi, updated_at
           FROM nacho_rebate_accrual
          WHERE wallet_id = $1",
    )
    .bind(wallet_id.0)
    .fetch_optional(executor)
    .await
    .map_err(DbError::from)
}

/// Add `delta_sompi` to a wallet's accrued amount. Creates the row
/// if it doesn't exist. Idempotent w.r.t. wallet identity — the
/// `(wallet_id)` primary key handles concurrent accrual.
pub async fn accrue<'e, E>(
    executor: E,
    wallet_id: WalletId,
    delta_sompi: i64,
) -> Result<NachoRebate, DbError>
where
    E: PgExecutor<'e>,
{
    if delta_sompi < 0 {
        return Err(DbError::Config {
            message: format!("accrue delta must be non-negative, got {delta_sompi}"),
        });
    }
    sqlx::query_as::<_, NachoRebate>(
        "INSERT INTO nacho_rebate_accrual (wallet_id, accrued_sompi)
         VALUES ($1, $2)
         ON CONFLICT (wallet_id) DO UPDATE
            SET accrued_sompi = nacho_rebate_accrual.accrued_sompi + EXCLUDED.accrued_sompi,
                updated_at = now()
         RETURNING wallet_id, accrued_sompi, paid_sompi, updated_at",
    )
    .bind(wallet_id.0)
    .bind(delta_sompi)
    .fetch_one(executor)
    .await
    .map_err(DbError::from)
}

/// Add `delta_sompi` to a wallet's paid amount.
///
/// Errors out if the resulting `paid_sompi` would exceed
/// `accrued_sompi` — the schema's CHECK constraint enforces this and
/// surfaces as [`DbError::Constraint`] SQLSTATE `23514`.
pub async fn mark_paid<'e, E>(
    executor: E,
    wallet_id: WalletId,
    delta_sompi: i64,
) -> Result<NachoRebate, DbError>
where
    E: PgExecutor<'e>,
{
    if delta_sompi < 0 {
        return Err(DbError::Config {
            message: format!("mark_paid delta must be non-negative, got {delta_sompi}"),
        });
    }
    sqlx::query_as::<_, NachoRebate>(
        "UPDATE nacho_rebate_accrual
            SET paid_sompi = paid_sompi + $2,
                updated_at = now()
          WHERE wallet_id = $1
         RETURNING wallet_id, accrued_sompi, paid_sompi, updated_at",
    )
    .bind(wallet_id.0)
    .bind(delta_sompi)
    .fetch_one(executor)
    .await
    .map_err(DbError::from)
}

/// Idempotent **set** of a wallet's accrued amount. Distinct from
/// [`accrue`] (additive): callers like the legacy importer need to
/// replace the stored value on every re-run, not add to it.
///
/// Preserves any existing `paid_sompi` value if the row already
/// exists; resets it to 0 on first insert. Returns the row after
/// the upsert.
pub async fn set_accrual<'e, E>(
    executor: E,
    wallet_id: WalletId,
    accrued_sompi: i64,
) -> Result<NachoRebate, DbError>
where
    E: PgExecutor<'e>,
{
    if accrued_sompi < 0 {
        return Err(DbError::Config {
            message: format!("set_accrual value must be non-negative, got {accrued_sompi}"),
        });
    }
    sqlx::query_as::<_, NachoRebate>(
        "INSERT INTO nacho_rebate_accrual (wallet_id, accrued_sompi)
         VALUES ($1, $2)
         ON CONFLICT (wallet_id) DO UPDATE
            SET accrued_sompi = EXCLUDED.accrued_sompi,
                updated_at = now()
         RETURNING wallet_id, accrued_sompi, paid_sompi, updated_at",
    )
    .bind(wallet_id.0)
    .bind(accrued_sompi)
    .fetch_one(executor)
    .await
    .map_err(DbError::from)
}

/// List wallets with pending NACHO rebate balance, descending.
/// Drives the NACHO payout cycle's recipient selection.
pub async fn list_pending<'e, E: PgExecutor<'e>>(
    executor: E,
    min_pending_sompi: i64,
    limit: i64,
) -> Result<Vec<NachoRebate>, DbError> {
    sqlx::query_as::<_, NachoRebate>(
        "SELECT wallet_id, accrued_sompi, paid_sompi, updated_at
           FROM nacho_rebate_accrual
          WHERE accrued_sompi - paid_sompi >= $1
          ORDER BY accrued_sompi - paid_sompi DESC
          LIMIT $2",
    )
    .bind(min_pending_sompi)
    .bind(limit)
    .fetch_all(executor)
    .await
    .map_err(DbError::from)
}
