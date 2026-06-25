// DAA scores and nonces are u64 in kaspa-consensus-core but the
// corresponding postgres BIGINT columns are signed i64. See the
// module-level note in `repo/share.rs` for the safety argument.
#![allow(clippy::cast_possible_wrap)]

//! Block aggregate — lifecycle state machine for blocks the pool
//! found.
//!
//! Status transitions are monotone (`found → submitted_to_node →
//! confirmed_blue → matured`), enforced by the schema's
//! `block_lifecycle_order` CHECK constraint. The repo functions
//! follow that convention: dedicated [`mark_submitted`],
//! [`mark_confirmed_blue`], and [`mark_matured`] helpers that set
//! both the `status` enum and the corresponding timestamp atomically.
//! Operators (and accountant logic) cannot skip a step.

use chrono::{DateTime, Utc};
use katpool_domain::{BlockHash, CorrelationId, DaaScore};
use sqlx::PgExecutor;
use uuid::Uuid;

use crate::DbError;
use crate::repo::{BlockId, WalletId, WorkerId};

/// One row of the `block` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Block {
    /// Synthetic primary key.
    pub id: BlockId,
    /// 32-byte block hash.
    pub hash: Vec<u8>,
    /// FK to `wallet.id`.
    pub finder_wallet_id: WalletId,
    /// FK to `worker.id`.
    pub finder_worker_id: WorkerId,
    /// DAA score of the block.
    pub daa_score: i64,
    /// Blue score, populated once kaspad confirms the block.
    pub blue_score: Option<i64>,
    /// Nonce that produced the winning `PoW`.
    pub nonce: i64,
    /// Current status in the lifecycle.
    pub status: BlockStatus,
    /// When the bridge detected the candidate.
    pub found_at: DateTime<Utc>,
    /// When kaspad ACK'd `submit_block` (status >= `submitted_to_node`).
    pub submitted_at: Option<DateTime<Utc>>,
    /// When kaspad confirmed the block blue (status >= `confirmed_blue`).
    pub confirmed_at: Option<DateTime<Utc>>,
    /// When the coinbase matured (status = `matured`).
    pub matured_at: Option<DateTime<Utc>>,
    /// Coinbase reward in sompi, populated at maturity.
    pub miner_reward_sompi: Option<i64>,
    /// Correlation id matching the source `PoolEvent::BlockFound`.
    pub correlation_id: Uuid,
}

/// The five lifecycle states. Mirrors the `block_status` Postgres
/// enum declared by the bootstrap migration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "block_status", rename_all = "snake_case")]
pub enum BlockStatus {
    /// Bridge detected the share's `PoW` met the network target.
    Found,
    /// `kaspad` accepted our `submit_block` call.
    SubmittedToNode,
    /// Block confirmed blue in the DAG.
    ConfirmedBlue,
    /// Coinbase has matured; reward is realised.
    Matured,
    /// A DAG re-org displaced the block; reward will never materialise.
    Orphaned,
}

/// Insert a new block in the `found` state. Returns the assigned
/// primary key.
pub async fn insert<'e, E>(
    executor: E,
    hash: BlockHash,
    finder_wallet_id: WalletId,
    finder_worker_id: WorkerId,
    daa_score: DaaScore,
    nonce: u64,
    correlation_id: CorrelationId,
) -> Result<BlockId, DbError>
where
    E: PgExecutor<'e>,
{
    let id: BlockId = sqlx::query_scalar::<_, BlockId>(
        "
        INSERT INTO block (hash, finder_wallet_id, finder_worker_id, daa_score, nonce, correlation_id)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING id
        ",
    )
    .bind(hash.as_bytes().to_vec())
    .bind(finder_wallet_id.0)
    .bind(finder_worker_id.0)
    .bind(daa_score.value() as i64)
    .bind(nonce as i64)
    .bind(*correlation_id.as_uuid())
    .fetch_one(executor)
    .await?;
    Ok(id)
}

/// Outcome of an idempotent insert: was the row created or did it
/// already exist?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnsureOutcome {
    /// A new row was created.
    Inserted,
    /// A row with the same `hash` already existed; no change.
    AlreadyExisted,
}

/// Idempotent insert of a candidate block.
///
/// Returns the `BlockId` plus whether the row was newly created.
/// Safe to call repeatedly with the same arguments — the table's
/// `UNIQUE(hash)` constraint guards.
///
/// Distinct from [`insert`]: callers that own deduplication (e.g.
/// the accountant's `BlockFound` consumer, which may see the
/// same event twice across reconnects) prefer `ensure`; callers
/// that want hard failure on duplicates (e.g. an importer that
/// expects exactly one row per hash) prefer `insert`.
pub async fn ensure<'e, E>(
    executor: E,
    hash: BlockHash,
    finder_wallet_id: WalletId,
    finder_worker_id: WorkerId,
    daa_score: DaaScore,
    nonce: u64,
    correlation_id: CorrelationId,
) -> Result<(BlockId, EnsureOutcome), DbError>
where
    E: PgExecutor<'e>,
{
    // The `xmax = 0` trick distinguishes a real INSERT from an
    // ON-CONFLICT no-op UPDATE: postgres only sets xmax > 0 when
    // it touches a row. The `DO UPDATE SET hash = EXCLUDED.hash`
    // is a forced no-op so RETURNING fires on the existing row.
    let row: (BlockId, bool) = sqlx::query_as(
        "INSERT INTO block
            (hash, finder_wallet_id, finder_worker_id, daa_score, nonce, correlation_id)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (hash) DO UPDATE SET hash = EXCLUDED.hash
         RETURNING id, (xmax = 0)",
    )
    .bind(hash.as_bytes().to_vec())
    .bind(finder_wallet_id.0)
    .bind(finder_worker_id.0)
    .bind(daa_score.value() as i64)
    .bind(nonce as i64)
    .bind(*correlation_id.as_uuid())
    .fetch_one(executor)
    .await?;
    let outcome = if row.1 {
        EnsureOutcome::Inserted
    } else {
        EnsureOutcome::AlreadyExisted
    };
    Ok((row.0, outcome))
}

/// Find a block by hash.
pub async fn find_by_hash<'e, E: PgExecutor<'e>>(
    executor: E,
    hash: BlockHash,
) -> Result<Option<Block>, DbError> {
    sqlx::query_as::<_, Block>(
        "SELECT id, hash, finder_wallet_id, finder_worker_id, daa_score, blue_score, nonce,
                status, found_at, submitted_at, confirmed_at, matured_at,
                miner_reward_sompi, correlation_id
           FROM block WHERE hash = $1",
    )
    .bind(hash.as_bytes().to_vec())
    .fetch_optional(executor)
    .await
    .map_err(DbError::from)
}

/// Advance the block to `submitted_to_node`. Idempotent — re-running
/// against an already-submitted block is a no-op (the existing row
/// already satisfies the lifecycle CHECK).
///
/// The schema's `block_lifecycle_order` CHECK will refuse the update
/// if the block isn't in the `found` state, surfacing as
/// [`DbError::Constraint`].
pub async fn mark_submitted<'e, E: PgExecutor<'e>>(
    executor: E,
    hash: BlockHash,
) -> Result<(), DbError> {
    sqlx::query(
        "
        UPDATE block
           SET status = 'submitted_to_node',
               submitted_at = COALESCE(submitted_at, now())
         WHERE hash = $1
           AND status IN ('found', 'submitted_to_node')
        ",
    )
    .bind(hash.as_bytes().to_vec())
    .execute(executor)
    .await?;
    Ok(())
}

/// Advance the block to `confirmed_blue`.
///
/// `blue_score` is optional telemetry: callers that have a
/// kaspad-provided blue score pass `Some(_)`; the maturity tracker
/// (which decides blueness via GHOSTDAG colour, not blue score) passes
/// `None` and leaves any existing value untouched.
pub async fn mark_confirmed_blue<'e, E: PgExecutor<'e>>(
    executor: E,
    hash: BlockHash,
    blue_score: Option<i64>,
) -> Result<(), DbError> {
    sqlx::query(
        "
        UPDATE block
           SET status = 'confirmed_blue',
               blue_score = COALESCE($2, blue_score),
               confirmed_at = COALESCE(confirmed_at, now())
         WHERE hash = $1
           AND status IN ('submitted_to_node', 'confirmed_blue')
        ",
    )
    .bind(hash.as_bytes().to_vec())
    .bind(blue_score)
    .execute(executor)
    .await?;
    Ok(())
}

/// Advance the block to `matured`, recording the coinbase reward.
pub async fn mark_matured<'e, E: PgExecutor<'e>>(
    executor: E,
    hash: BlockHash,
    miner_reward_sompi: i64,
) -> Result<(), DbError> {
    sqlx::query(
        "
        UPDATE block
           SET status = 'matured',
               miner_reward_sompi = $2,
               matured_at = COALESCE(matured_at, now())
         WHERE hash = $1
           AND status IN ('confirmed_blue', 'matured')
        ",
    )
    .bind(hash.as_bytes().to_vec())
    .bind(miner_reward_sompi)
    .execute(executor)
    .await?;
    Ok(())
}

/// Mark the block orphaned. Allowed from any non-terminal state;
/// callers should be cautious — orphaned is irreversible.
pub async fn mark_orphaned<'e, E: PgExecutor<'e>>(
    executor: E,
    hash: BlockHash,
) -> Result<(), DbError> {
    sqlx::query(
        "
        UPDATE block
           SET status = 'orphaned'
         WHERE hash = $1
           AND status <> 'matured'
        ",
    )
    .bind(hash.as_bytes().to_vec())
    .execute(executor)
    .await?;
    Ok(())
}

/// Recent blocks across all statuses, newest-first, keyset-paginated.
///
/// Pass `before_id = None` for the first page; for the next page pass
/// the smallest `id` from the previous page. Keyset (not offset)
/// pagination is stable under concurrent inserts and rides the primary
/// key index. `limit` is the caller's page size (the API caps it).
pub async fn list_recent<'e, E: PgExecutor<'e>>(
    executor: E,
    limit: i64,
    before_id: Option<i64>,
) -> Result<Vec<Block>, DbError> {
    sqlx::query_as::<_, Block>(
        "SELECT id, hash, finder_wallet_id, finder_worker_id, daa_score, blue_score, nonce,
                status, found_at, submitted_at, confirmed_at, matured_at,
                miner_reward_sompi, correlation_id
           FROM block
          WHERE ($2::bigint IS NULL OR id < $2)
          ORDER BY id DESC
          LIMIT $1",
    )
    .bind(limit)
    .bind(before_id)
    .fetch_all(executor)
    .await
    .map_err(DbError::from)
}

/// One recent block joined with its finder's worker name and wallet address.
///
/// Backs the legacy-compatible `MiningPoolStats` feed (`top_100_blocks`), which
/// reports the finding worker + wallet by their human identifiers rather than
/// the internal FK ids carried by [`Block`].
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RecentBlockIdentity {
    /// 32-byte block hash.
    pub hash: Vec<u8>,
    /// Finder's worker name (`worker.name`).
    pub worker_name: String,
    /// Finder's wallet address (`wallet.address`).
    pub wallet_address: String,
    /// DAA score of the block.
    pub daa_score: i64,
    /// Coinbase reward in sompi, populated at maturity (else `NULL`).
    pub miner_reward_sompi: Option<i64>,
    /// When the bridge detected the candidate.
    pub found_at: DateTime<Utc>,
}

/// The `limit` most recent blocks (newest-first) joined to the finder's worker
/// name and wallet address. Backs the `MiningPoolStats` `top_100_blocks` feed.
pub async fn list_recent_with_identity<'e, E: PgExecutor<'e>>(
    executor: E,
    limit: i64,
) -> Result<Vec<RecentBlockIdentity>, DbError> {
    sqlx::query_as::<_, RecentBlockIdentity>(
        "SELECT b.hash,
                w.name      AS worker_name,
                wal.address AS wallet_address,
                b.daa_score,
                b.miner_reward_sompi,
                b.found_at
           FROM block b
           JOIN worker w   ON w.id   = b.finder_worker_id
           JOIN wallet wal ON wal.id = b.finder_wallet_id
          ORDER BY b.id DESC
          LIMIT $1",
    )
    .bind(limit)
    .fetch_all(executor)
    .await
    .map_err(DbError::from)
}

/// Total number of blocks the pool has found across all statuses.
pub async fn total_count<'e, E: PgExecutor<'e>>(executor: E) -> Result<i64, DbError> {
    sqlx::query_scalar::<_, i64>("SELECT count(*)::bigint FROM block")
        .fetch_one(executor)
        .await
        .map_err(DbError::from)
}

/// Count blocks grouped by lifecycle status. Only statuses with ≥ 1
/// row appear; the caller zero-fills the rest. Drives the pool-stats
/// block-lifecycle breakdown.
pub async fn count_by_status<'e, E: PgExecutor<'e>>(
    executor: E,
) -> Result<Vec<(BlockStatus, i64)>, DbError> {
    sqlx::query_as::<_, (BlockStatus, i64)>(
        "SELECT status, count(*)::bigint
           FROM block
          GROUP BY status",
    )
    .fetch_all(executor)
    .await
    .map_err(DbError::from)
}

/// List blocks in any of the listed statuses, **oldest-first**.
///
/// A bounded-`limit` sweep drains the backlog FIFO rather than starving
/// older blocks behind a churn of newer ones. The maturity tracker
/// resolves every `submitted_to_node` block it sees to a terminal state
/// (`confirmed_blue` or `orphaned`), so oldest-first selection cannot
/// head-of-line block.
pub async fn list_by_status<'e, E: PgExecutor<'e>>(
    executor: E,
    statuses: &[BlockStatus],
    limit: i64,
) -> Result<Vec<Block>, DbError> {
    sqlx::query_as::<_, Block>(
        "SELECT id, hash, finder_wallet_id, finder_worker_id, daa_score, blue_score, nonce,
                status, found_at, submitted_at, confirmed_at, matured_at,
                miner_reward_sompi, correlation_id
           FROM block
          WHERE status = ANY($1)
          ORDER BY found_at ASC
          LIMIT $2",
    )
    .bind(statuses)
    .bind(limit)
    .fetch_all(executor)
    .await
    .map_err(DbError::from)
}
