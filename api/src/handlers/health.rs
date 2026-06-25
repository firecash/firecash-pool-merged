//! Liveness/readiness probes. Unversioned (orchestrators expect stable
//! top-level paths) and uncached (must reflect live state).
//!
//! - `GET /health`  — liveness: the process is up. Always `200`.
//! - `GET /ready`   — readiness: DB reachable **and** kaspad synced.
//!   `200` when ready, else `503`.
//! - `GET /started` — startup: initial sync completed at least once.
//!   `200` when started, else `503`.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;

use crate::models::{Health, Readiness, Started};
use crate::state::AppState;

/// Crate version, surfaced in `/health`.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// `GET /health` — always `200`; proves the listener is alive.
pub async fn health() -> Json<Health> {
    Json(Health {
        status: "ok",
        version: VERSION,
    })
}

/// `GET /ready` — `200` only when the pool can actually serve data.
pub async fn ready(State(state): State<AppState>) -> (StatusCode, Json<Readiness>) {
    let body = Readiness {
        ready: state.readiness.is_ready(),
        db_reachable: state.readiness.db_reachable(),
        kaspad_synced: state.readiness.kaspad_synced(),
    };
    let status = if body.ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (status, Json(body))
}

/// `GET /started` — `200` once initial startup has completed at least once.
pub async fn started(State(state): State<AppState>) -> (StatusCode, Json<Started>) {
    let started = state.readiness.is_started();
    let uptime_secs = (chrono::Utc::now() - state.started_at).num_seconds();
    let body = Started {
        started,
        started_at: state.started_at,
        uptime_secs,
    };
    let status = if started {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (status, Json(body))
}
