//! The KRC-20 NACHO payout engine: a single-leader periodic loop that drives
//! one rebate cycle per DAA window through plan → settle → credit → reconcile.
//!
//! Mirrors the Phase 4 KAS engine ([`payout_kas::PayoutEngine`]) and reuses its
//! safety properties:
//!
//! - **Single leader, fair to the other treasury spenders.** Each tick is
//!   guarded by a Postgres session advisory lock
//!   ([`katpool_idempotency::AdvisoryLock`]) under the *shared*
//!   `TREASURY_SPEND_LOCK_NAMESPACE` — the same lock the KAS payout and
//!   consolidation engines take, so at most one treasury spender acts at a
//!   time. Because the KAS engine ticks on the same `poll_interval` from the
//!   same startup instant, this engine **phase-staggers** its loop and waits a
//!   **bounded** interval for the lock (see [`Krc20PayoutEngine::run_loop`] /
//!   [`Krc20PayoutEngineConfig::lock_acquire_wait`]) so it defers to an
//!   in-flight payout instead of losing the race every tick and starving. The
//!   lock is leak-safe (released on connection close), so leadership always
//!   recovers — running multiple `katpool` replicas is safe.
//! - **Idempotent identity.** The cycle window comes from [`cycle_window`], so
//!   ticks inside one DAA bucket resume the same cycle
//!   ([`resume_or_plan_krc20_cycle`]); amounts/recipients never shift under an
//!   in-flight commit/reveal.
//! - **Confirmation never lags the bucket.** Construction rejects a
//!   `cycle_span_daa` that is not strictly greater than
//!   [`KAS_PAYOUT_CONFIRMATION_DAA`] (the same finality depth the executor
//!   confirms against), so a cycle's transfers always confirm before the
//!   window rolls over.
//! - **Safe-by-default.** [`ExecutionMode::DryRun`] settles without recording
//!   or broadcasting (M5.4b) and never credits; only a live tick moves funds or
//!   mutates `nacho_rebate.paid_sompi`.

use std::time::Duration;

use kaspa_addresses::Address;
use katpool_db::repo::payout::{Krc20TransferStatus, PayoutCycleStatus};
use katpool_idempotency::{AdvisoryLock, advisory_key};
use katpool_secrets::TreasurySecret;
use payout_kas::{
    ExecutionMode, KAS_PAYOUT_CONFIRMATION_DAA, KaspadClient, cycle_window, over_spend_cap,
};
use secp256k1::Keypair;
use sqlx::PgPool;
use tokio::sync::watch;
use tokio::time;
use tracing::{debug, info, warn};

use crate::cycle::{
    CreditReport, Krc20CycleError, Krc20CycleParams, credit_completed_transfers,
    reconcile_krc20_cycle_status, resume_or_plan_krc20_cycle,
};
use crate::execute::{Krc20ExecuteError, SettleReport, settle_pending};
use crate::quote::FloorPriceSource;

/// Delay between treasury-lock acquisition retries within a single tick (see
/// [`Krc20PayoutEngineConfig::lock_acquire_wait`]).
const LOCK_RETRY_INTERVAL: Duration = Duration::from_millis(500);

/// Errors from the KRC-20 payout engine.
#[derive(Debug, thiserror::Error)]
pub enum Krc20EngineError {
    /// Database / advisory-lock failure.
    #[error(transparent)]
    Db(#[from] katpool_db::DbError),

    /// Cycle planning / crediting / reconciliation failure.
    #[error(transparent)]
    Cycle(#[from] Krc20CycleError),

    /// Settlement (sign / submit / confirm) failure.
    #[error(transparent)]
    Execute(#[from] Krc20ExecuteError),

    /// kaspad RPC failure.
    #[error(transparent)]
    Kaspad(#[from] payout_kas::KaspadError),

    /// Treasury key could not be parsed into a keypair.
    #[error("treasury key: {0}")]
    Key(secp256k1::Error),

    /// `cycle_span_daa` is too small to guarantee in-window confirmation.
    #[error("cycle_span_daa ({span}) must exceed confirmation depth ({depth})")]
    SpanTooSmall {
        /// Configured span.
        span: u64,
        /// Required confirmation depth.
        depth: u64,
    },

    /// The cycle's total NACHO base units exceed the configured per-cycle cap.
    /// A money-safety stop: nothing is settled, so the NACHO stays in the
    /// treasury until an operator raises the cap or fixes the floor-price quote
    /// that inflated the amounts.
    #[error("krc20 cycle outbound {total} NACHO base units exceeds per-cycle cap {cap}")]
    SpendCapExceeded {
        /// Summed non-failed NACHO base units for the cycle.
        total: i64,
        /// Configured cap.
        cap: i64,
    },
}

/// Engine configuration. Built from runtime config / env in the binary.
#[derive(Debug, Clone)]
pub struct Krc20PayoutEngineConfig {
    /// Instance label for logs/metrics.
    pub instance_id: String,
    /// How often to attempt a tick.
    pub poll_interval: Duration,
    /// How long a tick keeps retrying the shared treasury lock before yielding
    /// this round. The KAS payout engine shares the same lock and ticks on the
    /// same `poll_interval`; this bounded wait lets a busy KAS tick merely
    /// *delay* this engine instead of starving it. Kept well under
    /// `poll_interval`.
    pub lock_acquire_wait: Duration,
    /// DAA width of one payout cycle (cadence + idempotency bucket).
    pub cycle_span_daa: u64,
    /// Live broadcast or dry-run rehearsal.
    pub mode: ExecutionMode,
    /// Advisory-lock namespace (hashed to the leader key). The **shared**
    /// `TREASURY_SPEND_LOCK_NAMESPACE`, so this engine serializes with the KAS
    /// payout and consolidation engines and never selects a UTXO concurrently
    /// with them.
    pub lock_namespace: String,
    /// Minimum pending KAS-sompi for a wallet to be selected (coarse filter).
    pub min_pending_sompi: i64,
    /// Minimum converted NACHO base units worth a reveal (dust gate).
    pub min_nacho_base_units: u128,
    /// Token ticker to quote and inscribe.
    pub ticker: String,
    /// KAS-sompi locked into each commit P2SH output.
    pub commit_amount_sompi: u64,
    /// Cap on recipients planned and transfers settled per tick.
    pub batch_limit: i64,
    /// Optional per-cycle NACHO spend cap (base units). `None` disables it.
    /// When set, a cycle whose total non-failed NACHO exceeds this is refused
    /// before any settle (money-safety circuit breaker, G1) — the primary
    /// guard against a poisoned floor-price quote inflating rebate amounts.
    pub max_nacho_base_units_per_cycle: Option<i64>,
}

impl Krc20PayoutEngineConfig {
    fn cycle_params(
        &self,
        daa_start: katpool_domain::DaaScore,
        daa_end: katpool_domain::DaaScore,
    ) -> Krc20CycleParams {
        Krc20CycleParams {
            daa_start,
            daa_end,
            min_pending_sompi: self.min_pending_sompi,
            min_nacho_base_units: self.min_nacho_base_units,
            ticker: self.ticker.clone(),
            commit_amount_sompi: self.commit_amount_sompi,
            limit: self.batch_limit,
        }
    }
}

/// Result of a single tick.
#[derive(Debug, Clone)]
pub enum Krc20TickOutcome {
    /// Another instance held the leader lock; this tick did no work.
    SkippedNotLeader,
    /// This instance was leader and ran a cycle.
    Ran(Box<Krc20TickReport>),
}

/// Details of a tick that ran.
#[derive(Debug, Clone)]
pub struct Krc20TickReport {
    /// Cycle that was driven.
    pub cycle_id: i64,
    /// Virtual DAA score observed at tick start.
    pub virtual_daa: u64,
    /// Window start (inclusive).
    pub daa_start: u64,
    /// Window end (exclusive).
    pub daa_end: u64,
    /// Settlement outcome.
    pub settle: SettleReport,
    /// Crediting outcome (empty in dry-run).
    pub credit: CreditReport,
    /// Cycle status after reconcile.
    pub status: PayoutCycleStatus,
}

/// The KRC-20 payout engine. Owns its kaspad client, treasury key, and
/// floor-price source for the life of the loop.
pub struct Krc20PayoutEngine<C: KaspadClient, Q: FloorPriceSource> {
    pool: PgPool,
    client: C,
    secret: TreasurySecret,
    treasury_address: Address,
    quote: Q,
    config: Krc20PayoutEngineConfig,
    lock_key: i64,
}

impl<C, Q> Krc20PayoutEngine<C, Q>
where
    C: KaspadClient + Sync,
    Q: FloorPriceSource,
{
    /// Build an engine, validating the span invariant.
    ///
    /// # Errors
    ///
    /// [`Krc20EngineError::SpanTooSmall`] if `cycle_span_daa` does not exceed
    /// the confirmation depth.
    pub fn new(
        pool: PgPool,
        client: C,
        secret: TreasurySecret,
        treasury_address: Address,
        quote: Q,
        config: Krc20PayoutEngineConfig,
    ) -> Result<Self, Krc20EngineError> {
        if config.cycle_span_daa <= KAS_PAYOUT_CONFIRMATION_DAA {
            return Err(Krc20EngineError::SpanTooSmall {
                span: config.cycle_span_daa,
                depth: KAS_PAYOUT_CONFIRMATION_DAA,
            });
        }
        let lock_key = advisory_key(&config.lock_namespace);
        Ok(Self {
            pool,
            client,
            secret,
            treasury_address,
            quote,
            config,
            lock_key,
        })
    }

    /// Attempt one tick. Acquires the leader lock; if another instance holds
    /// it, returns [`Krc20TickOutcome::SkippedNotLeader`] without doing work.
    ///
    /// # Errors
    ///
    /// See [`Krc20EngineError`].
    #[tracing::instrument(
        name = "krc20.payout.cycle",
        skip(self),
        fields(instance = %self.config.instance_id),
        err,
    )]
    pub async fn run_once(&self) -> Result<Krc20TickOutcome, Krc20EngineError> {
        let Some(lock) = self.acquire_treasury_lock().await? else {
            debug!(
                instance = %self.config.instance_id,
                waited_secs = self.config.lock_acquire_wait.as_secs(),
                "krc20 payout lock held by another spender for the whole wait; skipping tick"
            );
            return Ok(Krc20TickOutcome::SkippedNotLeader);
        };

        let result = self.run_locked().await;

        // Always release, regardless of the work result.
        if let Err(e) = lock.release().await {
            warn!(instance = %self.config.instance_id, error = %e, "failed to release krc20 payout lock");
        }

        // Payout-cycle metrics (B7): one observation per leader tick.
        match &result {
            Ok(Krc20TickOutcome::Ran(report)) => {
                katpool_metrics::record_payout_cycle(
                    &self.config.instance_id,
                    "krc20",
                    report.status.as_str(),
                );
                if report.status.is_success() {
                    katpool_metrics::mark_payout_success(&self.config.instance_id, "krc20");
                }
            }
            Ok(Krc20TickOutcome::SkippedNotLeader) => {}
            Err(_) => {
                katpool_metrics::record_payout_cycle(&self.config.instance_id, "krc20", "error");
            }
        }
        result
    }

    /// Acquire the shared treasury-spend lock, retrying for up to
    /// [`Krc20PayoutEngineConfig::lock_acquire_wait`] before yielding.
    ///
    /// The lock is non-blocking ([`AdvisoryLock::try_acquire`]); this engine is
    /// phase-staggered off the KAS payout engine (see [`Self::run_loop`]) so it
    /// usually finds the lock free, and this bounded retry covers the case where
    /// a payout tick is mid-flight — it waits its turn instead of losing the race
    /// outright. Returns `Ok(None)` only if the lock stays held for the entire
    /// budget.
    async fn acquire_treasury_lock(&self) -> Result<Option<AdvisoryLock>, Krc20EngineError> {
        let deadline = time::Instant::now() + self.config.lock_acquire_wait;
        loop {
            if let Some(lock) = AdvisoryLock::try_acquire(&self.pool, self.lock_key).await? {
                return Ok(Some(lock));
            }
            if time::Instant::now() >= deadline {
                return Ok(None);
            }
            debug!(instance = %self.config.instance_id, "treasury lock busy; retrying");
            time::sleep(LOCK_RETRY_INTERVAL).await;
        }
    }

    async fn run_locked(&self) -> Result<Krc20TickOutcome, Krc20EngineError> {
        let keypair = Keypair::from_seckey_slice(secp256k1::SECP256K1, self.secret.expose_secret())
            .map_err(Krc20EngineError::Key)?;
        let xonly = keypair.x_only_public_key().0.serialize();
        let prefix = self.treasury_address.prefix;

        let virtual_daa = self.client.virtual_daa_score().await?;
        let (daa_start, daa_end) = cycle_window(virtual_daa, self.config.cycle_span_daa);
        let params = self.config.cycle_params(daa_start, daa_end);

        // Plan or resume the cycle for this window (quotes the floor price once;
        // fails the tick closed if the quote source is degraded).
        let state =
            resume_or_plan_krc20_cycle(&self.pool, &self.quote, &xonly, prefix, &params).await?;
        let cycle_id = state.cycle.id;

        // Money-safety circuit breaker (G1): refuse to settle a cycle whose
        // total non-failed NACHO exceeds the operator-set ceiling. This is the
        // primary guard against a compromised/erroneous floor-price quote
        // inflating every recipient's converted amount. Funds (NACHO) stay in
        // the treasury until the cap is raised or the quote is corrected.
        let outbound_nacho: i64 = state
            .transfers
            .iter()
            .filter(|t| t.status != Krc20TransferStatus::Failed)
            .map(|t| t.nacho_amount)
            .fold(0_i64, i64::saturating_add);
        if over_spend_cap(outbound_nacho, self.config.max_nacho_base_units_per_cycle) {
            let cap = self
                .config
                .max_nacho_base_units_per_cycle
                .unwrap_or_default();
            warn!(
                instance = %self.config.instance_id,
                cycle_id,
                outbound_nacho,
                cap,
                "krc20 payout cycle exceeds per-cycle NACHO spend cap; refusing to settle"
            );
            return Err(Krc20EngineError::SpendCapExceeded {
                total: outbound_nacho,
                cap,
            });
        }

        // Drive every open transfer one step (record-before-broadcast,
        // crash-safe, idempotent). Dry-run records/broadcasts nothing.
        let settle = settle_pending(
            &self.pool,
            &self.client,
            &self.secret,
            &self.treasury_address,
            self.config.batch_limit,
            self.config.mode,
        )
        .await?;

        // Crediting mutates `nacho_rebate.paid_sompi` (real accounting), so it
        // only runs on a live tick. A dry-run reports an empty credit.
        let credit = if self.config.mode.is_dry_run() {
            CreditReport::default()
        } else {
            credit_completed_transfers(&self.pool, self.config.batch_limit).await?
        };

        let status = reconcile_krc20_cycle_status(&self.pool, cycle_id).await?;

        info!(
            instance = %self.config.instance_id,
            cycle_id,
            virtual_daa,
            daa_start = daa_start.value(),
            daa_end = daa_end.value(),
            commits = settle.commits_broadcast,
            reveals = settle.reveals_broadcast,
            rebroadcasts = settle.rebroadcasts,
            completed = settle.completed,
            settle_pending = settle.pending,
            settle_errors = settle.errors.len(),
            credited = credit.credited,
            status = ?status,
            dry_run = self.config.mode.is_dry_run(),
            "krc20 payout tick complete"
        );

        Ok(Krc20TickOutcome::Ran(Box::new(Krc20TickReport {
            cycle_id,
            virtual_daa,
            daa_start: daa_start.value(),
            daa_end: daa_end.value(),
            settle,
            credit,
            status,
        })))
    }

    /// Run the periodic loop until `shutdown` flips to `true`.
    ///
    /// # Errors
    ///
    /// Only construction/invariant errors propagate; per-tick failures are
    /// logged and retried on the next interval.
    pub async fn run_loop(
        self,
        mut shutdown: watch::Receiver<bool>,
    ) -> Result<(), Krc20EngineError> {
        // Phase-stagger off the KAS payout engine: it shares this lock and ticks
        // on the same `poll_interval` from the same startup instant, so an
        // aligned phase would lose the lock race every tick (the KAS engine is
        // spawned first) and never settle a transfer. Start a quarter-interval
        // late so this engine's ticks land between the KAS engine (phase 0) and
        // the consolidation engine (phase ½). `interval_at` also skips the
        // immediate tick, so startup does not double-fire.
        let start = time::Instant::now() + self.config.poll_interval / 4;
        let mut interval = time::interval_at(start, self.config.poll_interval);
        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        info!(instance = %self.config.instance_id, "krc20 payout engine shutdown requested; exiting");
                        return Ok(());
                    }
                }
                _ = interval.tick() => {
                    if let Err(e) = self.run_once().await {
                        warn!(instance = %self.config.instance_id, error = %e, "krc20 payout tick failed; will retry");
                    }
                }
            }
        }
    }
}
