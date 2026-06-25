//! Connection-session aggregate — per-stratum-TCP-connection record.
//!
//! A live row is [`open`]ed when the connection authenticates
//! (`mining.authorize`), with its `worker_id` bound up-front from the
//! authorize payload, and finalized by [`close`] at disconnect. The
//! matching open/close pair is correlated by the bridge connection id
//! in the accountant. `worker_id` is nullable because a connection can
//! authorize with a bare address (no `.worker` suffix) — such a session
//! carries no worker identity anywhere (its shares are likewise
//! unattributed), so the null is correct, not a gap to be backfilled.
//! Sessions that drop *before* authorize have no open row and are
//! instead persisted at close via [`record_closed`].
//!
//! Used for per-IP forensics, per-rig analytics, and anti-abuse
//! audit trails.

// Sessions populated with shares_credited/_rejected/malformed_frames
// counters from the bridge's anti-abuse layer; those are u64-like
// counts but stored as BIGINT (signed). See the share/block modules
// for the same boundary rationale.
#![allow(clippy::cast_possible_wrap)]

use std::net::IpAddr;

use chrono::{DateTime, Utc};
use sqlx::PgExecutor;

use crate::DbError;
use crate::repo::{SessionId, WorkerId};

/// One row of the `connection_session` table.
///
/// The `remote_ip` column is a postgres `INET` server-side. We map it
/// as `String` at the Rust boundary (canonical bech32-like text form)
/// rather than `std::net::IpAddr` because sqlx doesn't ship a built-in
/// `IpAddr` codec; using `String` avoids pulling in the `ipnetwork`
/// crate just for this one column. Postgres handles the text↔inet
/// cast both directions, so range queries on `inet` columns still work
/// from the application side.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ConnectionSession {
    /// Synthetic primary key.
    pub id: SessionId,
    /// FK to `worker.id`; nullable for pre-authorize sessions.
    pub worker_id: Option<WorkerId>,
    /// Remote endpoint IP as text (postgres `INET` column).
    pub remote_ip: String,
    /// Stratum `mining.subscribe` user-agent string, if any.
    pub remote_app: Option<String>,
    /// TCP-accept timestamp.
    pub connected_at: DateTime<Utc>,
    /// Disconnect timestamp; `None` while still active.
    pub disconnected_at: Option<DateTime<Utc>>,
    /// Running count of accepted shares for this session.
    pub shares_credited: i64,
    /// Running count of rejected shares for this session.
    pub shares_rejected: i64,
    /// Running count of frames that failed JSON-RPC parsing.
    pub malformed_frames: i64,
}

/// Open a live session row when a connection authenticates.
///
/// The row is created with `disconnected_at = NULL` so it counts as an
/// active session until [`close`] finalizes it. `connected_at` is the
/// real TCP-accept time (the bridge carries it from the connection),
/// and `worker_id`/`country` are bound up-front when known. Returns the
/// new id so the accountant can correlate the later close.
pub async fn open<'e, E>(
    executor: E,
    worker_id: Option<WorkerId>,
    remote_ip: IpAddr,
    remote_app: Option<&str>,
    country: Option<&str>,
    connected_at: DateTime<Utc>,
) -> Result<SessionId, DbError>
where
    E: PgExecutor<'e>,
{
    let id: SessionId = sqlx::query_scalar::<_, SessionId>(
        "INSERT INTO connection_session
             (worker_id, remote_ip, remote_app, country, connected_at)
         VALUES ($1, $2::inet, $3, $4, $5)
         RETURNING id",
    )
    .bind(worker_id.map(|w| w.0))
    .bind(remote_ip.to_string())
    .bind(remote_app)
    .bind(country)
    .bind(connected_at)
    .fetch_one(executor)
    .await?;
    Ok(id)
}

/// Insert an already-completed session row in a single statement.
///
/// Used by the accountant when it learns of a session only at
/// disconnect (the bridge holds no DB handle, so it cannot `open` at
/// accept time). `worker_id` is `None` for sessions that never
/// authorized. Per-session counters are left at their schema defaults
/// (0) — this path records identity + lifetime, not share tallies.
pub async fn record_closed<'e, E>(
    executor: E,
    worker_id: Option<WorkerId>,
    remote_ip: IpAddr,
    remote_app: Option<&str>,
    country: Option<&str>,
    connected_at: DateTime<Utc>,
    disconnected_at: DateTime<Utc>,
) -> Result<SessionId, DbError>
where
    E: PgExecutor<'e>,
{
    let id: SessionId = sqlx::query_scalar::<_, SessionId>(
        "INSERT INTO connection_session
             (worker_id, remote_ip, remote_app, country, connected_at, disconnected_at)
         VALUES ($1, $2::inet, $3, $4, $5, $6)
         RETURNING id",
    )
    .bind(worker_id.map(|w| w.0))
    .bind(remote_ip.to_string())
    .bind(remote_app)
    .bind(country)
    .bind(connected_at)
    .bind(disconnected_at)
    .fetch_one(executor)
    .await?;
    Ok(id)
}

/// Close the session at TCP-disconnect.
pub async fn close<'e, E: PgExecutor<'e>>(
    executor: E,
    session_id: SessionId,
) -> Result<(), DbError> {
    sqlx::query(
        "UPDATE connection_session
            SET disconnected_at = COALESCE(disconnected_at, now())
          WHERE id = $1",
    )
    .bind(session_id.0)
    .execute(executor)
    .await?;
    Ok(())
}

/// Close every still-open session in one statement; returns the number
/// of rows closed.
///
/// Called once at accountant startup. The bridge and accountant share a
/// single process, so a restart drops all TCP sockets — any row left
/// `disconnected_at IS NULL` belongs to a dead connection from the
/// previous incarnation and must be finalized so it doesn't linger as a
/// phantom "active" session. Surviving miners reconnect and re-`open`.
pub async fn close_all_open<'e, E: PgExecutor<'e>>(executor: E) -> Result<u64, DbError> {
    let result = sqlx::query(
        "UPDATE connection_session
            SET disconnected_at = now()
          WHERE disconnected_at IS NULL",
    )
    .execute(executor)
    .await?;
    Ok(result.rows_affected())
}

/// Live count of currently-open sessions and the distinct authenticated
/// workers among them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActiveSessions {
    /// Open sessions (`disconnected_at IS NULL`).
    pub sessions: i64,
    /// Distinct non-null `worker_id` among open sessions.
    pub workers: i64,
}

/// Summarize currently-connected sessions for the live "connected now"
/// view. Aggregate-only by construction — no IP or per-miner identity is
/// exposed.
pub async fn active_summary<'e, E: PgExecutor<'e>>(executor: E) -> Result<ActiveSessions, DbError> {
    let (sessions, workers): (i64, i64) = sqlx::query_as(
        "SELECT count(*)::bigint AS sessions,
                count(DISTINCT worker_id)::bigint AS workers
           FROM connection_session
          WHERE disconnected_at IS NULL",
    )
    .fetch_one(executor)
    .await?;
    Ok(ActiveSessions { sessions, workers })
}

/// Increment the per-session counters atomically. Called once per
/// `PoolEvent` the bridge surfaces.
pub async fn increment_counters<'e, E>(
    executor: E,
    session_id: SessionId,
    credited_delta: u32,
    rejected_delta: u32,
    malformed_delta: u32,
) -> Result<(), DbError>
where
    E: PgExecutor<'e>,
{
    sqlx::query(
        "UPDATE connection_session
            SET shares_credited  = shares_credited  + $2,
                shares_rejected  = shares_rejected  + $3,
                malformed_frames = malformed_frames + $4
          WHERE id = $1",
    )
    .bind(session_id.0)
    .bind(i64::from(credited_delta))
    .bind(i64::from(rejected_delta))
    .bind(i64::from(malformed_delta))
    .execute(executor)
    .await?;
    Ok(())
}

/// One row of the firmware / user-agent breakdown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirmwareCount {
    /// Reported stratum `mining.subscribe` user-agent, or `None` when the
    /// client sent none. Callers normalize this to a vendor for display.
    pub remote_app: Option<String>,
    /// Distinct workers reporting this user-agent in the window.
    pub workers: i64,
    /// Sessions opened with this user-agent in the window.
    pub sessions: i64,
}

/// Breakdown of distinct workers (and sessions) by reported stratum
/// user-agent, over sessions that overlapped `[since, now]`.
///
/// A session overlaps the window if it is still open
/// (`disconnected_at IS NULL`) or ended at/after `since`. `workers`
/// counts distinct non-null `worker_id` (pre-authorize sessions have a
/// null worker and contribute only to `sessions`). Drives the
/// dashboard's firmware/device breakdown. Ordered by descending
/// workers, then sessions, for a stable display.
pub async fn firmware_breakdown<'e, E>(
    executor: E,
    since: DateTime<Utc>,
) -> Result<Vec<FirmwareCount>, DbError>
where
    E: PgExecutor<'e>,
{
    let rows: Vec<(Option<String>, i64, i64)> = sqlx::query_as(
        "SELECT remote_app,
                count(DISTINCT worker_id)::bigint AS workers,
                count(*)::bigint AS sessions
           FROM connection_session
          WHERE disconnected_at IS NULL
             OR disconnected_at >= $1
          GROUP BY remote_app
          ORDER BY workers DESC, sessions DESC",
    )
    .bind(since)
    .fetch_all(executor)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(remote_app, workers, sessions)| FirmwareCount {
            remote_app,
            workers,
            sessions,
        })
        .collect())
}

/// One row of the per-country session breakdown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CountryCount {
    /// ISO-3166-1 alpha-2 country code resolved at session-persist time
    /// (ADR-0025). Only non-null countries are returned.
    pub country: String,
    /// Distinct workers reporting from this country in the window.
    pub workers: i64,
    /// Sessions opened from this country in the window.
    pub sessions: i64,
}

/// Aggregate distinct workers (and sessions) by resolved country, over
/// sessions that overlapped `[since, now]`.
///
/// Overlap matches the firmware breakdown: a session counts if it is
/// still open (`disconnected_at IS NULL`) or ended at/after `since`.
/// Rows with a `NULL` country (resolver disabled, private/unknown IP, or
/// sessions persisted before ADR-0025) are excluded. `workers` counts
/// distinct non-null `worker_id`. Ordered by descending workers, then
/// sessions, for stable display. Aggregate-only by construction — no IP
/// or per-miner geo is exposed.
pub async fn country_breakdown<'e, E>(
    executor: E,
    since: DateTime<Utc>,
) -> Result<Vec<CountryCount>, DbError>
where
    E: PgExecutor<'e>,
{
    let rows: Vec<(String, i64, i64)> = sqlx::query_as(
        "SELECT country,
                count(DISTINCT worker_id)::bigint AS workers,
                count(*)::bigint AS sessions
           FROM connection_session
          WHERE country IS NOT NULL
            AND (disconnected_at IS NULL OR disconnected_at >= $1)
          GROUP BY country
          ORDER BY workers DESC, sessions DESC",
    )
    .bind(since)
    .fetch_all(executor)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(country, workers, sessions)| CountryCount {
            country,
            workers,
            sessions,
        })
        .collect())
}

/// List recent sessions for a given worker, newest-first.
pub async fn list_for_worker<'e, E: PgExecutor<'e>>(
    executor: E,
    worker_id: WorkerId,
    limit: i64,
) -> Result<Vec<ConnectionSession>, DbError> {
    sqlx::query_as::<_, ConnectionSession>(
        "SELECT id, worker_id, host(remote_ip)::text AS remote_ip, remote_app,
                connected_at, disconnected_at,
                shares_credited, shares_rejected, malformed_frames
           FROM connection_session
          WHERE worker_id = $1
          ORDER BY connected_at DESC
          LIMIT $2",
    )
    .bind(worker_id.0)
    .bind(limit)
    .fetch_all(executor)
    .await
    .map_err(DbError::from)
}
