//! Share-reject aggregate — persisted rejections from the stratum
//! bridge.
//!
//! Each `PoolEvent::ShareRejected` the accountant observes lands
//! here as one row. The per-miner stats surface aggregates over
//! this table to answer "what's been rejected for this worker
//! lately, and why?"; operator forensics queries it to find
//! attack patterns (waves of `bad_pow` from a specific subnet,
//! sudden `low_difficulty` against a worker that was fine an hour
//! ago, etc.).
//!
//! The `reason` enum mirrors `katpool_domain::ShareRejectReason::as_str()`
//! one-for-one — see migration `20260527000000_share_reject.sql`
//! for the on-disk variant ordering, which must stay stable.

use chrono::{DateTime, Utc};
use katpool_domain::{CorrelationId, ShareRejectReason};
use sqlx::PgExecutor;
use uuid::Uuid;

use crate::DbError;
use crate::repo::{ShareRejectId, WalletId, WorkerId};

/// Postgres-enum-backed copy of [`ShareRejectReason`].
///
/// We don't reuse the domain enum directly because that crate is
/// intentionally `sqlx`-free (domain types must compile in
/// no-`sqlx` contexts like the bridge). Conversion is via
/// `TryFrom` so the build fails when the upstream
/// `#[non_exhaustive]` enum grows without a paired migration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(type_name = "share_reject_reason", rename_all = "snake_case")]
pub enum DbShareRejectReason {
    /// Submission too old.
    Stale,
    /// Worker's assigned difficulty wasn't met.
    LowDifficulty,
    /// `kaspad` rejected the candidate (typically bad `PoW`).
    BadPow,
    /// Stratum job id not recognised.
    MissingJob,
    /// Submission frame unparsable.
    MalformedFrame,
    /// Dedup window saw the same submission twice.
    DuplicateSubmit,
    /// Authenticated wallet didn't parse.
    BadAddress,
}

/// Unknown upstream `ShareRejectReason`.
///
/// Surfaces when the bridge adds a new reject category without a
/// paired DB migration — the build won't fail (the `TryFrom`
/// keeps types valid), but the runtime caller treats this as
/// recoverable (metric tick, skip the row).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("share-reject reason `{reason}` has no DB enum mapping; add a migration")]
pub struct UnknownShareRejectReason {
    /// The unmapped reason's stable `as_str()` label.
    pub reason: &'static str,
}

impl TryFrom<ShareRejectReason> for DbShareRejectReason {
    type Error = UnknownShareRejectReason;

    fn try_from(r: ShareRejectReason) -> Result<Self, Self::Error> {
        // Match deliberately rather than via wildcard so any new
        // variant added to the (non_exhaustive) domain enum fails
        // the build until the migration + this match grow together.
        match r {
            ShareRejectReason::Stale => Ok(Self::Stale),
            ShareRejectReason::LowDifficulty => Ok(Self::LowDifficulty),
            ShareRejectReason::BadPow => Ok(Self::BadPow),
            ShareRejectReason::MissingJob => Ok(Self::MissingJob),
            ShareRejectReason::MalformedFrame => Ok(Self::MalformedFrame),
            ShareRejectReason::DuplicateSubmit => Ok(Self::DuplicateSubmit),
            ShareRejectReason::BadAddress => Ok(Self::BadAddress),
            other => Err(UnknownShareRejectReason {
                reason: other.as_str(),
            }),
        }
    }
}

impl DbShareRejectReason {
    /// Stable lowercase string — must match the underlying
    /// postgres enum variant label exactly.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Stale => "stale",
            Self::LowDifficulty => "low_difficulty",
            Self::BadPow => "bad_pow",
            Self::MissingJob => "missing_job",
            Self::MalformedFrame => "malformed_frame",
            Self::DuplicateSubmit => "duplicate_submit",
            Self::BadAddress => "bad_address",
        }
    }
}

/// One row of the `share_reject` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ShareReject {
    /// Synthetic primary key.
    pub id: ShareRejectId,
    /// FK to `wallet.id`.
    pub wallet_id: WalletId,
    /// FK to `worker.id`.
    pub worker_id: WorkerId,
    /// Why the bridge rejected.
    pub reason: DbShareRejectReason,
    /// Wall-clock rejection time.
    pub rejected_at: DateTime<Utc>,
    /// `PoolEvent::ShareRejected.correlation_id`.
    pub correlation_id: Uuid,
}

/// Insert one rejection row.
///
/// Caller is responsible for upserting `wallet_id` / `worker_id`
/// before calling; the FK constraints will surface a
/// [`DbError::Constraint`] otherwise.
pub async fn insert<'e, E>(
    executor: E,
    wallet_id: WalletId,
    worker_id: WorkerId,
    reason: DbShareRejectReason,
    correlation_id: CorrelationId,
) -> Result<ShareRejectId, DbError>
where
    E: PgExecutor<'e>,
{
    let id = sqlx::query_scalar::<_, ShareRejectId>(
        "INSERT INTO share_reject (wallet_id, worker_id, reason, correlation_id)
         VALUES ($1, $2, $3, $4)
         RETURNING id",
    )
    .bind(wallet_id.0)
    .bind(worker_id.0)
    .bind(reason)
    .bind(*correlation_id.as_uuid())
    .fetch_one(executor)
    .await?;
    Ok(id)
}

/// Per-reason rejection counts for a wallet since `since`.
///
/// Returns one `(reason, count)` row per reason that has ≥ 1
/// rejection — reasons with zero rejections are omitted. Callers
/// that want a dense vector (zero-fill for absent reasons)
/// should post-process.
pub async fn count_by_reason_for_wallet<'e, E>(
    executor: E,
    wallet_id: WalletId,
    since: DateTime<Utc>,
) -> Result<Vec<(DbShareRejectReason, i64)>, DbError>
where
    E: PgExecutor<'e>,
{
    let rows: Vec<(DbShareRejectReason, i64)> = sqlx::query_as(
        "SELECT reason, count(*)::bigint
           FROM share_reject
          WHERE wallet_id = $1
            AND rejected_at >= $2
          GROUP BY reason
          ORDER BY count(*) DESC",
    )
    .bind(wallet_id.0)
    .bind(since)
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

/// Recent rejections for a wallet, newest first.
pub async fn list_for_wallet<'e, E>(
    executor: E,
    wallet_id: WalletId,
    limit: i64,
) -> Result<Vec<ShareReject>, DbError>
where
    E: PgExecutor<'e>,
{
    sqlx::query_as::<_, ShareReject>(
        "SELECT id, wallet_id, worker_id, reason, rejected_at, correlation_id
           FROM share_reject
          WHERE wallet_id = $1
          ORDER BY rejected_at DESC
          LIMIT $2",
    )
    .bind(wallet_id.0)
    .bind(limit)
    .fetch_all(executor)
    .await
    .map_err(DbError::from)
}

/// Total rejections across the whole pool since `since`, grouped
/// by reason. Used for operator-wide anti-abuse dashboards.
pub async fn count_by_reason_pool_wide<'e, E>(
    executor: E,
    since: DateTime<Utc>,
) -> Result<Vec<(DbShareRejectReason, i64)>, DbError>
where
    E: PgExecutor<'e>,
{
    let rows: Vec<(DbShareRejectReason, i64)> = sqlx::query_as(
        "SELECT reason, count(*)::bigint
           FROM share_reject
          WHERE rejected_at >= $1
          GROUP BY reason
          ORDER BY count(*) DESC",
    )
    .bind(since)
    .fetch_all(executor)
    .await?;
    Ok(rows)
}
