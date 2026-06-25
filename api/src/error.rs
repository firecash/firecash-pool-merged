//! API error model and HTTP mapping.
//!
//! Handlers return `Result<T, ApiError>`. `ApiError` is a small, `Clone`
//! classification (never a raw `sqlx`/`DbError`) so it can flow through the
//! `moka` cache's `try_get_with` and be rendered into a single stable JSON
//! envelope. Internal detail is logged with a redacted address but never
//! leaked to the client (`docs/threat-model.md`).

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use katpool_db::DbError;
use serde::Serialize;

/// Postgres SQLSTATE for a statement cancelled by `statement_timeout`.
const SQLSTATE_QUERY_CANCELED: &str = "57014";

/// A classified, client-safe API error.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ApiError {
    /// The requested resource does not exist (404).
    #[error("not found")]
    NotFound,
    /// The request was malformed — bad address, out-of-range query
    /// parameter, etc. (400). The message is safe to return.
    #[error("{0}")]
    BadRequest(String),
    /// An upstream dependency (database, kaspad) is unavailable; the
    /// caller should retry (503).
    #[error("service unavailable")]
    Unavailable,
    /// The query exceeded its time budget; surfaced as 503 with a
    /// distinct `timeout` code so the caller can retry (ADR-0021).
    #[error("query timed out")]
    Timeout,
    /// An unexpected internal error (500). The detail is logged, never
    /// returned.
    #[error("internal error")]
    Internal,
}

impl ApiError {
    /// Construct a `BadRequest` from any displayable value.
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self::BadRequest(msg.into())
    }

    /// The HTTP status this error maps to.
    #[must_use]
    pub const fn status(&self) -> StatusCode {
        match self {
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Unavailable | Self::Timeout => StatusCode::SERVICE_UNAVAILABLE,
            Self::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Stable machine-readable error code for the JSON envelope.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::NotFound => "not_found",
            Self::BadRequest(_) => "bad_request",
            Self::Unavailable => "unavailable",
            Self::Timeout => "timeout",
            Self::Internal => "internal",
        }
    }

    /// Recover an owned `ApiError` from the `Arc` that `moka`'s
    /// `try_get_with` hands back on a cache-miss failure.
    #[must_use]
    pub fn from_cache_err(err: &std::sync::Arc<Self>) -> Self {
        (**err).clone()
    }
}

/// Map a [`DbError`] to a client-safe [`ApiError`], collapsing the rich
/// internal taxonomy into the four externally meaningful classes.
impl From<DbError> for ApiError {
    fn from(err: DbError) -> Self {
        match &err {
            DbError::NotFound => Self::NotFound,
            DbError::Connection { .. } | DbError::AcquireTimeout { .. } => Self::Unavailable,
            DbError::Constraint { sqlstate, .. }
                if sqlstate.as_deref() == Some(SQLSTATE_QUERY_CANCELED) =>
            {
                Self::Timeout
            }
            // A `Config` error here means a programmer passed an invalid
            // window/range to a repo guard — surface as a 400.
            DbError::Config { message } => Self::BadRequest(message.clone()),
            DbError::Constraint { .. } | DbError::Migration { .. } | DbError::Other { .. } => {
                tracing::error!(error = %err, "unclassified database error");
                Self::Internal
            }
        }
    }
}

/// The JSON envelope returned for every error.
#[derive(Debug, Serialize)]
struct ErrorBody<'a> {
    error: ErrorDetail<'a>,
}

#[derive(Debug, Serialize)]
struct ErrorDetail<'a> {
    code: &'a str,
    message: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.status();
        if status == StatusCode::INTERNAL_SERVER_ERROR {
            tracing::error!(error = %self, "internal API error");
        }
        let body = ErrorBody {
            error: ErrorDetail {
                code: self.code(),
                message: self.to_string(),
            },
        };
        (status, Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn db_not_found_maps_to_404() {
        assert_eq!(ApiError::from(DbError::NotFound), ApiError::NotFound);
    }

    #[test]
    fn db_connection_maps_to_unavailable() {
        let e = ApiError::from(DbError::AcquireTimeout {
            elapsed: Duration::from_secs(1),
        });
        assert_eq!(e, ApiError::Unavailable);
        assert_eq!(e.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[test]
    fn statement_timeout_maps_to_503_timeout() {
        let e = ApiError::from(DbError::Constraint {
            sqlstate: Some(SQLSTATE_QUERY_CANCELED.to_owned()),
            source: sqlx::Error::PoolClosed,
        });
        assert_eq!(e, ApiError::Timeout);
        assert_eq!(e.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(e.code(), "timeout");
    }

    #[test]
    fn config_error_maps_to_bad_request() {
        let e = ApiError::from(DbError::Config {
            message: "until must be after from".to_owned(),
        });
        assert!(matches!(e, ApiError::BadRequest(_)));
        assert_eq!(e.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn codes_are_stable() {
        assert_eq!(ApiError::NotFound.code(), "not_found");
        assert_eq!(ApiError::Timeout.code(), "timeout");
    }
}
