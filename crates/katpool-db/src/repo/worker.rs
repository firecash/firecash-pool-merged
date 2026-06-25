//! Worker aggregate.
//!
//! Workers are rig identifiers within a wallet — one row per
//! `(wallet_id, worker_name)`. Cascade-delete on wallet removal.

use chrono::{DateTime, Utc};
use katpool_domain::WorkerName;
use sqlx::PgExecutor;

use crate::DbError;
use crate::repo::{WalletId, WorkerId};

/// A row from the `worker` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Worker {
    /// Synthetic primary key.
    pub id: WorkerId,
    /// FK to `wallet.id`.
    pub wallet_id: WalletId,
    /// Free-form worker label from stratum login.
    pub name: String,
    /// First time we observed this worker.
    pub first_seen_at: DateTime<Utc>,
    /// Last time the application touched this row.
    pub last_seen_at: DateTime<Utc>,
}

/// Find a worker by `(wallet_id, name)`; create it if missing. On a
/// hit, refreshes `last_seen_at`. Idempotent.
///
/// The DB's `worker_name_charset` CHECK constraint will refuse names
/// that violate the documented charset (matches
/// [`katpool_domain::WorkerName`] validation, so values that pass the
/// domain constructor will also pass the DB check).
pub async fn ensure<'e, E>(
    executor: E,
    wallet_id: WalletId,
    name: &WorkerName,
) -> Result<Worker, DbError>
where
    E: PgExecutor<'e>,
{
    sqlx::query_as::<_, Worker>(
        "
        INSERT INTO worker (wallet_id, name)
        VALUES ($1, $2)
        ON CONFLICT (wallet_id, name) DO UPDATE
            SET last_seen_at = now()
        RETURNING id, wallet_id, name, first_seen_at, last_seen_at
        ",
    )
    .bind(wallet_id.0)
    .bind(name.as_str())
    .fetch_one(executor)
    .await
    .map_err(DbError::from)
}

/// Fetch a worker by primary key. Returns [`DbError::NotFound`] if
/// missing.
pub async fn get_by_id<'e, E: PgExecutor<'e>>(
    executor: E,
    id: WorkerId,
) -> Result<Worker, DbError> {
    sqlx::query_as::<_, Worker>(
        "SELECT id, wallet_id, name, first_seen_at, last_seen_at FROM worker WHERE id = $1",
    )
    .bind(id.0)
    .fetch_one(executor)
    .await
    .map_err(DbError::from)
}

/// List every worker for a given wallet, newest-first by activity.
pub async fn list_for_wallet<'e, E: PgExecutor<'e>>(
    executor: E,
    wallet_id: WalletId,
) -> Result<Vec<Worker>, DbError> {
    sqlx::query_as::<_, Worker>(
        "SELECT id, wallet_id, name, first_seen_at, last_seen_at
           FROM worker
          WHERE wallet_id = $1
          ORDER BY last_seen_at DESC",
    )
    .bind(wallet_id.0)
    .fetch_all(executor)
    .await
    .map_err(DbError::from)
}
