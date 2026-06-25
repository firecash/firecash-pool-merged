//! Share-window aggregator.
//!
//! Closes a half-open DAA range `[start, end)` by scanning the
//! `share` table once and materialising one `share_window` row
//! per wallet that contributed at least one share in the range.
//!
//! ## Why this is a separate module from the consumer
//!
//! Window aggregation is **not** event-driven — it's triggered
//! either:
//!
//! - On block maturity (M3 wires this — PROP allocation reads the
//!   pre-aggregated rollups instead of scanning live shares), or
//! - On a scheduled cadence for sliding-window variants (Phase 7
//!   may add this for the hashrate-history surface).
//!
//! M1's consumer doesn't call it. M2 ships the primitive; M3
//! wires the trigger. Keeping it in `accountant` rather than
//! `katpool-db` is a layering choice: the aggregator owns a *policy*
//! (one row per wallet, weight = sum(difficulty), idempotent via
//! ON-CONFLICT) that's broader than a single SQL repo function.
//!
//! ## Idempotency
//!
//! The schema's `UNIQUE (wallet_id, daa_start, daa_end)` on
//! `share_window` is the safety net. The aggregator uses
//! `ON CONFLICT DO UPDATE` to refresh `total_weight` /
//! `share_count` / `ended_at` for a window that's been closed
//! before — useful if late shares for an already-closed window
//! land (e.g., from a delayed retransmit). The `started_at` value
//! is preserved on conflict so the original first-share timestamp
//! survives.
//!
//! ## Atomicity
//!
//! All inserts for one window go through a single transaction.
//! Partial closure isn't representable — either every wallet's
//! rollup landed or none did.

#![allow(clippy::cast_possible_wrap)]

use chrono::{DateTime, Utc};
use katpool_db::DbError;
use katpool_domain::DaaScore;
use sqlx::PgPool;
use tracing::{debug, info};

/// Outcome of a single window-close operation.
///
/// Doesn't derive `Eq` — `total_weight` is `f64` which only
/// satisfies `PartialEq`. Tests compare fields individually.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CloseOutcome {
    /// Number of `share_window` rows materialised (one per
    /// contributing wallet).
    pub wallets: i64,
    /// Total `share` rows aggregated.
    pub shares: i64,
    /// Sum of `share.difficulty` across the window.
    pub total_weight: f64,
}

/// Aggregator handle. Cheap to clone (`PgPool` is `Arc`-y).
#[derive(Debug, Clone)]
pub struct WindowAggregator {
    db: PgPool,
}

impl WindowAggregator {
    /// Construct an aggregator against the given DB pool.
    #[must_use]
    pub const fn new(db: PgPool) -> Self {
        Self { db }
    }

    /// Close a half-open DAA range, materialising
    /// `share_window` rows.
    ///
    /// Returns `Ok(CloseOutcome { wallets: 0, .. })` if no shares
    /// landed in the range — that's the legitimate "empty window"
    /// outcome (the next block's window may be short).
    ///
    /// The `now` parameter is the wall-clock instant used for
    /// `ended_at` and, for new rows, `started_at`. Callers pass
    /// `Utc::now()` in production and a deterministic stamp in
    /// tests. Keeping it as an argument keeps the function pure
    /// w.r.t. system clock so tests don't have to mock time.
    pub async fn close_window(
        &self,
        daa_start: DaaScore,
        daa_end: DaaScore,
        now: DateTime<Utc>,
    ) -> Result<CloseOutcome, DbError> {
        if daa_end.value() <= daa_start.value() {
            return Err(DbError::Config {
                message: format!(
                    "close_window: daa_end ({}) must be > daa_start ({})",
                    daa_end.value(),
                    daa_start.value()
                ),
            });
        }

        let mut tx = self.db.begin().await.map_err(DbError::from)?;

        // Materialise the rollups. The ON CONFLICT branch
        // refreshes total_weight / share_count / ended_at but
        // preserves the original started_at — so the table records
        // "first observed at" rather than "last refreshed at".
        let inserted: i64 = sqlx::query_scalar(
            "WITH agg AS (
                 SELECT wallet_id,
                        sum(difficulty)::float8 AS w,
                        count(*)::bigint        AS n,
                        min(credited_at)         AS first_credit
                   FROM share
                  WHERE daa_score >= $1
                    AND daa_score <  $2
                  GROUP BY wallet_id
             )
             INSERT INTO share_window
                 (wallet_id, daa_start, daa_end, started_at, ended_at, total_weight, share_count)
             SELECT wallet_id, $1, $2, COALESCE(first_credit, $3), $3, w, n FROM agg
             ON CONFLICT (wallet_id, daa_start, daa_end) DO UPDATE
                 SET total_weight = EXCLUDED.total_weight,
                     share_count  = EXCLUDED.share_count,
                     ended_at     = EXCLUDED.ended_at
             RETURNING wallet_id",
        )
        .bind(daa_start.value() as i64)
        .bind(daa_end.value() as i64)
        .bind(now)
        .fetch_all(&mut *tx)
        .await
        .map(|rows: Vec<i64>| rows.len() as i64)?;

        // Sum totals from the rows we just produced. One extra
        // round-trip but it's a single indexed scan keyed by the
        // (daa_start, daa_end) we already validated.
        // Postgres `sum(bigint)` returns `numeric`; cast to
        // bigint so sqlx decodes it as `i64` directly. The pool
        // would have to record ~9e18 shares in one window for
        // this cast to overflow — multiple millennia at 10K/s.
        let totals: (Option<f64>, Option<i64>) = sqlx::query_as(
            "SELECT sum(total_weight), sum(share_count)::bigint
               FROM share_window
              WHERE daa_start = $1 AND daa_end = $2",
        )
        .bind(daa_start.value() as i64)
        .bind(daa_end.value() as i64)
        .fetch_one(&mut *tx)
        .await
        .map_err(DbError::from)?;

        tx.commit().await.map_err(DbError::from)?;

        let outcome = CloseOutcome {
            wallets: inserted,
            shares: totals.1.unwrap_or(0),
            total_weight: totals.0.unwrap_or(0.0),
        };
        if outcome.wallets == 0 {
            debug!(
                daa_start = daa_start.value(),
                daa_end = daa_end.value(),
                "share window closed empty"
            );
        } else {
            info!(
                daa_start = daa_start.value(),
                daa_end = daa_end.value(),
                wallets = outcome.wallets,
                shares = outcome.shares,
                total_weight = outcome.total_weight,
                "share window closed"
            );
        }
        Ok(outcome)
    }
}
