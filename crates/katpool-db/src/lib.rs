//! `PostgreSQL` access layer for katpool.
//!
//! Owns the schema (via sqlx migrations under `migrations/`) and the
//! repository traits that the service crates depend on. Every public
//! function returns a typed [`error::DbError`]; the service crates
//! never see raw `sqlx::Error`.
//!
//! ## Surface map
//!
//! - [`PoolConfig`] / [`build_pool`] — connection pool construction
//!   and operator-tunable knobs via the `KATPOOL_DB_*` env vars.
//! - [`migrate::run`] — apply every pending migration in order.
//!   Called from the binary's startup path; refuses to start if a
//!   migration fails.
//! - [`error::DbError`] — typed error surface with classification
//!   helpers ([`error::DbError::is_transient`],
//!   [`error::DbError::is_not_found`], [`error::DbError::sqlstate`]).
//!
//! Repository traits (`WalletRepo`, `BlockRepo`, …) land in Phase 2
//! milestone 2.

#![cfg_attr(not(test), warn(missing_docs))]
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing,
        clippy::float_arithmetic,
    )
)]

/// Crate version constant.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod error;
pub mod migrate;
pub mod pool;
pub mod repo;

pub use error::DbError;
pub use pool::{PoolConfig, build_pool};

/// Re-export of the embedded migrator so callers can introspect or
/// run migrations without depending on `sqlx::migrate::Migrator`
/// directly.
pub use migrate::MIGRATIONS;
