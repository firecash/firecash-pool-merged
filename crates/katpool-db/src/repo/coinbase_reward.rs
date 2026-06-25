// DAA scores and outpoint indices are u64/u32 in kaspa but the
// corresponding postgres columns are signed BIGINT/INTEGER. See the
// module-level note in `repo/share.rs` for the safety argument.
#![allow(clippy::cast_possible_wrap)]

//! Coinbase-reward aggregate — the pool's realised, matured coinbase
//! UTXOs, anchored by outpoint.
//!
//! Each row is one coinbase UTXO credited to the pool address that has
//! reached the consensus coinbase-maturity depth. It is the
//! ground-truth unit of PROP reward: an exact sompi amount with an
//! acceptance DAA score. The outpoint `UNIQUE` constraint makes
//! discovery idempotent; the `allocated_at` timestamp gates allocation
//! to exactly once.
//!
//! See [`crate::repo::block`] for the (now telemetry-only) block
//! lifecycle, and the accountant's maturity tracker for how rows here
//! are discovered and handed to the allocation engine.

use chrono::{DateTime, Utc};
use sqlx::PgExecutor;

use crate::DbError;
use crate::repo::CoinbaseRewardId;
use crate::repo::block::EnsureOutcome;

/// One row of the `coinbase_reward` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CoinbaseReward {
    /// Synthetic primary key.
    pub id: CoinbaseRewardId,
    /// 32-byte coinbase transaction id of the UTXO outpoint.
    pub outpoint_transaction_id: Vec<u8>,
    /// Output index within the coinbase transaction.
    pub outpoint_index: i32,
    /// Exact sompi value of the UTXO.
    pub amount_sompi: i64,
    /// DAA score of the block that created the UTXO.
    pub block_daa_score: i64,
    /// When the tracker first observed the matured UTXO.
    pub discovered_at: DateTime<Utc>,
    /// When the allocation engine distributed it; `None` until then.
    pub allocated_at: Option<DateTime<Utc>>,
}

/// Idempotently record a matured coinbase UTXO.
///
/// Returns the `CoinbaseRewardId` plus whether the row was newly
/// created. Safe to call repeatedly with the same outpoint — the
/// `UNIQUE(outpoint_transaction_id, outpoint_index)` constraint guards.
///
/// The `xmax = 0` trick distinguishes a real INSERT from an
/// ON-CONFLICT no-op UPDATE (postgres only sets `xmax > 0` when it
/// touches a row); the forced no-op `DO UPDATE` makes `RETURNING` fire
/// on the existing row.
pub async fn ensure<'e, E>(
    executor: E,
    outpoint_transaction_id: &[u8; 32],
    outpoint_index: u32,
    amount_sompi: i64,
    block_daa_score: u64,
) -> Result<(CoinbaseRewardId, EnsureOutcome), DbError>
where
    E: PgExecutor<'e>,
{
    let row: (CoinbaseRewardId, bool) = sqlx::query_as(
        "INSERT INTO coinbase_reward
            (outpoint_transaction_id, outpoint_index, amount_sompi, block_daa_score)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (outpoint_transaction_id, outpoint_index)
            DO UPDATE SET outpoint_index = EXCLUDED.outpoint_index
         RETURNING id, (xmax = 0)",
    )
    .bind(outpoint_transaction_id.as_slice())
    .bind(outpoint_index as i32)
    .bind(amount_sompi)
    .bind(block_daa_score as i64)
    .fetch_one(executor)
    .await?;
    let outcome = if row.1 {
        EnsureOutcome::Inserted
    } else {
        EnsureOutcome::AlreadyExisted
    };
    Ok((row.0, outcome))
}

/// List unallocated rewards, oldest acceptance first. Bounds tail
/// latency of any single allocation sweep via `limit`.
pub async fn list_unallocated<'e, E: PgExecutor<'e>>(
    executor: E,
    limit: i64,
) -> Result<Vec<CoinbaseReward>, DbError> {
    sqlx::query_as::<_, CoinbaseReward>(
        "SELECT id, outpoint_transaction_id, outpoint_index, amount_sompi,
                block_daa_score, discovered_at, allocated_at
           FROM coinbase_reward
          WHERE allocated_at IS NULL
          ORDER BY block_daa_score ASC, id ASC
          LIMIT $1",
    )
    .bind(limit)
    .fetch_all(executor)
    .await
    .map_err(DbError::from)
}

/// Lock a reward row `FOR UPDATE` and return it, so the allocation
/// engine can gate on `allocated_at` without a race against a
/// concurrent sweep. Returns `None` if the id doesn't exist.
pub async fn lock_for_allocation<'e, E: PgExecutor<'e>>(
    executor: E,
    id: CoinbaseRewardId,
) -> Result<Option<CoinbaseReward>, DbError> {
    sqlx::query_as::<_, CoinbaseReward>(
        "SELECT id, outpoint_transaction_id, outpoint_index, amount_sompi,
                block_daa_score, discovered_at, allocated_at
           FROM coinbase_reward
          WHERE id = $1
          FOR UPDATE",
    )
    .bind(id.0)
    .fetch_optional(executor)
    .await
    .map_err(DbError::from)
}

/// Mark a reward allocated. Idempotent — only sets `allocated_at` if it
/// was still NULL.
pub async fn mark_allocated<'e, E: PgExecutor<'e>>(
    executor: E,
    id: CoinbaseRewardId,
) -> Result<(), DbError> {
    sqlx::query(
        "UPDATE coinbase_reward
            SET allocated_at = COALESCE(allocated_at, now())
          WHERE id = $1",
    )
    .bind(id.0)
    .execute(executor)
    .await?;
    Ok(())
}

/// Find a reward by outpoint (diagnostics / tests).
pub async fn find_by_outpoint<'e, E: PgExecutor<'e>>(
    executor: E,
    outpoint_transaction_id: &[u8; 32],
    outpoint_index: u32,
) -> Result<Option<CoinbaseReward>, DbError> {
    sqlx::query_as::<_, CoinbaseReward>(
        "SELECT id, outpoint_transaction_id, outpoint_index, amount_sompi,
                block_daa_score, discovered_at, allocated_at
           FROM coinbase_reward
          WHERE outpoint_transaction_id = $1 AND outpoint_index = $2",
    )
    .bind(outpoint_transaction_id.as_slice())
    .bind(outpoint_index as i32)
    .fetch_optional(executor)
    .await
    .map_err(DbError::from)
}
