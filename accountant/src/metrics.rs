//! Prometheus instrumentation for the accountant.
//!
//! All counters carry an `instance` label so a single Grafana
//! dashboard can disambiguate primary vs. shadow accountant
//! instances during the Phase 7 shadow-run window. Counters never
//! reset within a process lifetime; reset semantics are managed
//! by the `up` metric the metrics crate already exports.
//!
//! Metric registration can only fail on a duplicate or malformed metric
//! name — a startup-time programming error, never a recoverable runtime
//! condition — so the `OnceLock` initializers `expect`. This mirrors the
//! bridge's `prom.rs` convention under the workspace `expect_used = "deny"`
//! policy; the blanket allow is scoped to this registration-only module.
#![allow(clippy::expect_used)]

use prometheus::{IntCounterVec, register_int_counter_vec};
use std::sync::OnceLock;

/// Total `PoolEvent`s the accountant has observed, by variant.
/// Variants: `share_credited`, `share_rejected`, `block_found`,
/// `block_accepted`.
pub fn ks_accountant_events_total() -> &'static IntCounterVec {
    static M: OnceLock<IntCounterVec> = OnceLock::new();
    M.get_or_init(|| {
        register_int_counter_vec!(
            "ks_accountant_events_total",
            "Pool events the accountant observed, by variant + instance",
            &["instance", "variant"]
        )
        .expect("register ks_accountant_events_total")
    })
}

/// Lagged events the accountant skipped.
///
/// Each tick = one `tokio::sync::broadcast::error::RecvError::Lagged`
/// result. Skipped-event count isn't labelled (it would explode
/// cardinality); operators chart `rate(...)` to detect lag bursts.
pub fn ks_accountant_events_lagged_total() -> &'static IntCounterVec {
    static M: OnceLock<IntCounterVec> = OnceLock::new();
    M.get_or_init(|| {
        register_int_counter_vec!(
            "ks_accountant_events_lagged_total",
            "Broadcast channel lag events the accountant skipped",
            &["instance"]
        )
        .expect("register ks_accountant_events_lagged_total")
    })
}

/// Per-event-kind handler errors. The accountant logs and counts;
/// it does not abort the consumer task.
pub fn ks_accountant_event_errors_total() -> &'static IntCounterVec {
    static M: OnceLock<IntCounterVec> = OnceLock::new();
    M.get_or_init(|| {
        register_int_counter_vec!(
            "ks_accountant_event_errors_total",
            "Event-handler errors, by variant + error kind + instance",
            &["instance", "variant", "kind"]
        )
        .expect("register ks_accountant_event_errors_total")
    })
}

/// Successful share inserts (DB write completed).
pub fn ks_accountant_share_inserts_total() -> &'static IntCounterVec {
    static M: OnceLock<IntCounterVec> = OnceLock::new();
    M.get_or_init(|| {
        register_int_counter_vec!(
            "ks_accountant_share_inserts_total",
            "Share rows written, by instance",
            &["instance"]
        )
        .expect("register ks_accountant_share_inserts_total")
    })
}

/// Block lifecycle transitions, labelled `found` / `submitted` /
/// `dup_found` (the new-or-existing distinguisher from `block::ensure`).
pub fn ks_accountant_block_transitions_total() -> &'static IntCounterVec {
    static M: OnceLock<IntCounterVec> = OnceLock::new();
    M.get_or_init(|| {
        register_int_counter_vec!(
            "ks_accountant_block_transitions_total",
            "Block lifecycle transitions, by kind + instance",
            &["instance", "transition"]
        )
        .expect("register ks_accountant_block_transitions_total")
    })
}

/// Bumps the per-variant event counter.
pub fn record_event(instance: &str, variant: &str) {
    ks_accountant_events_total()
        .with_label_values(&[instance, variant])
        .inc();
}

/// Bumps the lag counter.
pub fn record_lag(instance: &str) {
    ks_accountant_events_lagged_total()
        .with_label_values(&[instance])
        .inc();
}

/// Bumps the error counter.
pub fn record_event_error(instance: &str, variant: &str, kind: &str) {
    ks_accountant_event_errors_total()
        .with_label_values(&[instance, variant, kind])
        .inc();
}

/// Bumps the share-insert counter.
pub fn record_share_insert(instance: &str) {
    ks_accountant_share_inserts_total()
        .with_label_values(&[instance])
        .inc();
}

/// Bumps the block-transition counter.
pub fn record_block_transition(instance: &str, transition: &str) {
    ks_accountant_block_transitions_total()
        .with_label_values(&[instance, transition])
        .inc();
}
