//! HTTP handlers, grouped by surface: [`health`] (liveness/readiness),
//! [`pool`] (pool-wide aggregates), and [`miner`] (per-wallet reads).
//!
//! Cached handlers compute a typed response, serialize it to a
//! `serde_json::Value`, and memoize that behind the appropriate TTL cache
//! (pool-wide vs per-wallet). Failures are never cached.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

pub mod health;
pub mod miner;
pub mod pool;

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::error::ApiError;
use crate::state::JsonCache;

/// A resolved sliding window: its start, end (now), and length in seconds.
#[derive(Debug, Clone, Copy)]
struct Window {
    since: DateTime<Utc>,
    until: DateTime<Utc>,
    secs: u64,
}

/// Resolve a [`Duration`] window into a `[since, now]` instant pair.
fn resolve_window(window: Duration) -> Window {
    let until = Utc::now();
    let secs = window.as_secs();
    let since = until - chrono::Duration::seconds(secs as i64);
    Window { since, until, secs }
}

/// Serialize a typed response to JSON, mapping failures to a 500.
fn to_value<T: serde::Serialize>(value: &T) -> Result<Value, ApiError> {
    serde_json::to_value(value).map_err(|err| {
        tracing::error!(error = %err, "response serialization failed");
        ApiError::Internal
    })
}

/// Memoize a JSON response behind `cache` under `key`, computing it via
/// `init` on a miss. Returns the axum `Json` envelope over the shared `Arc`,
/// so cache hits don't re-serialize.
///
/// `cache` is a cheap clone of one of the [`AppState`](crate::state::AppState)
/// caches; the caller clones it before moving `state` into `init`, so the
/// init future can own everything it needs without fighting the borrow
/// checker. Failures are never cached (moka's `try_get_with` contract).
async fn cached_json<Fut>(
    cache: &JsonCache,
    key: String,
    init: Fut,
) -> Result<Json<Arc<Value>>, ApiError>
where
    Fut: Future<Output = Result<Value, ApiError>>,
{
    let value = cache
        .try_get_with(key, async move { init.await.map(Arc::new) })
        .await
        .map_err(|arc| ApiError::from_cache_err(&arc))?;
    Ok(Json(value))
}
