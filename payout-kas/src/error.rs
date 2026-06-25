//! Typed errors for the KAS payout engine.

use katpool_db::DbError;

/// Errors from the KAS payout cycle lifecycle (plan, resume, reconcile).
#[derive(Debug, thiserror::Error)]
pub enum PayoutKasError {
    /// Database failure.
    #[error(transparent)]
    Db(#[from] DbError),
}
