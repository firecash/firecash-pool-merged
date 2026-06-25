//! Typed errors for the katpool database layer.
//!
//! Service crates never see raw `sqlx::Error`. They see a
//! [`DbError`] that classifies the failure by *what the caller can do
//! about it* — retryable transient (connection lost, deadlock),
//! permanent (unique-violation, FK-violation), or programmer error
//! (schema mismatch, type-conversion failure).
//!
//! The classification matters because the upstream retry / circuit-
//! breaker logic in `accountant` and `payout-*` needs different
//! handling per class.

use std::time::Duration;

use thiserror::Error;

/// Top-level error type for every `katpool-db` operation.
#[derive(Debug, Error)]
pub enum DbError {
    /// Cannot acquire a connection or the connection broke mid-query.
    /// Callers should back off and retry; if it persists, fail open
    /// (page the on-call).
    #[error("connection error: {source}")]
    Connection {
        /// Underlying sqlx error.
        #[source]
        source: sqlx::Error,
    },

    /// The query failed for a domain reason (unique violation, FK
    /// violation, check-constraint violation). Caller's responsibility
    /// to interpret. Includes the postgres SQLSTATE code for
    /// classification.
    #[error("query rejected by postgres (sqlstate {sqlstate:?}): {source}")]
    Constraint {
        /// 5-character SQLSTATE code from postgres.
        sqlstate: Option<String>,
        /// Underlying sqlx error.
        #[source]
        source: sqlx::Error,
    },

    /// Row-not-found where the caller required exactly one row.
    /// Distinct from `Constraint` so the typical "row missing, retry
    /// later" pattern can be handled without inspecting strings.
    #[error("row not found")]
    NotFound,

    /// Migration failed to apply. Fatal: refuse to start.
    #[error("migration failed: {source}")]
    Migration {
        /// Underlying sqlx migrate error.
        #[source]
        source: sqlx::migrate::MigrateError,
    },

    /// Configuration error (invalid `DATABASE_URL`, missing env, etc.).
    /// Caller should surface this to operator with actionable detail.
    #[error("database configuration error: {message}")]
    Config {
        /// Human-readable message.
        message: String,
    },

    /// Pool acquisition timed out. Either the pool is exhausted (legit
    /// hot-spot — add capacity) or every connection is stuck (operator
    /// issue — investigate kaspad / postgres latency).
    #[error("connection pool acquire timed out after {elapsed:?}")]
    AcquireTimeout {
        /// Wall-clock waited before timeout.
        elapsed: Duration,
    },

    /// Any other sqlx error we have not classified yet. Used sparingly;
    /// every new variant we add to handle a previously-unclassified case
    /// is a small win in operator-debuggability.
    #[error("database error: {source}")]
    Other {
        /// Underlying sqlx error.
        #[source]
        source: sqlx::Error,
    },
}

impl DbError {
    /// `true` if the error is worth retrying after a brief back-off.
    /// Callers in `accountant` use this to drive retry loops.
    #[must_use]
    pub const fn is_transient(&self) -> bool {
        matches!(self, Self::Connection { .. } | Self::AcquireTimeout { .. })
    }

    /// `true` if the error means the row-set the caller expected to
    /// find is empty. Distinct from "operation failed" — frequently a
    /// non-error in the calling logic (first-time read).
    #[must_use]
    pub const fn is_not_found(&self) -> bool {
        matches!(self, Self::NotFound)
    }

    /// SQLSTATE code if this is a constraint violation; otherwise
    /// `None`. Lets callers distinguish unique-violation (`23505`)
    /// from FK-violation (`23503`) without parsing message strings.
    #[must_use]
    pub fn sqlstate(&self) -> Option<&str> {
        match self {
            Self::Constraint { sqlstate, .. } => sqlstate.as_deref(),
            _ => None,
        }
    }
}

impl From<sqlx::Error> for DbError {
    fn from(value: sqlx::Error) -> Self {
        match &value {
            sqlx::Error::RowNotFound => Self::NotFound,
            sqlx::Error::Io(_) | sqlx::Error::Tls(_) | sqlx::Error::PoolClosed => {
                Self::Connection { source: value }
            }
            sqlx::Error::PoolTimedOut => Self::AcquireTimeout {
                elapsed: Duration::ZERO,
            },
            sqlx::Error::Database(db_err) => {
                let sqlstate = db_err.code().map(std::borrow::Cow::into_owned);
                Self::Constraint {
                    sqlstate,
                    source: value,
                }
            }
            _ => Self::Other { source: value },
        }
    }
}

impl From<sqlx::migrate::MigrateError> for DbError {
    fn from(source: sqlx::migrate::MigrateError) -> Self {
        Self::Migration { source }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_not_found_classifies() {
        let e: DbError = sqlx::Error::RowNotFound.into();
        assert!(e.is_not_found());
        assert!(!e.is_transient());
    }

    #[test]
    fn pool_timed_out_is_transient() {
        let e: DbError = sqlx::Error::PoolTimedOut.into();
        assert!(e.is_transient());
        assert!(!e.is_not_found());
    }

    #[test]
    fn pool_closed_is_connection_transient() {
        let e: DbError = sqlx::Error::PoolClosed.into();
        assert!(e.is_transient());
        assert!(matches!(e, DbError::Connection { .. }));
    }

    #[test]
    fn config_error_is_neither_transient_nor_not_found() {
        let e = DbError::Config {
            message: "bad url".to_owned(),
        };
        assert!(!e.is_transient());
        assert!(!e.is_not_found());
    }
}
