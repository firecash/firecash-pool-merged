// DAA scores are stored as u64 in kaspa-consensus-core but as BIGINT
// (signed i64) in postgres. The chain's age would have to exceed
// 9.2e18 blocks (≥ 30 billion years at 10 BPS) for the cast to wrap,
// so the truncating-cast variant is fine in practice.
#![allow(clippy::cast_possible_wrap)]

//! Share aggregate — high-volume insert table plus the PROP-window
//! aggregation reads.
//!
//! `insert_credited` is the single hot-path call: every accepted
//! `ShareCredited` `PoolEvent` from the bridge produces one row here.
//! Throughput target: ≥ 200 inserts/sec sustained, which the schema's
//! `(daa_score, wallet_id)` index handles comfortably with batched
//! commits.
//!
//! Aggregation queries (`sum_weight_for_window`, `count_for_window`)
//! drive PROP allocation; they're called once per closed window, not
//! per share, so a sequential index scan over the relevant DAA range
//! is acceptable even at hundreds of millions of rows.

use chrono::{DateTime, Utc};
use katpool_domain::{CorrelationId, DaaScore, ShareDifficulty};
use sqlx::PgExecutor;
use uuid::Uuid;

use crate::DbError;
use crate::repo::{SessionId, ShareId, WalletId, WorkerId};

/// A row from the `share` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Share {
    /// Synthetic primary key.
    pub id: ShareId,
    /// FK to `wallet.id`.
    pub wallet_id: WalletId,
    /// FK to `worker.id`.
    pub worker_id: WorkerId,
    /// FK to `connection_session.id`, nullable (cleaned-up sessions
    /// SET NULL).
    pub session_id: Option<SessionId>,
    /// Pool difficulty assigned to this share at submit time.
    pub difficulty: f64,
    /// DAA score of the job the share was submitted against.
    pub daa_score: i64,
    /// Wall-clock credit time.
    pub credited_at: DateTime<Utc>,
    /// Correlation id from the source `PoolEvent::ShareCredited`.
    pub correlation_id: Uuid,
}

/// Insert one credited share.
///
/// Caller is responsible for ensuring `wallet_id` and `worker_id`
/// resolve to existing rows; the DB's FK constraints will surface a
/// `Constraint` error otherwise. `session_id` is optional — pass
/// `None` if the share arrived before the bridge's session-tracking
/// hook recorded a row.
#[allow(clippy::too_many_arguments)]
pub async fn insert_credited<'e, E>(
    executor: E,
    wallet_id: WalletId,
    worker_id: WorkerId,
    session_id: Option<SessionId>,
    difficulty: ShareDifficulty,
    daa_score: DaaScore,
    correlation_id: CorrelationId,
) -> Result<ShareId, DbError>
where
    E: PgExecutor<'e>,
{
    let id: ShareId = sqlx::query_scalar::<_, ShareId>(
        "
        INSERT INTO share
            (wallet_id, worker_id, session_id, difficulty, daa_score, correlation_id)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING id
        ",
    )
    .bind(wallet_id.0)
    .bind(worker_id.0)
    .bind(session_id.map(|s| s.0))
    .bind(difficulty.value())
    .bind(daa_score.value() as i64)
    .bind(*correlation_id.as_uuid())
    .fetch_one(executor)
    .await?;
    Ok(id)
}

/// Sum the `difficulty` column for a wallet over the half-open DAA
/// window `[daa_start, daa_end)`. This is the per-wallet PROP weight.
///
/// Returns `0.0` when the wallet has zero shares in the range (this
/// is *not* a not-found case — wallets with no shares legitimately
/// have weight 0).
pub async fn sum_weight_for_window<'e, E>(
    executor: E,
    wallet_id: WalletId,
    daa_start: DaaScore,
    daa_end: DaaScore,
) -> Result<f64, DbError>
where
    E: PgExecutor<'e>,
{
    let weight: Option<f64> = sqlx::query_scalar(
        "
        SELECT sum(difficulty)
          FROM share
         WHERE wallet_id = $1
           AND daa_score >= $2
           AND daa_score <  $3
        ",
    )
    .bind(wallet_id.0)
    .bind(daa_start.value() as i64)
    .bind(daa_end.value() as i64)
    .fetch_one(executor)
    .await?;
    Ok(weight.unwrap_or(0.0))
}

/// Count shares for a wallet over a half-open DAA window.
pub async fn count_for_window<'e, E>(
    executor: E,
    wallet_id: WalletId,
    daa_start: DaaScore,
    daa_end: DaaScore,
) -> Result<i64, DbError>
where
    E: PgExecutor<'e>,
{
    let count: i64 = sqlx::query_scalar(
        "
        SELECT count(*)::bigint
          FROM share
         WHERE wallet_id = $1
           AND daa_score >= $2
           AND daa_score <  $3
        ",
    )
    .bind(wallet_id.0)
    .bind(daa_start.value() as i64)
    .bind(daa_end.value() as i64)
    .fetch_one(executor)
    .await?;
    Ok(count)
}

/// Total share-weight across every wallet for a DAA window. Used as
/// the denominator in PROP allocation.
pub async fn total_weight_for_window<'e, E>(
    executor: E,
    daa_start: DaaScore,
    daa_end: DaaScore,
) -> Result<f64, DbError>
where
    E: PgExecutor<'e>,
{
    let weight: Option<f64> = sqlx::query_scalar(
        "
        SELECT sum(difficulty)
          FROM share
         WHERE daa_score >= $1
           AND daa_score <  $2
        ",
    )
    .bind(daa_start.value() as i64)
    .bind(daa_end.value() as i64)
    .fetch_one(executor)
    .await?;
    Ok(weight.unwrap_or(0.0))
}
