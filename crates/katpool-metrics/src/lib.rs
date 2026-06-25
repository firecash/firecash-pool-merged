//! Prometheus registry helpers used by every service crate.
//!
//! Enforces the operating principle that high-cardinality labels (per-wallet,
//! per-IP) never appear on hot metrics. Helper macros and builders that fail
//! to compile (or at runtime in test mode) if a forbidden cardinality slips in.
//!
//! Implemented in Phase 1 (bridge metrics) and refined in Phase 3 (accountant).

#![cfg_attr(not(test), warn(missing_docs))]

use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use prometheus::{CounterVec, GaugeVec, register_counter_vec, register_gauge_vec};

/// Crate version constant.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

// ---- payout / treasury metrics (B7) ---------------------------------------
//
// These register into the process-global Prometheus default registry, the same
// one the bridge's `/metrics` exporter gathers (`bridge/src/prom.rs`). Each
// carries an `instance` label so the exporter's instance filter keeps them. The
// payout engines (`payout-kas`, `payout-krc20`) and the consolidation engine
// feed them through the primitive-typed `record_*` / `set_*` helpers below, so
// this low-level crate stays decoupled from the payout domain types.

/// `ks_payout_cycles_total{instance, engine, status}` — payout cycles observed,
/// keyed by engine (`kas` / `krc20`) and terminal cycle status (the
/// `PayoutCycleStatus` snake-case label, plus `error` for a failed tick).
static PAYOUT_CYCLES_TOTAL: OnceLock<CounterVec> = OnceLock::new();

/// `ks_payout_last_success_timestamp_seconds{instance, engine}` — Unix time of
/// the last cycle that made forward progress (settled, fully or partially).
/// Powers dashboards and a "payouts have stalled" signal without paging on
/// legitimately idle cycles (the canary miner is the end-to-end ground truth).
static PAYOUT_LAST_SUCCESS: OnceLock<GaugeVec> = OnceLock::new();

/// `ks_treasury_balance_sompi{instance}` — spendable treasury balance (sompi)
/// from the most recent consolidation snapshot.
static TREASURY_BALANCE_SOMPI: OnceLock<GaugeVec> = OnceLock::new();

/// `ks_treasury_spendable_utxos{instance}` — spendable treasury UTXO count from
/// the most recent consolidation snapshot (fragmentation indicator).
static TREASURY_SPENDABLE_UTXOS: OnceLock<GaugeVec> = OnceLock::new();

/// Register the payout/treasury metrics into the global registry.
///
/// Call once at startup when the Prometheus exporter is enabled. Idempotent and
/// best-effort: a registration failure (e.g. a duplicate registration) is
/// logged and leaves the metric absent rather than aborting the process —
/// telemetry must never be the reason a node fails to boot.
pub fn init_payout_metrics() {
    fn store_counter(cell: &OnceLock<CounterVec>, name: &str, help: &str, labels: &[&str]) {
        match register_counter_vec!(name, help, labels) {
            Ok(metric) => {
                let _ = cell.set(metric);
            }
            Err(e) => tracing::warn!(metric = name, error = %e, "failed to register metric"),
        }
    }
    fn store_gauge(cell: &OnceLock<GaugeVec>, name: &str, help: &str, labels: &[&str]) {
        match register_gauge_vec!(name, help, labels) {
            Ok(metric) => {
                let _ = cell.set(metric);
            }
            Err(e) => tracing::warn!(metric = name, error = %e, "failed to register metric"),
        }
    }

    store_counter(
        &PAYOUT_CYCLES_TOTAL,
        "ks_payout_cycles_total",
        "Payout cycles observed, by engine and terminal status",
        &["instance", "engine", "status"],
    );
    store_gauge(
        &PAYOUT_LAST_SUCCESS,
        "ks_payout_last_success_timestamp_seconds",
        "Unix timestamp of the last payout cycle that settled (fully or partially), by engine",
        &["instance", "engine"],
    );
    store_gauge(
        &TREASURY_BALANCE_SOMPI,
        "ks_treasury_balance_sompi",
        "Spendable treasury balance (sompi) from the latest consolidation snapshot",
        &["instance"],
    );
    store_gauge(
        &TREASURY_SPENDABLE_UTXOS,
        "ks_treasury_spendable_utxos",
        "Spendable treasury UTXO count from the latest consolidation snapshot",
        &["instance"],
    );
}

/// Record one completed payout cycle for `engine` (`kas` / `krc20`) with its
/// terminal `status` label. No-op until [`init_payout_metrics`] has run.
pub fn record_payout_cycle(instance: &str, engine: &str, status: &str) {
    if let Some(counter) = PAYOUT_CYCLES_TOTAL.get() {
        counter.with_label_values(&[instance, engine, status]).inc();
    }
}

/// Mark a successful (settled) payout cycle for `engine`: sets the
/// last-success gauge to the current Unix time. No-op until initialized.
pub fn mark_payout_success(instance: &str, engine: &str) {
    if let Some(gauge) = PAYOUT_LAST_SUCCESS.get() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
        gauge.with_label_values(&[instance, engine]).set(now);
    }
}

/// Publish the latest treasury snapshot totals (spendable balance in sompi and
/// spendable UTXO count). No-op until initialized.
#[allow(
    clippy::cast_precision_loss,
    reason = "Prometheus gauges are f64; sompi balances and UTXO counts stay far below 2^53, so the cast is exact"
)]
pub fn set_treasury_snapshot(instance: &str, balance_sompi: i64, spendable_utxos: i64) {
    if let Some(gauge) = TREASURY_BALANCE_SOMPI.get() {
        gauge
            .with_label_values(&[instance])
            .set(balance_sompi as f64);
    }
    if let Some(gauge) = TREASURY_SPENDABLE_UTXOS.get() {
        gauge
            .with_label_values(&[instance])
            .set(spendable_utxos as f64);
    }
}
