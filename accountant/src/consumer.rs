//! Event consumer.
//!
//! Drains a `tokio::sync::broadcast::Receiver<PoolEvent>` and
//! mirrors every event into the new schema's `wallet`, `worker`,
//! `share`, and `block` tables.
//!
//! ## Lifecycle
//!
//! ```text
//!     ┌── ShareCredited   → wallet::ensure  → worker::ensure → share::insert_credited
//!     │
//!     ├── ShareRejected   → metric tick only (per `docs/decisions/0012`,
//!     │                     reject persistence is M2 scope; M1 keeps the
//!     │                     hot path lean).
//!     │
//!     ├── BlockFound      → wallet::ensure  → worker::ensure → block::ensure
//!     │                                                       (status='found')
//!     │
//!     └── BlockAccepted   → block::mark_submitted (no-op if no prior found)
//! ```
//!
//! ## Lag tolerance
//!
//! The broadcast channel is bounded. A slow consumer eventually
//! sees `RecvError::Lagged(n)` — the consumer increments the
//! `ks_accountant_events_lagged_total` counter and continues
//! draining. We do **not** terminate the consumer on lag: missed
//! shares are unrecoverable (the bridge channel isn't durable),
//! but the consumer must remain healthy so subsequent events
//! still land.
//!
//! ## Channel close
//!
//! `RecvError::Closed` ends the consumer task with `Ok(())`.
//! Callers can `await` the returned `JoinHandle` to observe a
//! clean shutdown.
//!
//! ## Graceful shutdown drain
//!
//! [`EventConsumer::run_with_shutdown`] adds an explicit shutdown
//! signal so the runtime can drain the broadcast backlog at SIGTERM
//! instead of aborting the task mid-buffer. On the signal the consumer
//! stops blocking on new events and drains everything already on the
//! bus, finishing once the channel is idle for `drain_idle` (no event
//! for that gap) or the hard `drain_budget` elapses, then returns
//! `Ok(())`. The runtime stops the producer (the stratum bridge listener)
//! first, so in steady state every event that reached the bus before
//! shutdown is persisted. The narrow residual — events the bridge's
//! detached per-connection tasks emit *after* the signal — is bounded by
//! `drain_idle` and is the documented limit until the vendored bridge
//! grows a cooperative shutdown of its own (ADR-0002 follow-up).
//!
//! ## Failure isolation
//!
//! Per-event DB errors are logged, counted, and swallowed. A
//! single bad event must never poison the consumer — Phase 1's
//! `PoolEvent` types validate everything domain-side, so a DB
//! constraint failure is almost always either a transient
//! Postgres issue (resolved on the next event) or a bug we want
//! the metric tick to surface.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{DateTime, Utc};
use katpool_db::repo::block::{self, EnsureOutcome};
use katpool_db::repo::share_reject::{self, DbShareRejectReason};
use katpool_db::repo::{SessionId, connection_session, share, wallet, worker};
use katpool_domain::{PoolEvent, WalletAddress, WorkerName};
use sqlx::PgPool;
use tokio::sync::{broadcast, watch};
use tokio::time::{Instant, timeout};
use tracing::{debug, error, info, warn};

use crate::error::EventError;
use crate::geoip::GeoIp;
use crate::metrics::{
    record_block_transition, record_event, record_event_error, record_lag, record_share_insert,
};

/// The five network identifiers the schema's `wallet_address_format`
/// CHECK constraint accepts (see
/// `crates/katpool-db/migrations/20260526000000_bootstrap.sql`).
///
/// Keep this list in lock-step with the migration: a value the
/// schema rejects would cause every `wallet::ensure` to fail with
/// SQLSTATE `23514`, which is exactly the M3d `wallet_ensure`
/// production incident this constant was introduced to prevent.
pub const VALID_NETWORKS: &[&str] = &["mainnet", "testnet-10", "testnet-11", "devnet", "simnet"];

/// Identity + lifetime metadata shared by the session-open and
/// session-close handlers. Borrowed for the duration of one event so the
/// handlers stay within clippy's argument-count budget without losing the
/// per-field documentation the event already carries.
struct SessionMeta<'a> {
    /// Authenticated wallet, if the session reached authorize.
    wallet: Option<&'a WalletAddress>,
    /// Worker rig, if the authorize payload carried one.
    worker: Option<&'a WorkerName>,
    /// Remote miner IP as text (parsed to `inet` by the repo layer).
    remote_ip: &'a str,
    /// Reported stratum `mining.subscribe` user-agent, if any.
    remote_app: Option<&'a str>,
    /// TCP-accept timestamp.
    connected_at: DateTime<Utc>,
}

/// Configuration carried by every consumer instance. Cheap to
/// clone (it's all `Arc`-able internals + a small instance label).
#[derive(Debug, Clone)]
pub struct ConsumerConfig {
    /// Stable label used by every metric the consumer emits.
    /// Typical value: the systemd instance name (e.g. `primary`,
    /// `shadow`).
    pub instance_id: String,
    /// Kaspa network identifier — passed verbatim to
    /// [`katpool_db::repo::wallet::ensure`]. Must be one of
    /// [`VALID_NETWORKS`]; otherwise every wallet upsert would fail
    /// the schema's `wallet_address_format` CHECK constraint at
    /// runtime. Validated at construction time so misconfiguration
    /// fails fast instead of silently dropping every share /
    /// block event.
    pub network: String,
}

impl ConsumerConfig {
    /// Construct with the given instance id and network.
    ///
    /// # Errors
    /// Returns an error if `network` is not one of [`VALID_NETWORKS`].
    pub fn new(instance_id: String, network: String) -> Result<Self, ConsumerConfigError> {
        if !VALID_NETWORKS.contains(&network.as_str()) {
            return Err(ConsumerConfigError::InvalidNetwork {
                supplied: network,
                allowed: VALID_NETWORKS,
            });
        }
        Ok(Self {
            instance_id,
            network,
        })
    }
}

/// Errors raised by [`ConsumerConfig::new`].
#[derive(Debug, thiserror::Error)]
pub enum ConsumerConfigError {
    /// The supplied `network` is not in [`VALID_NETWORKS`]; the DB's
    /// `wallet_address_format` CHECK constraint would reject every
    /// wallet upsert.
    #[error("invalid network `{supplied}` (allowed: {allowed:?})")]
    InvalidNetwork {
        /// The rejected value the caller passed.
        supplied: String,
        /// The set of values the schema accepts.
        allowed: &'static [&'static str],
    },
}

/// Whether the consumer event loop should keep running or exit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoopStep {
    Continue,
    Exit,
}

/// The consumer task. Holds the DB pool + the consumer's
/// configuration; `run` consumes both and drives the event loop.
pub struct EventConsumer {
    db: PgPool,
    cfg: ConsumerConfig,
    /// Optional IP→country resolver (ADR-0025). `None` when no `GeoLite2`
    /// database is configured — sessions then record a `NULL` country.
    geoip: Option<Arc<GeoIp>>,
    /// Maps a bridge connection id to the live `connection_session` row
    /// opened for it, so a later `SessionClosed` finalizes the right row.
    /// In-process only (cleared on restart; the startup sweep finalizes
    /// any rows orphaned by the previous incarnation).
    open_sessions: Arc<Mutex<HashMap<u64, SessionId>>>,
}

impl EventConsumer {
    /// Construct a consumer ready to be `run`.
    #[must_use]
    pub fn new(db: PgPool, cfg: ConsumerConfig) -> Self {
        Self {
            db,
            cfg,
            geoip: None,
            open_sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Attach an optional `GeoIP` resolver for session country tagging.
    ///
    /// Pass `None` to leave geo resolution disabled (the default).
    #[must_use]
    pub fn with_geoip(mut self, geoip: Option<Arc<GeoIp>>) -> Self {
        self.geoip = geoip;
        self
    }

    /// Drive the consumer until the broadcast channel closes.
    ///
    /// Returns when the broadcast channel is closed by every
    /// sender. Per-event errors are logged + counted, never
    /// returned.
    pub async fn run(self, mut rx: broadcast::Receiver<PoolEvent>) -> Result<(), anyhow::Error> {
        self.startup_session_sweep().await;
        loop {
            if matches!(self.step(rx.recv().await).await, LoopStep::Exit) {
                return Ok(());
            }
        }
    }

    /// Drive the consumer until the broadcast channel closes **or** `shutdown`
    /// flips to `true`, then drain the in-bus backlog before returning.
    ///
    /// On the shutdown signal the consumer stops blocking on new events and
    /// drains everything already buffered, finishing once the bus has been
    /// idle for `drain_idle` or the hard `drain_budget` elapses (whichever
    /// comes first), then returns `Ok(())`. See the module-level "Graceful
    /// shutdown drain" note for the producer-stop ordering this relies on.
    /// Per-event errors are logged + counted, never returned.
    pub async fn run_with_shutdown(
        self,
        mut rx: broadcast::Receiver<PoolEvent>,
        mut shutdown: watch::Receiver<bool>,
        drain_idle: Duration,
        drain_budget: Duration,
    ) -> Result<(), anyhow::Error> {
        self.startup_session_sweep().await;

        // A shutdown that already latched before we subscribed must still be
        // honoured (the signal task may fire before this task is scheduled).
        if *shutdown.borrow() {
            self.drain_backlog(&mut rx, drain_idle, drain_budget).await;
            return Ok(());
        }

        loop {
            tokio::select! {
                recv = rx.recv() => {
                    if matches!(self.step(recv).await, LoopStep::Exit) {
                        return Ok(());
                    }
                }
                changed = shutdown.changed() => {
                    // Sender dropped (`Err`) is treated as a shutdown request:
                    // the runtime is tearing down regardless.
                    if changed.is_err() || *shutdown.borrow() {
                        info!(instance = %self.cfg.instance_id, "shutdown signalled; draining event backlog");
                        self.drain_backlog(&mut rx, drain_idle, drain_budget).await;
                        return Ok(());
                    }
                }
            }
        }
    }

    /// Finalize any sessions left open by a previous process incarnation.
    /// Bridge + accountant share one process, so a restart already dropped
    /// every socket; surviving miners reconnect and re-open. Best-effort: a
    /// sweep failure must not stop the consumer from draining live events.
    async fn startup_session_sweep(&self) {
        info!(instance = %self.cfg.instance_id, "accountant consumer starting");
        match connection_session::close_all_open(&self.db).await {
            Ok(n) if n > 0 => {
                info!(instance = %self.cfg.instance_id, closed = n, "closed orphaned sessions at startup");
            }
            Ok(_) => {}
            Err(e) => {
                warn!(instance = %self.cfg.instance_id, error = %e, "startup session sweep failed");
            }
        }
    }

    /// Apply one received broadcast result. Returns whether the loop should
    /// continue or exit (the channel closed).
    async fn step(&self, recv: Result<PoolEvent, broadcast::error::RecvError>) -> LoopStep {
        match recv {
            Ok(event) => {
                self.handle_event(event).await;
                LoopStep::Continue
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                record_lag(&self.cfg.instance_id);
                warn!(
                    instance = %self.cfg.instance_id,
                    skipped = n,
                    "broadcast channel lag; events dropped"
                );
                LoopStep::Continue
            }
            Err(broadcast::error::RecvError::Closed) => {
                info!(instance = %self.cfg.instance_id, "broadcast channel closed; consumer exiting");
                LoopStep::Exit
            }
        }
    }

    /// Drain the broadcast backlog at shutdown. Keeps consuming events until
    /// the bus is idle for `drain_idle` (no event within that gap) or the hard
    /// `drain_budget` elapses, then returns. Bounded so a misbehaving producer
    /// cannot wedge teardown.
    async fn drain_backlog(
        &self,
        rx: &mut broadcast::Receiver<PoolEvent>,
        drain_idle: Duration,
        drain_budget: Duration,
    ) {
        let deadline = Instant::now() + drain_budget;
        let mut drained = 0_usize;
        loop {
            let now = Instant::now();
            if now >= deadline {
                warn!(
                    instance = %self.cfg.instance_id,
                    drained,
                    "drain budget exhausted; exiting with possible backlog remaining"
                );
                break;
            }
            let wait = drain_idle.min(deadline.saturating_duration_since(now));
            match timeout(wait, rx.recv()).await {
                Ok(Ok(event)) => {
                    self.handle_event(event).await;
                    drained += 1;
                }
                Ok(Err(broadcast::error::RecvError::Lagged(n))) => {
                    record_lag(&self.cfg.instance_id);
                    warn!(instance = %self.cfg.instance_id, skipped = n, "broadcast channel lag during drain");
                }
                Ok(Err(broadcast::error::RecvError::Closed)) => break,
                // No event within the idle gap: the backlog is drained.
                Err(_elapsed) => break,
            }
        }
        info!(instance = %self.cfg.instance_id, drained, "event backlog drained; consumer exiting");
    }

    /// Single-event dispatch. Public for testing — callers in
    /// production should always go through `run`.
    // A flat match over every `PoolEvent` variant; each arm is a thin
    // record-metric-then-delegate. Splitting it would only scatter the
    // dispatch table without reducing complexity.
    #[allow(clippy::too_many_lines)]
    pub async fn handle_event(&self, event: PoolEvent) {
        match event {
            PoolEvent::ShareCredited {
                wallet,
                worker,
                difficulty,
                daa_score,
                ts: _,
                correlation_id,
            } => {
                let variant = "share_credited";
                record_event(&self.cfg.instance_id, variant);
                if let Err(e) = self
                    .handle_share_credited(&wallet, &worker, difficulty, daa_score, correlation_id)
                    .await
                {
                    self.log_event_error(variant, &e, &correlation_id);
                }
            }
            PoolEvent::ShareRejected {
                wallet,
                worker,
                reason,
                ts: _,
                correlation_id,
            } => {
                let variant = "share_rejected";
                record_event(&self.cfg.instance_id, variant);
                if let Err(e) = self
                    .handle_share_rejected(&wallet, &worker, reason, correlation_id)
                    .await
                {
                    self.log_event_error(variant, &e, &correlation_id);
                }
            }
            PoolEvent::BlockFound {
                wallet,
                worker,
                hash,
                daa_score,
                ts: _,
                correlation_id,
            } => {
                let variant = "block_found";
                record_event(&self.cfg.instance_id, variant);
                if let Err(e) = self
                    .handle_block_found(&wallet, &worker, hash, daa_score, correlation_id)
                    .await
                {
                    self.log_event_error(variant, &e, &correlation_id);
                }
            }
            PoolEvent::BlockAccepted {
                hash,
                ts: _,
                correlation_id,
            } => {
                let variant = "block_accepted";
                record_event(&self.cfg.instance_id, variant);
                if let Err(e) = self.handle_block_accepted(hash, correlation_id).await {
                    self.log_event_error(variant, &e, &correlation_id);
                }
            }
            PoolEvent::SessionOpened {
                conn_id,
                wallet,
                worker,
                remote_ip,
                remote_app,
                connected_at,
                correlation_id,
            } => {
                let variant = "session_opened";
                record_event(&self.cfg.instance_id, variant);
                if let Err(e) = self
                    .handle_session_opened(
                        conn_id,
                        SessionMeta {
                            wallet: wallet.as_ref(),
                            worker: worker.as_ref(),
                            remote_ip: &remote_ip,
                            remote_app: remote_app.as_deref(),
                            connected_at,
                        },
                    )
                    .await
                {
                    self.log_event_error(variant, &e, &correlation_id);
                }
            }
            PoolEvent::SessionClosed {
                conn_id,
                wallet,
                worker,
                remote_ip,
                remote_app,
                connected_at,
                ts,
                correlation_id,
            } => {
                let variant = "session_closed";
                record_event(&self.cfg.instance_id, variant);
                if let Err(e) = self
                    .handle_session_closed(
                        conn_id,
                        SessionMeta {
                            wallet: wallet.as_ref(),
                            worker: worker.as_ref(),
                            remote_ip: &remote_ip,
                            remote_app: remote_app.as_deref(),
                            connected_at,
                        },
                        ts,
                    )
                    .await
                {
                    self.log_event_error(variant, &e, &correlation_id);
                }
            }
            // `PoolEvent` is `#[non_exhaustive]` by design; we
            // must keep this arm so adding a new variant upstream
            // doesn't break the build, but log loudly so an
            // operator knows the bridge added something the
            // accountant doesn't yet understand.
            other => {
                record_event(&self.cfg.instance_id, "unknown");
                warn!(event = ?other, "accountant received unknown PoolEvent variant");
            }
        }
    }

    async fn handle_share_rejected(
        &self,
        wallet_addr: &katpool_domain::WalletAddress,
        worker_name: &katpool_domain::WorkerName,
        reason: katpool_domain::ShareRejectReason,
        correlation_id: katpool_domain::CorrelationId,
    ) -> Result<(), EventError> {
        // Translate first so we can bail before opening a tx if
        // the reason has no schema mapping (recoverable, surfaces
        // as a metric tick via the caller's error-logging path).
        let db_reason = DbShareRejectReason::try_from(reason)
            .map_err(|e| EventError::UnknownRejectReason { reason: e.reason })?;

        let mut tx = self
            .db
            .begin()
            .await
            .map_err(katpool_db::DbError::from)
            .map_err(EventError::ShareRejectInsert)?;
        let w = wallet::ensure(&mut *tx, wallet_addr, &self.cfg.network)
            .await
            .map_err(EventError::WalletEnsure)?;
        let wk = worker::ensure(&mut *tx, w.id, worker_name)
            .await
            .map_err(EventError::WorkerEnsure)?;
        share_reject::insert(&mut *tx, w.id, wk.id, db_reason, correlation_id)
            .await
            .map_err(EventError::ShareRejectInsert)?;
        tx.commit()
            .await
            .map_err(katpool_db::DbError::from)
            .map_err(EventError::ShareRejectInsert)?;
        Ok(())
    }

    async fn handle_share_credited(
        &self,
        wallet_addr: &katpool_domain::WalletAddress,
        worker_name: &katpool_domain::WorkerName,
        difficulty: katpool_domain::ShareDifficulty,
        daa_score: katpool_domain::DaaScore,
        correlation_id: katpool_domain::CorrelationId,
    ) -> Result<(), EventError> {
        let mut tx = self
            .db
            .begin()
            .await
            .map_err(katpool_db::DbError::from)
            .map_err(EventError::ShareInsert)?;
        let w = wallet::ensure(&mut *tx, wallet_addr, &self.cfg.network)
            .await
            .map_err(EventError::WalletEnsure)?;
        let wk = worker::ensure(&mut *tx, w.id, worker_name)
            .await
            .map_err(EventError::WorkerEnsure)?;
        share::insert_credited(
            &mut *tx,
            w.id,
            wk.id,
            None,
            difficulty,
            daa_score,
            correlation_id,
        )
        .await
        .map_err(EventError::ShareInsert)?;
        tx.commit()
            .await
            .map_err(katpool_db::DbError::from)
            .map_err(EventError::ShareInsert)?;
        record_share_insert(&self.cfg.instance_id);
        Ok(())
    }

    async fn handle_block_found(
        &self,
        wallet_addr: &katpool_domain::WalletAddress,
        worker_name: &katpool_domain::WorkerName,
        hash: katpool_domain::BlockHash,
        daa_score: katpool_domain::DaaScore,
        correlation_id: katpool_domain::CorrelationId,
    ) -> Result<(), EventError> {
        let mut tx = self
            .db
            .begin()
            .await
            .map_err(katpool_db::DbError::from)
            .map_err(EventError::BlockEnsure)?;
        let w = wallet::ensure(&mut *tx, wallet_addr, &self.cfg.network)
            .await
            .map_err(EventError::WalletEnsure)?;
        let wk = worker::ensure(&mut *tx, w.id, worker_name)
            .await
            .map_err(EventError::WorkerEnsure)?;
        // PoolEvent::BlockFound doesn't carry a nonce — that
        // belongs to the candidate template the bridge submits,
        // not to the share-validation event. Phase 1's event type
        // omits it deliberately. We record 0 here; the schema's
        // `nonce` column is informational only (no CHECK), and the
        // M3 maturity path overwrites it from the kaspad header
        // when it lands.
        let nonce: u64 = 0;
        let (_, outcome) = block::ensure(
            &mut *tx,
            hash,
            w.id,
            wk.id,
            daa_score,
            nonce,
            correlation_id,
        )
        .await
        .map_err(EventError::BlockEnsure)?;
        tx.commit()
            .await
            .map_err(katpool_db::DbError::from)
            .map_err(EventError::BlockEnsure)?;
        match outcome {
            EnsureOutcome::Inserted => record_block_transition(&self.cfg.instance_id, "found"),
            EnsureOutcome::AlreadyExisted => {
                record_block_transition(&self.cfg.instance_id, "dup_found");
                debug!(hash = %hash, "duplicate BlockFound event; ignoring");
            }
        }
        Ok(())
    }

    async fn handle_block_accepted(
        &self,
        hash: katpool_domain::BlockHash,
        correlation_id: katpool_domain::CorrelationId,
    ) -> Result<(), EventError> {
        // The repo's `mark_submitted` is itself idempotent
        // (it gates on status IN ('found', 'submitted_to_node'))
        // so we don't need our own pre-check.
        //
        // But we DO want to know whether the row existed — if it
        // didn't, the BlockAccepted arrived without a prior
        // BlockFound (race condition during consumer startup),
        // and we surface that as a metric + warning.
        let existing = block::find_by_hash(&self.db, hash)
            .await
            .map_err(EventError::BlockMarkSubmitted)?;
        if existing.is_none() {
            record_event_error(&self.cfg.instance_id, "block_accepted", "orphan");
            warn!(
                correlation_id = %correlation_id,
                hash = %hash,
                "BlockAccepted arrived without prior BlockFound; ignoring"
            );
            return Err(EventError::OrphanBlockAccepted {
                hash: hash.to_string(),
            });
        }
        block::mark_submitted(&self.db, hash)
            .await
            .map_err(EventError::BlockMarkSubmitted)?;
        record_block_transition(&self.cfg.instance_id, "submitted");
        Ok(())
    }

    /// Open a live `connection_session` row when a connection
    /// authenticates: resolve the worker id + country and insert a row
    /// with `disconnected_at = NULL`, then remember the row id keyed by
    /// the bridge connection id so the matching close finalizes it.
    /// Identity-only — never touches the hot crediting path.
    async fn handle_session_opened(
        &self,
        conn_id: u64,
        meta: SessionMeta<'_>,
    ) -> Result<(), EventError> {
        let SessionMeta {
            wallet: wallet_addr,
            worker: worker_name,
            remote_ip,
            remote_app,
            connected_at,
        } = meta;
        let ip: IpAddr = remote_ip.parse().map_err(|_| EventError::SessionBadIp {
            ip: remote_ip.to_owned(),
        })?;

        // Resolve country off the hot path (ADR-0025). Absent resolver or
        // unknown IP ⇒ NULL country; never fails the session write.
        let country = self.geoip.as_ref().and_then(|g| g.country(ip));

        let mut tx = self
            .db
            .begin()
            .await
            .map_err(katpool_db::DbError::from)
            .map_err(EventError::SessionRecord)?;

        let worker_id = match (wallet_addr, worker_name) {
            (Some(addr), Some(name)) => {
                let w = wallet::ensure(&mut *tx, addr, &self.cfg.network)
                    .await
                    .map_err(EventError::WalletEnsure)?;
                let wk = worker::ensure(&mut *tx, w.id, name)
                    .await
                    .map_err(EventError::WorkerEnsure)?;
                Some(wk.id)
            }
            _ => None,
        };

        let session_id = connection_session::open(
            &mut *tx,
            worker_id,
            ip,
            remote_app,
            country.as_deref(),
            connected_at,
        )
        .await
        .map_err(EventError::SessionRecord)?;

        tx.commit()
            .await
            .map_err(katpool_db::DbError::from)
            .map_err(EventError::SessionRecord)?;

        // Short, non-async critical section — never held across an await.
        if let Ok(mut map) = self.open_sessions.lock() {
            map.insert(conn_id, session_id);
        }
        Ok(())
    }

    /// Finalize a stratum session at disconnect. If we opened a live row
    /// for this connection (it authorized), close that exact row so its
    /// duration and bound worker are preserved. Otherwise — a session
    /// that dropped before authorize — fall back to inserting a completed
    /// row for per-IP forensics + the firmware breakdown. Identity-only.
    async fn handle_session_closed(
        &self,
        conn_id: u64,
        meta: SessionMeta<'_>,
        disconnected_at: DateTime<Utc>,
    ) -> Result<(), EventError> {
        // Did we open a live row for this connection? (Lock released
        // before any await.)
        let open_row = self
            .open_sessions
            .lock()
            .ok()
            .and_then(|mut map| map.remove(&conn_id));

        if let Some(session_id) = open_row {
            connection_session::close(&self.db, session_id)
                .await
                .map_err(EventError::SessionRecord)?;
            return Ok(());
        }

        // No live row: legacy insert-at-close path for pre-authorize
        // sessions (subscribe-only blips that still reported a user-agent).
        let SessionMeta {
            wallet: wallet_addr,
            worker: worker_name,
            remote_ip,
            remote_app,
            connected_at,
        } = meta;
        let ip: IpAddr = remote_ip.parse().map_err(|_| EventError::SessionBadIp {
            ip: remote_ip.to_owned(),
        })?;

        let country = self.geoip.as_ref().and_then(|g| g.country(ip));

        let mut tx = self
            .db
            .begin()
            .await
            .map_err(katpool_db::DbError::from)
            .map_err(EventError::SessionRecord)?;

        let worker_id = match (wallet_addr, worker_name) {
            (Some(addr), Some(name)) => {
                let w = wallet::ensure(&mut *tx, addr, &self.cfg.network)
                    .await
                    .map_err(EventError::WalletEnsure)?;
                let wk = worker::ensure(&mut *tx, w.id, name)
                    .await
                    .map_err(EventError::WorkerEnsure)?;
                Some(wk.id)
            }
            _ => None,
        };

        connection_session::record_closed(
            &mut *tx,
            worker_id,
            ip,
            remote_app,
            country.as_deref(),
            connected_at,
            disconnected_at,
        )
        .await
        .map_err(EventError::SessionRecord)?;

        tx.commit()
            .await
            .map_err(katpool_db::DbError::from)
            .map_err(EventError::SessionRecord)?;
        Ok(())
    }

    fn log_event_error(
        &self,
        variant: &'static str,
        err: &EventError,
        correlation_id: &katpool_domain::CorrelationId,
    ) {
        let kind = match err {
            EventError::WalletEnsure(_) => "wallet_ensure",
            EventError::WorkerEnsure(_) => "worker_ensure",
            EventError::ShareInsert(_) => "share_insert",
            EventError::ShareRejectInsert(_) => "share_reject_insert",
            EventError::UnknownRejectReason { .. } => "unknown_reject_reason",
            EventError::BlockEnsure(_) => "block_ensure",
            EventError::BlockMarkSubmitted(_) => "block_mark_submitted",
            EventError::OrphanBlockAccepted { .. } => "orphan_block_accepted",
            EventError::SessionBadIp { .. } => "session_bad_ip",
            EventError::SessionRecord(_) => "session_record",
        };
        record_event_error(&self.cfg.instance_id, variant, kind);
        error!(
            correlation_id = %correlation_id,
            variant,
            kind,
            error = %err,
            "accountant event handler failed"
        );
    }
}

#[cfg(test)]
mod consumer_config_tests {
    //! Regression guards for the Goldshell M3d live-exercise
    //! production incident in which `ConsumerConfig` hard-coded
    //! `NETWORK = "mainnet"`, causing every `wallet::ensure` on a
    //! testnet run to fail the schema's `wallet_address_format`
    //! CHECK constraint (9,775 occurrences in a single 2.5-minute
    //! window). These tests pin both the validation behaviour and
    //! the set of accepted networks.
    use super::{ConsumerConfig, ConsumerConfigError, VALID_NETWORKS};

    #[test]
    fn accepts_every_network_the_schema_accepts() {
        for net in VALID_NETWORKS {
            let cfg = ConsumerConfig::new("test".to_owned(), (*net).to_owned())
                .unwrap_or_else(|e| panic!("{net} should be accepted: {e}"));
            assert_eq!(cfg.network, *net);
        }
    }

    #[test]
    fn rejects_obvious_typos() {
        // `kaspatest` is the address bech32 prefix, not the
        // schema-network value — easy operator confusion. Make
        // sure the constructor catches it.
        let err = ConsumerConfig::new("test".to_owned(), "kaspatest".to_owned())
            .expect_err("must reject bech32 prefix");
        assert!(matches!(err, ConsumerConfigError::InvalidNetwork { .. }));
    }

    #[test]
    fn rejects_legacy_capitalised_value() {
        // The schema CHECK is case-sensitive; `Mainnet` would
        // pass `ConsumerConfig::new` only to fail every DB call.
        // Fail fast at startup instead.
        assert!(ConsumerConfig::new("test".to_owned(), "Mainnet".to_owned()).is_err());
    }

    #[test]
    fn rejects_empty_string() {
        assert!(ConsumerConfig::new("test".to_owned(), String::new()).is_err());
    }

    #[test]
    fn schema_network_list_matches_migration() {
        // The full list is duplicated in
        // `crates/katpool-db/migrations/20260526000000_bootstrap.sql`
        // (`wallet_network_valid` CHECK). Treat this assertion as
        // a compile-time-style contract: if the migration ever
        // adds or removes a network, this test must be updated in
        // the same commit so the two stay in lock-step.
        assert_eq!(
            VALID_NETWORKS,
            &["mainnet", "testnet-10", "testnet-11", "devnet", "simnet"]
        );
    }
}
