//! Tracker-only accountant runner.
//!
//! Runs the [`MaturityTracker`] against a real kaspad endpoint
//! and the new schema's `block` table. Does NOT bridge stratum
//! share events into the DB — that's the unified runner in M3d
//! (Phase 7 wiring).
//!
//! Intended for the M3c live-test runbook
//! (`docs/runbooks/15-testnet10-tracker-live.md`): the operator
//! seeds a known testnet block into the DB, runs this binary,
//! and watches the tracker advance the block through its
//! lifecycle while talking to the operator's `kaspad-tn10` node.
//!
//! ## Configuration
//!
//! All required knobs are env vars (no config file in M3c). The
//! binary fails-fast on any missing or malformed value.
//!
//! | Env var                              | Required | Default |
//! |--------------------------------------|----------|---------|
//! | `KASPAD_GRPC_URL`                    | yes      | —       |
//! | `KATPOOL_DATABASE_URL`               | yes      | —       |
//! | `KATPOOL_POOL_ADDRESS`               | yes      | —       |
//! | `KATPOOL_INSTANCE_ID`                | no       | `tracker-runner` |
//! | `KATPOOL_FEE_TOPLINE_BPS`            | no       | 75 |
//! | `KATPOOL_MATURITY_POLL_SECS`         | no       | 15 |
//! | `KATPOOL_COINBASE_MATURITY`          | no       | 1000 |
//! | `KATPOOL_WINDOW_DAA_SPAN`            | no       | 600 |
//! | `KATPOOL_MATURITY_BATCH_SIZE`        | no       | 200 |
//!
//! The pool address is the kaspa-testnet address whose coinbase
//! outputs in matured blocks count as pool revenue.

#![allow(clippy::print_stdout, clippy::print_stderr, clippy::expect_used)]

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context as _, Result};
use kaspa_addresses::Address;
use kaspa_grpc_client::GrpcClient;
use kaspa_rpc_core::notify::mode::NotificationMode;
use tokio::signal;
use tokio::sync::watch;
use tracing::{Level, info, warn};
use tracing_subscriber::EnvFilter;

use accountant::{
    AllocationEngine, FeeConfig, KaspadGrpcClient, MaturityConfig, MaturityTracker,
    StaticTierClassifier,
};
use katpool_db::{PoolConfig, build_pool};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_target(true)
        .init();

    let cfg = RunnerConfig::from_env().context("loading runner config")?;
    info!(
        kaspad = %cfg.kaspad_url,
        instance = %cfg.instance_id,
        poll_interval_secs = cfg.maturity.poll_interval.as_secs(),
        coinbase_maturity = cfg.maturity.coinbase_maturity,
        window_daa_span = cfg.maturity.window_daa_span,
        pool_addresses = ?cfg.pool_addresses.iter().map(ToString::to_string).collect::<Vec<_>>(),
        "accountant tracker runner starting"
    );

    // ---- DB pool -----------------------------------------------------
    let db = build_pool(&PoolConfig {
        url: cfg.database_url.clone(),
        min_connections: 1,
        max_connections: 8,
        application_name: format!("katpool-tracker[{}]", cfg.instance_id),
        ..PoolConfig::production("placeholder".to_owned())
    })
    .await
    .context("opening Postgres pool")?;

    // ---- kaspad gRPC client -----------------------------------------
    let grpc = GrpcClient::connect_with_args(
        NotificationMode::Direct,
        cfg.kaspad_url.clone(),
        None,           // notification subscriber
        true,           // reconnect
        None,           // override default subnet
        false,          // disable notifier
        Some(500_000),  // request timeout (ms)
        Arc::default(), // override options
    )
    .await
    .context("connecting to kaspad")?;
    let kaspad = Arc::new(KaspadGrpcClient::new(
        Arc::new(grpc),
        cfg.pool_addresses.clone(),
    ));

    // ---- accountant pipeline ----------------------------------------
    let fee =
        FeeConfig::new(cfg.fee_topline_bps).map_err(|e| anyhow::anyhow!("fee config: {e}"))?;
    let engine = Arc::new(AllocationEngine::new(
        db.clone(),
        fee,
        Arc::new(StaticTierClassifier::standard()),
        cfg.instance_id.clone(),
    ));
    let tracker = MaturityTracker::new(
        db.clone(),
        kaspad,
        engine,
        cfg.maturity,
        cfg.instance_id.clone(),
    );

    // ---- shutdown plumbing ------------------------------------------
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let signal_task = tokio::spawn(async move {
        tokio::select! {
            res = signal::ctrl_c() => {
                if res.is_ok() { info!("SIGINT received"); }
            }
            () = sigterm() => info!("SIGTERM received"),
        }
        if shutdown_tx.send(true).is_err() {
            warn!("shutdown channel closed before signal arrived");
        }
    });

    // ---- run --------------------------------------------------------
    let tracker_result = tracker.run_loop(shutdown_rx).await;
    if let Err(e) = tracker_result {
        // tracker exited with an error (rare; only on a hard
        // panic, since the loop catches transient sweep errors).
        anyhow::bail!("tracker exited with error: {e}");
    }
    // Cancel signal task in case shutdown came from a different
    // path; it'll see the channel closed.
    signal_task.abort();
    let _ = signal_task.await;
    info!("tracker runner exiting cleanly");
    Ok(())
}

async fn sigterm() {
    // tokio's signal::unix is gated behind cfg(unix). On Linux
    // hosts (our target) it's always available.
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        if let Ok(mut sig) = signal(SignalKind::terminate()) {
            sig.recv().await;
        }
    }
    #[cfg(not(unix))]
    {
        // On non-Unix targets we don't have SIGTERM; just wait
        // forever so the SIGINT branch wins.
        std::future::pending::<()>().await;
    }
}

#[derive(Debug)]
struct RunnerConfig {
    kaspad_url: String,
    database_url: String,
    pool_addresses: Vec<Address>,
    instance_id: String,
    fee_topline_bps: u16,
    maturity: MaturityConfig,
}

impl RunnerConfig {
    fn from_env() -> Result<Self> {
        let kaspad_url = required("KASPAD_GRPC_URL")?;
        let database_url = required("KATPOOL_DATABASE_URL")?;
        let pool_address_raw = required("KATPOOL_POOL_ADDRESS")?;
        let pool_addresses = pool_address_raw
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| {
                Address::try_from(s)
                    .map_err(|e| anyhow::anyhow!("KATPOOL_POOL_ADDRESS entry `{s}`: {e}"))
            })
            .collect::<Result<Vec<_>>>()?;
        if pool_addresses.is_empty() {
            anyhow::bail!("KATPOOL_POOL_ADDRESS produced an empty list");
        }
        let instance_id =
            optional("KATPOOL_INSTANCE_ID").unwrap_or_else(|| "tracker-runner".to_owned());
        let fee_topline_bps = optional_u16("KATPOOL_FEE_TOPLINE_BPS")?.unwrap_or(75);
        let poll_secs = optional_u64("KATPOOL_MATURITY_POLL_SECS")?.unwrap_or(15);
        let coinbase_maturity = optional_u64("KATPOOL_COINBASE_MATURITY")?.unwrap_or(1000);
        let window_daa_span = optional_u64("KATPOOL_WINDOW_DAA_SPAN")?.unwrap_or(600);
        let batch_size = optional_i64("KATPOOL_MATURITY_BATCH_SIZE")?.unwrap_or(200);
        let coinbase_min_daa_score = optional_u64("KATPOOL_COINBASE_MIN_DAA_SCORE")?.unwrap_or(0);
        Ok(Self {
            kaspad_url,
            database_url,
            pool_addresses,
            instance_id,
            fee_topline_bps,
            maturity: MaturityConfig {
                poll_interval: Duration::from_secs(poll_secs),
                coinbase_maturity,
                window_daa_span,
                batch_size,
                coinbase_min_daa_score,
            },
        })
    }
}

fn required(var: &str) -> Result<String> {
    std::env::var(var).map_err(|_| anyhow::anyhow!("required env var {var} unset"))
}

fn optional(var: &str) -> Option<String> {
    std::env::var(var).ok().filter(|s| !s.is_empty())
}

fn optional_u16(var: &str) -> Result<Option<u16>> {
    optional(var)
        .map(|s| {
            s.parse::<u16>()
                .map_err(|e| anyhow::anyhow!("{var}=`{s}`: {e}"))
        })
        .transpose()
}

fn optional_u64(var: &str) -> Result<Option<u64>> {
    optional(var)
        .map(|s| {
            s.parse::<u64>()
                .map_err(|e| anyhow::anyhow!("{var}=`{s}`: {e}"))
        })
        .transpose()
}

fn optional_i64(var: &str) -> Result<Option<i64>> {
    optional(var)
        .map(|s| {
            s.parse::<i64>()
                .map_err(|e| anyhow::anyhow!("{var}=`{s}`: {e}"))
        })
        .transpose()
}

// Unused but referenced by tracing_subscriber's `with_max_level`
// in some configurations; declared at the bottom so the compiler
// doesn't warn about unused. (Workspace lints deny unused.)
#[allow(dead_code)]
const _MAX_LOG_LEVEL: Level = Level::INFO;
