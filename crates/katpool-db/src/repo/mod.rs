//! Repository layer — typed queries over the schema introduced by
//! [`crate::migrate`].
//!
//! ## Shape
//!
//! Queries are organised by *aggregate* (the schema table family that
//! shares an FK ownership root). Each aggregate gets a module with
//! free functions that take an `impl sqlx::PgExecutor<'_>`. That lets
//! callers pass either a `&PgPool` for single-statement contexts or a
//! `&mut sqlx::Transaction<'_, Postgres>` (via `&mut *tx`) when atomic
//! multi-statement work is needed:
//!
//! ```no_run
//! # async fn example(pool: &sqlx::PgPool) -> Result<(), katpool_db::DbError> {
//! use katpool_db::repo::{wallet, worker};
//! use katpool_domain::{WalletAddress, WorkerName};
//!
//! let wallet_addr =
//!     WalletAddress::new("kaspa:qz4j8mu269z8llgcczmfukm9fan2fq822kzxu4cfukd5fq").unwrap();
//! let worker_name = WorkerName::new("rig-01").unwrap();
//!
//! // Atomic: wallet + worker upsert in one transaction.
//! let mut tx = pool.begin().await.map_err(katpool_db::DbError::from)?;
//! let w = wallet::ensure(&mut *tx, &wallet_addr, "mainnet").await?;
//! let _ = worker::ensure(&mut *tx, w.id, &worker_name).await?;
//! tx.commit().await.map_err(katpool_db::DbError::from)?;
//! # Ok(()) }
//! ```
//!
//! ## Why not a `Repository` trait?
//!
//! The trait-object pattern (`Box<dyn Repository>`) doesn't earn its
//! complexity here. Real-DB integration tests via testcontainers are
//! cheap enough that mock impls would only slow things down. Free
//! functions keep the code simple; the trait pattern is available if
//! we ever need it (the function signatures are stable enough that
//! retrofitting one is a mechanical refactor, not a redesign).
//!
//! ## SQL-checking strategy
//!
//! Queries use the **runtime-checked** `sqlx::query` /
//! `sqlx::query_as` constructors, not the compile-time-checked
//! `sqlx::query!` / `sqlx::query_as!` macros. Reasoning:
//!
//! - The macros require a live `DATABASE_URL` at build time, or a
//!   committed `.sqlx/` offline cache. Both add CI complexity that
//!   we don't need until repo-layer churn is higher.
//! - The integration tests in `tests/repo_*.rs` exercise every query
//!   shipped here against a real postgres, so the safety gap vs the
//!   macros is "fail at test run" instead of "fail at build". For a
//!   schema this stable, that's fine.
//!
//! Upgrade to the compile-time-checked macros + offline cache lands
//! as a Phase 9 / pre-release hardening task.

pub mod audit;
pub mod block;
pub mod coinbase_reward;
pub mod connection_session;
pub mod nacho_rebate;
pub mod payout;
pub mod pool_meta;
pub mod share;
pub mod share_allocation;
pub mod share_reject;
pub mod share_stats;
pub mod share_window;
pub mod treasury;
pub mod wallet;
pub mod worker;

/// Strongly-typed wrapper around a `wallet.id` value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(transparent)]
pub struct WalletId(pub i64);

/// Strongly-typed wrapper around a `worker.id` value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(transparent)]
pub struct WorkerId(pub i64);

/// Strongly-typed wrapper around a `connection_session.id` value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(transparent)]
pub struct SessionId(pub i64);

/// Strongly-typed wrapper around a `share.id` value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(transparent)]
pub struct ShareId(pub i64);

/// Strongly-typed wrapper around a `block.id` value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(transparent)]
pub struct BlockId(pub i64);

/// Strongly-typed wrapper around a `coinbase_reward.id` value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(transparent)]
pub struct CoinbaseRewardId(pub i64);

/// Strongly-typed wrapper around an `audit_log.id` value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(transparent)]
pub struct AuditLogId(pub i64);

/// Strongly-typed wrapper around a `share_reject.id` value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(transparent)]
pub struct ShareRejectId(pub i64);
