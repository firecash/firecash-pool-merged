//! Core domain types for katpool.
//!
//! This crate is the lowest layer in the workspace. It contains only pure,
//! deterministic types — no I/O, no async, no global state. Every other crate
//! depends on it.
//!
//! Newtype rationale: every domain primitive is wrapped to prevent confusion
//! at call sites where a "raw string" or "raw u64" could mean any of several
//! semantically distinct things (a wallet address vs. a worker name; a DAA
//! score vs. a sompi count). Every wrapper has a validating constructor that
//! returns a typed error on bad input, and `Display`/`Debug` impls that
//! produce safe-to-log output.

#![cfg_attr(not(test), warn(missing_docs))]
// Tests use `expect`/`unwrap` and assertion macros that look like
// `panic` to clippy; relaxing those lints under `cfg(test)` keeps the
// workspace-wide strict policy intact for production code paths.
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

/// Crate version constant, useful for diagnostic reporting and logging.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod address;
pub mod correlation_id;
pub mod difficulty;
pub mod events;
pub mod hash;
pub mod redact;
pub mod score;
pub mod worker;

pub use address::{AddressError, WalletAddress};
pub use correlation_id::CorrelationId;
pub use difficulty::{DifficultyError, ShareDifficulty};
pub use events::{PoolEvent, ShareRejectReason};
pub use hash::{BlockHash, BlockHashError};
pub use score::DaaScore;
pub use worker::{WorkerName, WorkerNameError};
