//! Treasury-snapshot aggregate — periodic hot-wallet balance
//! captures for audit and reconcile.
//!
//! Snapshots are append-only (no updates after insert) and feed the
//! Phase 8 auditor's running-balance ledger. The
//! [runbook 11](../runbooks/11-key-rotation.md) procedure inserts a
//! row at every treasury key rotation; the periodic capture job
//! (Phase 4+) inserts daily.

use chrono::{DateTime, Utc};
use sqlx::PgExecutor;

use crate::DbError;

/// One row of the `treasury_snapshot` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TreasurySnapshot {
    /// Synthetic primary key.
    pub id: i64,
    /// Capture timestamp.
    pub captured_at: DateTime<Utc>,
    /// Hot-wallet KAS balance in sompi.
    pub kas_balance_sompi: i64,
    /// Hot-wallet NACHO balance (KRC-20 tokens, integer units).
    pub nacho_balance: i64,
    /// DAA score of the chain at capture time.
    pub daa_score: i64,
    /// Blue score of the chain at capture time.
    pub blue_score: i64,
    /// Operator-facing free-form note.
    pub notes: Option<String>,
    /// Spendable treasury UTXO count at capture time. `None` for snapshot
    /// paths that do not observe it (e.g. key rotation) and for rows that
    /// predate the consolidation engine; the consolidation tick always sets it.
    pub utxo_count: Option<i32>,
}

/// Append a new snapshot. Always succeeds modulo connection errors.
pub async fn insert<'e, E>(
    executor: E,
    kas_balance_sompi: i64,
    nacho_balance: i64,
    daa_score: i64,
    blue_score: i64,
    notes: Option<&str>,
) -> Result<i64, DbError>
where
    E: PgExecutor<'e>,
{
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO treasury_snapshot
            (kas_balance_sompi, nacho_balance, daa_score, blue_score, notes)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING id",
    )
    .bind(kas_balance_sompi)
    .bind(nacho_balance)
    .bind(daa_score)
    .bind(blue_score)
    .bind(notes)
    .fetch_one(executor)
    .await?;
    Ok(id)
}

/// Append a consolidation-engine snapshot: the spendable KAS balance, the
/// spendable UTXO `count`, and the chain DAA score.
///
/// This path observes only the fields the consolidation engine reads from
/// kaspad, so `nacho_balance` and `blue_score` are recorded as `0` (the
/// engine does not query them); `utxo_count` is the meaningful new signal.
pub async fn insert_snapshot<'e, E>(
    executor: E,
    kas_balance_sompi: i64,
    utxo_count: i32,
    daa_score: i64,
    notes: Option<&str>,
) -> Result<i64, DbError>
where
    E: PgExecutor<'e>,
{
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO treasury_snapshot
            (kas_balance_sompi, nacho_balance, daa_score, blue_score, notes, utxo_count)
         VALUES ($1, 0, $2, 0, $3, $4)
         RETURNING id",
    )
    .bind(kas_balance_sompi)
    .bind(daa_score)
    .bind(notes)
    .bind(utxo_count)
    .fetch_one(executor)
    .await?;
    Ok(id)
}

/// Latest snapshot, or `None` if the table is empty.
pub async fn latest<'e, E: PgExecutor<'e>>(
    executor: E,
) -> Result<Option<TreasurySnapshot>, DbError> {
    sqlx::query_as::<_, TreasurySnapshot>(
        "SELECT id, captured_at, kas_balance_sompi, nacho_balance, daa_score, blue_score, notes,
                utxo_count
           FROM treasury_snapshot
          ORDER BY captured_at DESC
          LIMIT 1",
    )
    .fetch_optional(executor)
    .await
    .map_err(DbError::from)
}

/// Recent snapshots, newest-first.
pub async fn list_recent<'e, E: PgExecutor<'e>>(
    executor: E,
    limit: i64,
) -> Result<Vec<TreasurySnapshot>, DbError> {
    sqlx::query_as::<_, TreasurySnapshot>(
        "SELECT id, captured_at, kas_balance_sompi, nacho_balance, daa_score, blue_score, notes,
                utxo_count
           FROM treasury_snapshot
          ORDER BY captured_at DESC
          LIMIT $1",
    )
    .bind(limit)
    .fetch_all(executor)
    .await
    .map_err(DbError::from)
}
