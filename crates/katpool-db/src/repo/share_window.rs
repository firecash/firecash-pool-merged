//! Share-window aggregate — pre-aggregated PROP rollups for closed
//! DAA windows.
//!
//! Materialised by the accountant once a window closes (typically
//! once per block-found event for PROP-N pools, or on a fixed cadence
//! for sliding-window variants). The payout engines read this table
//! instead of scanning the live `share` table for every payout
//! computation.

// DAA scores cross the u64↔i64 BIGINT boundary; see the share/block
// module-level allow for the safety argument.
#![allow(clippy::cast_possible_wrap)]

use chrono::{DateTime, Utc};
use katpool_domain::DaaScore;
use sqlx::PgExecutor;

use crate::DbError;
use crate::repo::WalletId;

/// One row of the `share_window` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ShareWindow {
    /// Synthetic primary key.
    pub id: i64,
    /// FK to `wallet.id`.
    pub wallet_id: WalletId,
    /// Half-open window start (inclusive).
    pub daa_start: i64,
    /// Half-open window end (exclusive).
    pub daa_end: i64,
    /// Wall-clock window start.
    pub started_at: DateTime<Utc>,
    /// Wall-clock window end.
    pub ended_at: DateTime<Utc>,
    /// Aggregate weight: sum(share.difficulty) over the window.
    pub total_weight: f64,
    /// Number of shares in the window.
    pub share_count: i64,
}

/// Insert one rollup row. The `UNIQUE (wallet_id, daa_start, daa_end)`
/// constraint enforces no duplicate aggregation; callers should
/// double-check with [`find`] before recomputing.
#[allow(clippy::too_many_arguments)]
pub async fn insert<'e, E>(
    executor: E,
    wallet_id: WalletId,
    daa_start: DaaScore,
    daa_end: DaaScore,
    started_at: DateTime<Utc>,
    ended_at: DateTime<Utc>,
    total_weight: f64,
    share_count: i64,
) -> Result<i64, DbError>
where
    E: PgExecutor<'e>,
{
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO share_window
            (wallet_id, daa_start, daa_end, started_at, ended_at, total_weight, share_count)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         RETURNING id",
    )
    .bind(wallet_id.0)
    .bind(daa_start.value() as i64)
    .bind(daa_end.value() as i64)
    .bind(started_at)
    .bind(ended_at)
    .bind(total_weight)
    .bind(share_count)
    .fetch_one(executor)
    .await?;
    Ok(id)
}

/// Look up the rollup for a (wallet, window) tuple.
pub async fn find<'e, E: PgExecutor<'e>>(
    executor: E,
    wallet_id: WalletId,
    daa_start: DaaScore,
    daa_end: DaaScore,
) -> Result<Option<ShareWindow>, DbError> {
    sqlx::query_as::<_, ShareWindow>(
        "SELECT id, wallet_id, daa_start, daa_end, started_at, ended_at, total_weight, share_count
           FROM share_window
          WHERE wallet_id = $1
            AND daa_start = $2
            AND daa_end   = $3",
    )
    .bind(wallet_id.0)
    .bind(daa_start.value() as i64)
    .bind(daa_end.value() as i64)
    .fetch_optional(executor)
    .await
    .map_err(DbError::from)
}

/// List every wallet that contributed to a given DAA window.
pub async fn list_for_window<'e, E: PgExecutor<'e>>(
    executor: E,
    daa_start: DaaScore,
    daa_end: DaaScore,
) -> Result<Vec<ShareWindow>, DbError> {
    sqlx::query_as::<_, ShareWindow>(
        "SELECT id, wallet_id, daa_start, daa_end, started_at, ended_at, total_weight, share_count
           FROM share_window
          WHERE daa_start = $1
            AND daa_end   = $2
          ORDER BY total_weight DESC",
    )
    .bind(daa_start.value() as i64)
    .bind(daa_end.value() as i64)
    .fetch_all(executor)
    .await
    .map_err(DbError::from)
}
