//! Shared application state: the DB pool, readiness flags, the two TTL
//! caches, and process start time.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::{DateTime, Utc};
use moka::future::Cache;
use serde_json::Value;
use sqlx::PgPool;

use crate::config::{ApiConfig, CACHE_MAX_ENTRIES};

/// A bounded TTL cache of pre-serialized JSON responses keyed by
/// route + normalized params. Storing `serde_json::Value` lets one cache
/// serve heterogeneous endpoints; `Arc` makes hits clone-free.
pub type JsonCache = Cache<String, Arc<Value>>;

/// Liveness/readiness flags, shared between the runtime (which sets them)
/// and the API handlers (which read them). Cheap atomics — no locking on
/// the health-probe hot path.
///
/// - `db_reachable`: a recent `SELECT 1` succeeded.
/// - `kaspad_synced`: kaspad reports synced (driven by the maturity poller).
/// - `started`: the initial sync/backfill completed at least once (latched).
#[derive(Clone, Default)]
pub struct ReadinessHandle {
    inner: Arc<ReadinessInner>,
}

#[derive(Default)]
struct ReadinessInner {
    db_reachable: AtomicBool,
    kaspad_synced: AtomicBool,
    started: AtomicBool,
}

impl ReadinessHandle {
    /// A fresh handle with every flag `false`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record whether the database is currently reachable.
    pub fn set_db_reachable(&self, value: bool) {
        self.inner.db_reachable.store(value, Ordering::Relaxed);
    }

    /// Record whether kaspad currently reports synced.
    pub fn set_kaspad_synced(&self, value: bool) {
        self.inner.kaspad_synced.store(value, Ordering::Relaxed);
    }

    /// Latch the "initial startup complete" flag (idempotent).
    pub fn mark_started(&self) {
        self.inner.started.store(true, Ordering::Relaxed);
    }

    /// Is the database currently reachable?
    #[must_use]
    pub fn db_reachable(&self) -> bool {
        self.inner.db_reachable.load(Ordering::Relaxed)
    }

    /// Does kaspad currently report synced?
    #[must_use]
    pub fn kaspad_synced(&self) -> bool {
        self.inner.kaspad_synced.load(Ordering::Relaxed)
    }

    /// Has initial startup completed at least once?
    #[must_use]
    pub fn is_started(&self) -> bool {
        self.inner.started.load(Ordering::Relaxed)
    }

    /// Ready to serve data: DB reachable **and** kaspad synced.
    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.db_reachable() && self.kaspad_synced()
    }
}

/// Cloneable handle to everything a handler needs. `axum` clones this per
/// request; every field is `Arc`/pool-backed so the clone is cheap.
#[derive(Clone)]
pub struct AppState {
    /// Shared read-only access to the database (clone of the runtime pool).
    pub pool: PgPool,
    /// Readiness flags, written by the runtime.
    pub readiness: ReadinessHandle,
    /// Immutable configuration.
    pub config: Arc<ApiConfig>,
    /// Cache for pool-wide aggregates/series.
    pub pool_cache: JsonCache,
    /// Cache for per-wallet reads (shorter TTL).
    pub wallet_cache: JsonCache,
    /// Process start time, for `/started` and uptime.
    pub started_at: DateTime<Utc>,
}

impl AppState {
    /// Build state from a pool, readiness handle, and config, materializing
    /// the two TTL caches from the config's TTLs.
    #[must_use]
    pub fn new(pool: PgPool, readiness: ReadinessHandle, config: ApiConfig) -> Self {
        let pool_cache = Cache::builder()
            .max_capacity(CACHE_MAX_ENTRIES)
            .time_to_live(config.pool_cache_ttl)
            .build();
        let wallet_cache = Cache::builder()
            .max_capacity(CACHE_MAX_ENTRIES)
            .time_to_live(config.wallet_cache_ttl)
            .build();
        Self {
            pool,
            readiness,
            config: Arc::new(config),
            pool_cache,
            wallet_cache,
            started_at: Utc::now(),
        }
    }
}
