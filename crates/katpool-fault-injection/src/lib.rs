//! Test-only chaos primitives used by the chaos test suite and load harness.
//!
//! Nothing in this crate is compiled into release binaries (the crate is
//! `cfg(any(test, feature = "enabled"))`-gated where appropriate).

#![cfg_attr(not(test), warn(missing_docs))]

/// Crate version constant.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
