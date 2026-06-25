//! The consolidation engine: lock → snapshot → plan → sign → broadcast.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use kaspa_addresses::Address;
use kaspa_consensus_core::tx::{ScriptPublicKey, TransactionId};
use kaspa_txscript::pay_to_address_script;
use katpool_db::repo::{audit, payout, treasury};
use katpool_idempotency::{AdvisoryLock, advisory_key};
use katpool_secrets::TreasurySecret;
use katpool_storagemass::{FeeRate, MassEvaluator, TreasuryUtxo, plan_consolidation};
use serde_json::json;
use sqlx::PgPool;
use tokio::sync::watch;
use tokio::time;
use tracing::{debug, error, info, warn};

use crate::client::KaspadClient;
use crate::confirm::is_spendable;
use crate::execute::{ExecuteError, ExecutionMode, sign_batch_with_exact_fee};

/// Audit-log actor for consolidation broadcasts.
const AUDIT_ACTOR: &str = "treasury-consolidation";

/// Delay between treasury-lock acquisition retries within a single tick (see
/// [`ConsolidationEngineConfig::lock_acquire_wait`]).
const LOCK_RETRY_INTERVAL: Duration = Duration::from_millis(500);

/// Errors from the consolidation engine.
#[derive(Debug, thiserror::Error)]
pub enum ConsolidationError {
    /// Database / advisory-lock / snapshot failure.
    #[error(transparent)]
    Db(#[from] katpool_db::DbError),

    /// Sign / fee-resize failure for a batch.
    #[error(transparent)]
    Execute(#[from] ExecuteError),

    /// kaspad RPC failure.
    #[error(transparent)]
    Kaspad(#[from] crate::client::KaspadError),

    /// The tick exceeded its wall-clock budget (e.g. a hung kaspad RPC). The
    /// shared treasury lock is always released before this surfaces, so a stalled
    /// node can never wedge the other treasury spenders.
    #[error("consolidation tick exceeded timeout of {0:?}")]
    Timeout(Duration),
}

/// Consolidation engine configuration. Built from runtime config / env.
#[derive(Debug, Clone)]
pub struct ConsolidationEngineConfig {
    /// Instance label for logs.
    pub instance_id: String,
    /// How often to attempt a tick.
    pub poll_interval: Duration,
    /// Per-tick wall-clock budget. If a tick (kaspad RPCs + signing + broadcast)
    /// exceeds this, it is abandoned and the shared treasury lock is released, so
    /// a hung node can never wedge the payout engines.
    pub tick_timeout: Duration,
    /// How long a tick keeps retrying the shared treasury lock before yielding
    /// this round. Consolidation is the lowest-priority treasury spender, so it
    /// must defer to in-flight payouts; this bounded wait lets a busy payout
    /// tick merely *delay* consolidation instead of starving it (the payout
    /// engines share the same lock and tick on the same cadence). Kept well
    /// under `poll_interval`.
    pub lock_acquire_wait: Duration,
    /// Live broadcast or dry-run rehearsal.
    pub mode: ExecutionMode,
    /// High-water mark: a consolidation campaign starts only once the spendable
    /// UTXO count rises above this. Until then the engine is idle.
    pub trigger_utxo_count: usize,
    /// Low-water mark: once a campaign is active it keeps sweeping every tick
    /// until the spendable UTXO count falls to this floor, then rests until the
    /// count climbs back above `trigger_utxo_count` (hysteresis).
    pub target_utxo_count: usize,
    /// Upper bound on inputs per consolidation transaction (the real guard is
    /// the per-tx mass limit, which is checked per batch by the planner).
    pub max_inputs_per_tx: usize,
    /// Maximum consolidation transactions to broadcast per tick.
    pub max_txs_per_tick: usize,
    /// Advisory-lock namespace (the shared treasury-spend lock).
    pub lock_namespace: String,
}

/// Result of a single consolidation tick.
#[derive(Debug, Clone)]
pub enum ConsolidationTickOutcome {
    /// Another treasury spender held the leader lock; this tick did no work.
    SkippedNotLeader,
    /// This instance was leader and evaluated the treasury.
    Ran(Box<ConsolidationTickReport>),
}

/// Details of a consolidation tick that ran.
#[derive(Debug, Clone, Default)]
pub struct ConsolidationTickReport {
    /// Virtual DAA score observed at tick start.
    pub virtual_daa: u64,
    /// Spendable treasury UTXOs observed.
    pub spendable_utxos: usize,
    /// Summed spendable balance (sompi) recorded in the snapshot.
    pub spendable_balance_sompi: i64,
    /// `treasury_snapshot` row id written this tick.
    pub snapshot_id: i64,
    /// True when the tick was idle (no planning attempted): either no campaign
    /// is active, or the count is already at/below the target floor.
    pub below_ceiling: bool,
    /// Whether a consolidation campaign is active after this tick's hysteresis
    /// update (`true` once the count crossed `trigger_utxo_count`, until it
    /// falls to `target_utxo_count`).
    pub campaign_active: bool,
    /// Mass-valid consolidation batches the planner produced.
    pub planned_batches: usize,
    /// txids submitted (or, in dry-run, that would be).
    pub submitted_txids: Vec<TransactionId>,
    /// Non-fatal per-batch broadcast errors (txid: message).
    pub submit_errors: Vec<String>,
}

/// The consolidation engine. Owns its kaspad client and treasury key for the
/// life of the loop.
pub struct ConsolidationEngine<C: KaspadClient> {
    pool: PgPool,
    client: C,
    secret: TreasurySecret,
    treasury_address: Address,
    config: ConsolidationEngineConfig,
    lock_key: i64,
    /// Hysteresis latch: `true` while a sweep is in progress (set when the count
    /// crosses `trigger_utxo_count`, cleared when it reaches `target_utxo_count`).
    campaign_active: AtomicBool,
}

impl<C: KaspadClient> ConsolidationEngine<C> {
    /// Build a consolidation engine.
    #[must_use]
    pub fn new(
        pool: PgPool,
        client: C,
        secret: TreasurySecret,
        treasury_address: Address,
        config: ConsolidationEngineConfig,
    ) -> Self {
        let lock_key = advisory_key(&config.lock_namespace);
        Self {
            pool,
            client,
            secret,
            treasury_address,
            config,
            lock_key,
            campaign_active: AtomicBool::new(false),
        }
    }

    /// Attempt one tick. Acquires the shared treasury-spend lock; if another
    /// spender holds it, returns [`ConsolidationTickOutcome::SkippedNotLeader`]
    /// without doing work.
    #[tracing::instrument(
        name = "treasury.consolidation.tick",
        skip(self),
        fields(
            instance = %self.config.instance_id,
            trigger = self.config.trigger_utxo_count,
            target = self.config.target_utxo_count,
        ),
        err,
    )]
    pub async fn run_once(&self) -> Result<ConsolidationTickOutcome, ConsolidationError> {
        let Some(lock) = self.acquire_treasury_lock().await? else {
            info!(
                instance = %self.config.instance_id,
                waited_secs = self.config.lock_acquire_wait.as_secs(),
                "treasury lock held by another spender for the whole wait; skipping consolidation tick"
            );
            return Ok(ConsolidationTickOutcome::SkippedNotLeader);
        };

        // Bound the locked section: if a kaspad RPC (or signing/broadcast) hangs,
        // the lock-holding connection would otherwise wedge the payout engines
        // indefinitely. On timeout we abandon the tick and fall through to the
        // unconditional release below.
        let result = match time::timeout(self.config.tick_timeout, self.run_locked()).await {
            Ok(result) => result,
            Err(_elapsed) => {
                error!(
                    instance = %self.config.instance_id,
                    timeout_secs = self.config.tick_timeout.as_secs(),
                    "consolidation tick exceeded timeout; releasing treasury lock"
                );
                Err(ConsolidationError::Timeout(self.config.tick_timeout))
            }
        };

        if let Err(e) = lock.release().await {
            warn!(instance = %self.config.instance_id, error = %e, "failed to release treasury lock");
        }

        // Treasury-balance metrics (B7): publish the snapshot totals this tick
        // observed. This is the workspace's authoritative treasury-balance gauge.
        if let Ok(ConsolidationTickOutcome::Ran(report)) = &result {
            katpool_metrics::set_treasury_snapshot(
                &self.config.instance_id,
                report.spendable_balance_sompi,
                i64::try_from(report.spendable_utxos).unwrap_or(i64::MAX),
            );
        }
        result
    }

    /// Acquire the shared treasury-spend lock, retrying for up to
    /// [`ConsolidationEngineConfig::lock_acquire_wait`] before yielding.
    ///
    /// The lock is non-blocking ([`AdvisoryLock::try_acquire`]); consolidation
    /// is phase-staggered off the payout engines (see [`Self::run_loop`]) so it
    /// usually finds the lock free, and this bounded retry covers the case where
    /// a payout tick is mid-flight — it waits its turn instead of losing the
    /// race outright. Returns `Ok(None)` only if the lock stays held for the
    /// entire budget.
    async fn acquire_treasury_lock(&self) -> Result<Option<AdvisoryLock>, ConsolidationError> {
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

    // The tick is a single linear procedure (snapshot → ceiling check → plan →
    // per-batch sign/broadcast/audit); splitting it would scatter the ordering
    // contract across helpers for no real gain.
    #[allow(clippy::too_many_lines)]
    async fn run_locked(&self) -> Result<ConsolidationTickOutcome, ConsolidationError> {
        let treasury_script = pay_to_address_script(&self.treasury_address);
        let virtual_daa = self.client.virtual_daa_score().await?;

        // Change outputs of payouts that have not yet confirmed. Confirmation
        // detects acceptance from the payout's treasury change coin, so sweeping
        // it before the payout settles would strand that payout (and its cycle)
        // forever. Exclude any treasury coin produced by such a transaction.
        let protected: HashSet<[u8; 32]> = payout::in_flight_spend_tx_hashes(&self.pool)
            .await?
            .into_iter()
            .filter_map(|h| <[u8; 32]>::try_from(h.as_slice()).ok())
            .collect();

        // Spendable (mature) treasury coins only — immature coinbase coins
        // cannot be spent, so they neither count toward the ceiling nor fund a
        // consolidation transaction. Unconfirmed-payout change coins are held
        // back (see `protected`) so consolidation never strands a payout.
        let utxos: Vec<TreasuryUtxo> = self
            .client
            .treasury_utxos(&self.treasury_address)
            .await?
            .into_iter()
            .filter(|s| is_spendable(s.entry.block_daa_score, s.entry.is_coinbase, virtual_daa))
            .filter(|s| !protected.contains(&s.outpoint.transaction_id.as_bytes()))
            .map(crate::client::TreasuryUtxoSnapshot::into_treasury_utxo)
            .collect();

        let spendable_utxos = utxos.len();
        let balance_sompi: u64 = utxos.iter().map(|u| u.entry.amount).sum();
        let spendable_balance_sompi = clamp_i64(balance_sompi);

        // Record an observability snapshot every tick (count is the signal).
        let note = if self.config.mode.is_dry_run() {
            "consolidation tick (dry-run)"
        } else {
            "consolidation tick"
        };
        let snapshot_id = treasury::insert_snapshot(
            &self.pool,
            spendable_balance_sompi,
            clamp_i32(spendable_utxos),
            clamp_i64(virtual_daa),
            Some(note),
        )
        .await?;

        // Hysteresis: start a sweep only once fragmentation rises above the
        // trigger (high-water mark); keep sweeping every tick until the count
        // falls to the target floor (low-water mark), then rest until it climbs
        // back above the trigger. This batches consolidation into efficient
        // bursts instead of churning on every newly-matured coinbase coin.
        if spendable_utxos > self.config.trigger_utxo_count {
            self.campaign_active.store(true, Ordering::Relaxed);
        } else if spendable_utxos <= self.config.target_utxo_count {
            self.campaign_active.store(false, Ordering::Relaxed);
        }
        let campaign_active = self.campaign_active.load(Ordering::Relaxed);

        let mut report = ConsolidationTickReport {
            virtual_daa,
            spendable_utxos,
            spendable_balance_sompi,
            snapshot_id,
            campaign_active,
            ..Default::default()
        };

        if !campaign_active {
            report.below_ceiling = true;
            info!(
                instance = %self.config.instance_id,
                spendable_utxos,
                trigger = self.config.trigger_utxo_count,
                target = self.config.target_utxo_count,
                "treasury below consolidation trigger; idle"
            );
            return Ok(ConsolidationTickOutcome::Ran(Box::new(report)));
        }

        // Reserve a real network fee out of each merged coin (same policy as
        // payouts); a fee-estimate RPC failure falls back to the relay floor.
        let fee_rate = match self.client.fee_estimate_sompi_per_gram().await {
            Ok(feerate) => FeeRate::from_feerate(feerate),
            Err(e) => {
                warn!(error = %e, "fee-estimate RPC failed; using minimum relay fee floor");
                FeeRate::from_feerate(0.0)
            }
        };

        let evaluator = MassEvaluator::mainnet();
        let batches = plan_consolidation(
            &evaluator,
            utxos,
            &treasury_script,
            &fee_rate,
            self.config.max_inputs_per_tx,
            self.config.max_txs_per_tick,
        );
        report.planned_batches = batches.len();

        for batch in &batches {
            // Sign with the exact fee sized from the *signed* transaction's
            // mass (in-memory, verified; no external effect on failure).
            let signed = sign_batch_with_exact_fee(
                batch,
                &treasury_script,
                &self.secret,
                &fee_rate,
                &evaluator,
            )?;
            let txid = signed.txid();

            if self.config.mode.is_dry_run() {
                report.submitted_txids.push(txid);
                continue;
            }

            match self.client.submit_transaction(&signed.tx, false).await {
                Ok(_accepted) => {
                    report.submitted_txids.push(txid);
                    let payload = json!({
                        "txid": txid.to_string(),
                        "inputs": batch.inputs.len(),
                        "merged_sompi": batch.change_amount_sompi,
                        "virtual_daa": virtual_daa,
                    });
                    // Audit is best-effort: a logging-table write must never
                    // unwind a successful on-chain broadcast.
                    if let Err(e) = audit::append(
                        &self.pool,
                        audit::NewEntry::new(AUDIT_ACTOR, "treasury.consolidate")
                            .subject("treasury_snapshot", snapshot_id)
                            .payload(payload),
                    )
                    .await
                    {
                        warn!(%txid, error = %e, "failed to write consolidation audit entry");
                    }
                }
                Err(e) => {
                    error!(%txid, inputs = batch.inputs.len(), error = %e,
                        "consolidation broadcast rejected by kaspad");
                    report.submit_errors.push(format!("{txid}: {e}"));
                }
            }
        }

        info!(
            instance = %self.config.instance_id,
            virtual_daa,
            spendable_utxos,
            trigger = self.config.trigger_utxo_count,
            target = self.config.target_utxo_count,
            planned_batches = report.planned_batches,
            submitted = report.submitted_txids.len(),
            errors = report.submit_errors.len(),
            dry_run = self.config.mode.is_dry_run(),
            "consolidation sweep tick complete"
        );

        Ok(ConsolidationTickOutcome::Ran(Box::new(report)))
    }

    /// Run the periodic loop until `shutdown` flips to `true`.
    pub async fn run_loop(
        self,
        mut shutdown: watch::Receiver<bool>,
    ) -> Result<(), ConsolidationError> {
        // Phase-stagger off the payout engines: they share this lock and tick on
        // the same `poll_interval` from the same startup instant, so an aligned
        // phase would lose the lock race every tick and never consolidate. Start
        // half an interval late so our ticks land in the gap when payouts are
        // idle and the lock is free. `interval_at` also skips the immediate
        // tick, so startup does not double-fire.
        let start = time::Instant::now() + self.config.poll_interval / 2;
        let mut interval = time::interval_at(start, self.config.poll_interval);
        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        info!(instance = %self.config.instance_id, "consolidation engine shutdown requested; exiting");
                        return Ok(());
                    }
                }
                _ = interval.tick() => {
                    if let Err(e) = self.run_once().await {
                        warn!(instance = %self.config.instance_id, error = %e, "consolidation tick failed; will retry");
                    }
                }
            }
        }
    }

    /// The treasury script this engine consolidates into (for tests/tooling).
    #[must_use]
    pub fn treasury_script(&self) -> ScriptPublicKey {
        pay_to_address_script(&self.treasury_address)
    }
}

/// Saturating `u64 → i64` for DB columns (BIGINT). Treasury balances and DAA
/// scores are far below `i64::MAX` in practice; clamp defensively.
fn clamp_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

/// Saturating `usize → i32` for the `utxo_count` column.
fn clamp_i32(value: usize) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}
