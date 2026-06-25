//! Restart-safe executor for the KRC-20 commit/reveal state machine.
//!
//! Drives one [`Krc20PendingTransfer`] across its lifecycle —
//! `pending → commit_submitted → reveal_submitted → completed` — reusing the
//! Phase 4 KAS scaffolding for everything chain-facing: the [`KaspadClient`]
//! RPC trait, the maturity gate ([`is_spendable`]), and the confirmation
//! policy ([`classify_confirmation`], same `KAS_PAYOUT_CONFIRMATION_DAA`
//! finality depth).
//!
//! # Crash-safety contract
//!
//! Every broadcast is preceded by an atomic *record-before-broadcast* step:
//! the deterministic txid (signature scripts excluded, see [`crate::sign`]) is
//! written to the parent payout row **and** the transfer advanced one state,
//! in a single Postgres transaction, *before* the transaction hits the wire.
//! A crash anywhere after the record re-derives the identical txid on resume
//! from the same inputs, so re-broadcast is a no-op for kaspad and never
//! double-pays.
//!
//! The resume path is defensive about UTXO drift: if a recorded commit is
//! neither on chain (its P2SH output) nor reproducible from the *current*
//! treasury UTXO set, the executor refuses to broadcast a divergent commit and
//! surfaces [`Krc20ExecuteError::CommitDrift`] for an operator instead of
//! risking a second, distinct spend.
//!
//! Scope (M5.4b): this module owns the per-transfer state machine and its
//! chain interaction. Wiring `payout.status`/cycle reconciliation and the
//! end-to-end engine is M5.5; here the `krc20_pending_transfer` row is the
//! source of truth and only the commit/reveal hashes are written onto the
//! payout row.

use std::collections::HashSet;

use kaspa_addresses::{Address, Prefix};
use kaspa_consensus_core::tx::{ScriptPublicKey, TransactionId, TransactionOutpoint, UtxoEntry};
use kaspa_txscript::pay_to_address_script;
use katpool_db::DbError;
use katpool_db::repo::payout::{self, Krc20PendingTransfer, Krc20TransferStatus, Payout};
use katpool_db::repo::wallet;
use katpool_domain::BlockHash;
use katpool_secrets::TreasurySecret;
use katpool_storagemass::{FeeRate, MassEvaluator, TreasuryUtxo};
use payout_kas::{
    ConfirmationInputs, ConfirmationState, ExecutionMode, KaspadClient, KaspadError,
    TreasuryUtxoSnapshot, classify_confirmation, is_spendable,
};
use secp256k1::Keypair;
use sqlx::PgPool;
use tracing::warn;

use crate::inscription::{Krc20Transfer, commit_address};
use crate::plan::{
    CommitRevealConfig, Krc20FeePolicy, PlanError, PlannedCommitReveal, plan_commit_reveal,
};
use crate::sign::{
    COMMIT_CHANGE_OUTPUT_INDEX, COMMIT_P2SH_OUTPUT_INDEX, SignError, commit_txid, reveal_txid,
    sign_commit, sign_reveal,
};

/// KRC-20 token ticker paid by the NACHO rebate engine.
const NACHO_TICK: &str = "NACHO";

/// The state transition a single [`advance_transfer`] call performed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferStep {
    /// A fresh commit was recorded and broadcast (`pending → commit_submitted`).
    CommitBroadcast,
    /// A previously recorded commit was re-broadcast on resume (idempotent).
    CommitRebroadcast,
    /// The commit is recorded but not yet spendable; waiting on chain/mempool.
    CommitPending,
    /// The reveal was recorded and broadcast (`commit_submitted → reveal_submitted`).
    RevealBroadcast,
    /// A previously recorded reveal was re-broadcast on resume (idempotent).
    RevealRebroadcast,
    /// The reveal is recorded but below confirmation depth; still waiting.
    RevealPending,
    /// The reveal confirmed; the transfer is `completed`.
    Completed,
    /// Nothing to do (terminal state, or dry-run with no side effects).
    NoChange,
}

/// Aggregate outcome of a [`settle_pending`] sweep.
#[derive(Debug, Clone, Default)]
pub struct SettleReport {
    /// Commits broadcast for the first time.
    pub commits_broadcast: usize,
    /// Reveals broadcast for the first time.
    pub reveals_broadcast: usize,
    /// Re-broadcasts of already-recorded commits/reveals (crash recovery).
    pub rebroadcasts: usize,
    /// Transfers that reached `completed` this sweep.
    pub completed: usize,
    /// Transfers still awaiting inclusion/maturity/confirmation.
    pub pending: usize,
    /// Per-transfer non-fatal errors (one bad transfer never blocks others).
    pub errors: Vec<String>,
}

impl SettleReport {
    const fn record(&mut self, step: TransferStep) {
        match step {
            TransferStep::CommitBroadcast => self.commits_broadcast += 1,
            TransferStep::RevealBroadcast => self.reveals_broadcast += 1,
            TransferStep::CommitRebroadcast | TransferStep::RevealRebroadcast => {
                self.rebroadcasts += 1;
            }
            TransferStep::Completed => self.completed += 1,
            TransferStep::CommitPending | TransferStep::RevealPending => self.pending += 1,
            TransferStep::NoChange => {}
        }
    }
}

/// Per-sweep UTXO bookkeeping that lets sibling commits in one [`settle_pending`]
/// sweep chain off each other instead of colliding on the same treasury coin.
///
/// `get_utxos_by_addresses` returns only the **confirmed** UTXO set, so a commit
/// just broadcast (and still in the mempool) does not yet remove its spent coin
/// nor surface its change. Planning every fresh commit in a sweep against that
/// stale snapshot makes the greedy, largest-first funder pick the *same* coin
/// for all of them — only the first is accepted and the rest are rejected as
/// double-spends (and then strand on [`Krc20ExecuteError::CommitDrift`]).
///
/// Mirroring the KAS planner ([`payout_kas`]'s `plan_batches`), this ledger
/// removes the inputs each commit consumes and re-injects its change output —
/// keyed by the **real** signed commit txid, since the KRC-20 signer rejects
/// planning-virtual inputs — so the next transfer funds from a coherent set.
/// On a crash mid-sweep the chained change coin stays the largest, so the
/// resume rebuild re-selects it and reproduces the recorded txid; the existing
/// drift guard covers the residual window without ever double-spending.
#[derive(Debug, Default)]
struct SweepLedger {
    /// Treasury outpoints already consumed by earlier commits in this sweep.
    consumed: HashSet<TransactionOutpoint>,
    /// Change outputs minted by earlier commits this sweep, now spendable.
    chained: Vec<TreasuryUtxo>,
}

impl SweepLedger {
    /// Reconcile a freshly fetched (confirmed-only) UTXO set with this sweep's
    /// in-flight commits: drop coins already spent, add coins already minted.
    fn reconcile(&self, utxos: &mut Vec<TreasuryUtxo>) {
        if self.consumed.is_empty() && self.chained.is_empty() {
            return;
        }
        utxos.retain(|u| !self.consumed.contains(&u.outpoint));
        utxos.extend(self.chained.iter().cloned());
    }

    /// Record the inputs a just-signed commit consumed and re-inject its change
    /// output (if any) so the next transfer in the sweep can chain off it.
    fn note_commit(
        &mut self,
        commit_id: TransactionId,
        plan: &PlannedCommitReveal,
        treasury_script: &ScriptPublicKey,
    ) {
        for input in &plan.commit_inputs {
            self.consumed.insert(input.outpoint);
        }
        // A chained coin consumed by this commit must not be re-offered.
        self.chained
            .retain(|u| !self.consumed.contains(&u.outpoint));
        if plan.commit_change_sompi > 0 {
            self.chained.push(TreasuryUtxo {
                outpoint: TransactionOutpoint {
                    transaction_id: commit_id,
                    index: COMMIT_CHANGE_OUTPUT_INDEX,
                },
                entry: UtxoEntry {
                    amount: plan.commit_change_sompi,
                    script_public_key: treasury_script.clone(),
                    block_daa_score: 0,
                    is_coinbase: false,
                    covenant_id: None,
                },
            });
        }
    }
}

/// Failures that abort processing of a single transfer.
#[derive(Debug, thiserror::Error)]
pub enum Krc20ExecuteError {
    /// Database error.
    #[error("db: {0}")]
    Db(#[from] DbError),

    /// kaspad RPC error.
    #[error("kaspad: {0}")]
    Kaspad(#[from] KaspadError),

    /// Planning error (inscription, funding, mass, or sub-floor return).
    #[error("plan: {0}")]
    Plan(#[from] PlanError),

    /// Inscription/address derivation error.
    #[error("inscription: {0}")]
    Inscription(#[from] crate::inscription::InscriptionError),

    /// Signing or post-sign verification error.
    #[error("sign: {0}")]
    Sign(#[from] SignError),

    /// The treasury secret is not a valid secp256k1 key.
    #[error("invalid treasury key")]
    Key(secp256k1::Error),

    /// A persisted amount (NACHO units or commit lock) is negative/out of range.
    #[error("non-representable amount for payout {payout_id}: {field}={value}")]
    Amount {
        /// Parent payout id.
        payout_id: i64,
        /// Offending column.
        field: &'static str,
        /// Stored value.
        value: i64,
    },

    /// A recorded tx hash is not 32 bytes.
    #[error("malformed tx hash on payout {payout_id}")]
    MalformedHash {
        /// Parent payout id.
        payout_id: i64,
    },

    /// The transfer expected a recorded hash that is absent.
    #[error("missing {kind} hash on payout {payout_id}")]
    MissingHash {
        /// Parent payout id.
        payout_id: i64,
        /// `"commit"` or `"reveal"`.
        kind: &'static str,
    },

    /// A submitted/in-flight transfer is missing its frozen network fees,
    /// so its commit/reveal cannot be reconstructed deterministically.
    #[error("missing frozen {kind} fee on payout {payout_id}")]
    MissingFee {
        /// Parent payout id.
        payout_id: i64,
        /// `"commit"` or `"reveal"`.
        kind: &'static str,
    },

    /// The stored P2SH address does not match the rebuilt inscription —
    /// configuration drift (wrong key, ticker, amount, or recipient).
    #[error("inscription drift on payout {payout_id}: stored {stored}, rebuilt {rebuilt}")]
    InscriptionMismatch {
        /// Parent payout id.
        payout_id: i64,
        /// P2SH address persisted at planning.
        stored: String,
        /// P2SH address derived from the current inscription.
        rebuilt: String,
    },

    /// On resume, the recorded commit is neither on chain nor reproducible
    /// from the live treasury UTXO set; refuse to broadcast a divergent spend.
    #[error("commit drift on payout {payout_id}: recorded {recorded}, would rebuild {rebuilt}")]
    CommitDrift {
        /// Parent payout id.
        payout_id: i64,
        /// The recorded (already-intended) commit txid.
        recorded: String,
        /// The txid the current UTXO set would produce.
        rebuilt: String,
    },

    /// On resume, the recorded reveal is absent and the reconstructed reveal
    /// would produce a different txid (commit outpoint drift).
    #[error("reveal drift on payout {payout_id}: recorded {recorded}, would rebuild {rebuilt}")]
    RevealDrift {
        /// Parent payout id.
        payout_id: i64,
        /// The recorded reveal txid.
        recorded: String,
        /// The txid the current reconstruction would produce.
        rebuilt: String,
    },
}

/// Everything derived once per transfer and shared by the state handlers.
struct TransferCtx<'a> {
    secret: &'a TreasurySecret,
    treasury_address: &'a Address,
    treasury_script: ScriptPublicKey,
    prefix: Prefix,
    xonly: [u8; 32],
    /// Live fee-rate, used to size fees on the **first** plan of a transfer.
    fee_rate: FeeRate,
    /// Commit fee frozen at first execution (`None` for a fresh transfer).
    frozen_commit_fee: Option<u64>,
    /// Reveal fee frozen at first execution (`None` for a fresh transfer).
    frozen_reveal_fee: Option<u64>,
    inscription: Krc20Transfer,
    commit_amount_sompi: u64,
    payout_id: i64,
    p2sh_address: String,
}

impl TransferCtx<'_> {
    /// The frozen reveal fee, required once the transfer is in flight.
    fn reveal_fee(&self) -> Result<u64, Krc20ExecuteError> {
        self.frozen_reveal_fee.ok_or(Krc20ExecuteError::MissingFee {
            payout_id: self.payout_id,
            kind: "reveal",
        })
    }

    /// The frozen fee policy, replayed for deterministic crash-resume of an
    /// already-recorded commit/reveal.
    fn frozen_policy(&self) -> Result<Krc20FeePolicy, Krc20ExecuteError> {
        let commit_fee_sompi = self
            .frozen_commit_fee
            .ok_or(Krc20ExecuteError::MissingFee {
                payout_id: self.payout_id,
                kind: "commit",
            })?;
        Ok(Krc20FeePolicy::Frozen {
            commit_fee_sompi,
            reveal_fee_sompi: self.reveal_fee()?,
        })
    }
}

/// Convert a persisted optional fee column to `u64`, rejecting a negative
/// value (which the CHECK constraint forbids, but defend in depth).
fn opt_fee(
    value: Option<i64>,
    payout_id: i64,
    field: &'static str,
) -> Result<Option<u64>, Krc20ExecuteError> {
    value
        .map(|v| {
            u64::try_from(v).map_err(|_| Krc20ExecuteError::Amount {
                payout_id,
                field,
                value: v,
            })
        })
        .transpose()
}

/// Process every actionable transfer (`pending`, `commit_submitted`,
/// `reveal_submitted`) up to `limit`, advancing each at most one state.
///
/// Per-transfer errors are collected into [`SettleReport::errors`] so one bad
/// transfer never blocks the rest; infrastructure errors surface from the
/// initial load.
///
/// # Errors
///
/// [`Krc20ExecuteError::Db`] if the transfer list cannot be loaded.
pub async fn settle_pending<C: KaspadClient + Sync>(
    pool: &PgPool,
    client: &C,
    secret: &TreasurySecret,
    treasury_address: &Address,
    limit: i64,
    mode: ExecutionMode,
) -> Result<SettleReport, Krc20ExecuteError> {
    let transfers = payout::list_krc20_by_status(
        pool,
        &[
            Krc20TransferStatus::Pending,
            Krc20TransferStatus::CommitSubmitted,
            Krc20TransferStatus::RevealSubmitted,
        ],
        limit,
    )
    .await?;

    // Size fresh transfers' fees from the live node fee-rate (frozen per-row at
    // first execution). A fee-estimate RPC failure is non-fatal: fall back to
    // the relay-minimum floor so transfers still go out. Mirrors `payout-kas`.
    let fee_rate = match client.fee_estimate_sompi_per_gram().await {
        Ok(feerate) => FeeRate::from_feerate(feerate),
        Err(e) => {
            warn!(error = %e, "fee-estimate RPC failed; using minimum relay fee floor");
            FeeRate::from_feerate(0.0)
        }
    };

    let mut report = SettleReport::default();
    // Chains sibling commits in this sweep so they fund from disjoint coins
    // (the mempool snapshot is stale) instead of double-spending one another.
    let mut ledger = SweepLedger::default();
    for transfer in &transfers {
        match advance_transfer_inner(
            pool,
            client,
            secret,
            treasury_address,
            &fee_rate,
            &mut ledger,
            transfer,
            mode,
        )
        .await
        {
            Ok(step) => report.record(step),
            Err(e) => {
                // Surface the per-transfer cause: the engine only logs the
                // aggregate error *count*, so without this the reason is lost.
                warn!(
                    transfer_id = transfer.id,
                    payout_id = transfer.payout_id,
                    error = %e,
                    "krc20 transfer settle failed; continuing with the rest"
                );
                report.errors.push(format!(
                    "krc20 transfer {} (payout {}): {e}",
                    transfer.id, transfer.payout_id
                ));
            }
        }
    }
    Ok(report)
}

/// Advance one transfer by at most one state, performing the chain reads its
/// current status requires.
///
/// # Errors
///
/// See [`Krc20ExecuteError`].
pub async fn advance_transfer<C: KaspadClient + Sync>(
    pool: &PgPool,
    client: &C,
    secret: &TreasurySecret,
    treasury_address: &Address,
    fee_rate: &FeeRate,
    transfer: &Krc20PendingTransfer,
    mode: ExecutionMode,
) -> Result<TransferStep, Krc20ExecuteError> {
    // A single-transfer advance has no siblings to chain against.
    advance_transfer_inner(
        pool,
        client,
        secret,
        treasury_address,
        fee_rate,
        &mut SweepLedger::default(),
        transfer,
        mode,
    )
    .await
}

/// [`advance_transfer`] with the per-sweep [`SweepLedger`] threaded in so a
/// fresh commit funds from a set that accounts for siblings already committed
/// in this sweep (see [`SweepLedger`]).
#[allow(clippy::too_many_arguments)]
async fn advance_transfer_inner<C: KaspadClient + Sync>(
    pool: &PgPool,
    client: &C,
    secret: &TreasurySecret,
    treasury_address: &Address,
    fee_rate: &FeeRate,
    ledger: &mut SweepLedger,
    transfer: &Krc20PendingTransfer,
    mode: ExecutionMode,
) -> Result<TransferStep, Krc20ExecuteError> {
    if matches!(
        transfer.status,
        Krc20TransferStatus::Completed | Krc20TransferStatus::Failed
    ) {
        return Ok(TransferStep::NoChange);
    }

    let keypair = Keypair::from_seckey_slice(secp256k1::SECP256K1, secret.expose_secret())
        .map_err(Krc20ExecuteError::Key)?;
    let xonly = keypair.x_only_public_key().0.serialize();

    let payout_row = payout::get_payout(pool, transfer.payout_id).await?;
    let recipient = wallet::get_by_id(pool, payout_row.wallet_id).await?;

    let nacho_amount =
        u64::try_from(transfer.nacho_amount).map_err(|_| Krc20ExecuteError::Amount {
            payout_id: transfer.payout_id,
            field: "nacho_amount",
            value: transfer.nacho_amount,
        })?;
    let commit_amount_sompi =
        u64::try_from(transfer.sompi_to_miner).map_err(|_| Krc20ExecuteError::Amount {
            payout_id: transfer.payout_id,
            field: "sompi_to_miner",
            value: transfer.sompi_to_miner,
        })?;

    let ctx = TransferCtx {
        secret,
        treasury_address,
        treasury_script: pay_to_address_script(treasury_address),
        prefix: treasury_address.prefix,
        xonly,
        fee_rate: *fee_rate,
        frozen_commit_fee: opt_fee(transfer.commit_fee_sompi, transfer.payout_id, "commit")?,
        frozen_reveal_fee: opt_fee(transfer.reveal_fee_sompi, transfer.payout_id, "reveal")?,
        inscription: Krc20Transfer::new(NACHO_TICK, nacho_amount.to_string(), recipient.address),
        commit_amount_sompi,
        payout_id: transfer.payout_id,
        p2sh_address: transfer.p2sh_address.clone(),
    };

    match transfer.status {
        Krc20TransferStatus::Pending => handle_pending(pool, client, &ctx, ledger, mode).await,
        Krc20TransferStatus::CommitSubmitted => {
            handle_commit_submitted(pool, client, &ctx, &payout_row, mode).await
        }
        Krc20TransferStatus::RevealSubmitted => {
            handle_reveal_submitted(pool, client, &ctx, &payout_row, mode).await
        }
        Krc20TransferStatus::Completed | Krc20TransferStatus::Failed => Ok(TransferStep::NoChange),
    }
}

// ---- state handlers -------------------------------------------------

async fn handle_pending<C: KaspadClient + Sync>(
    pool: &PgPool,
    client: &C,
    ctx: &TransferCtx<'_>,
    ledger: &mut SweepLedger,
    mode: ExecutionMode,
) -> Result<TransferStep, Krc20ExecuteError> {
    // A fresh transfer sizes both fees from the live fee-rate; they are then
    // frozen onto the row so every later reconstruction reproduces this exact
    // commit/reveal. Fund it from the live spendable set reconciled with this
    // sweep's in-flight commits, so siblings chain instead of double-spending.
    let mut utxos = fetch_spendable_utxos(pool, client, ctx.treasury_address).await?;
    ledger.reconcile(&mut utxos);
    let plan = plan_from_utxos(ctx, &utxos, Krc20FeePolicy::Adaptive(ctx.fee_rate))?;
    verify_p2sh(ctx, &plan)?;

    let signed = sign_commit(&plan, &ctx.treasury_script, ctx.secret)?;
    let commit_id = signed.txid();

    // Chain the consumed inputs + minted change forward so the next transfer in
    // this sweep funds from a coherent set (applies in dry-run too, so the
    // rehearsal validates multi-recipient cycles without false contention).
    ledger.note_commit(commit_id, &plan, &ctx.treasury_script);

    if matches!(mode, ExecutionMode::DryRun) {
        return Ok(TransferStep::NoChange);
    }

    let commit_fee =
        i64::try_from(plan.commit_fee_sompi).map_err(|_| Krc20ExecuteError::Amount {
            payout_id: ctx.payout_id,
            field: "commit_fee_sompi",
            value: i64::MAX,
        })?;
    let reveal_fee =
        i64::try_from(plan.reveal_fee_sompi).map_err(|_| Krc20ExecuteError::Amount {
            payout_id: ctx.payout_id,
            field: "reveal_fee_sompi",
            value: i64::MAX,
        })?;

    // Record intent (frozen fees + commit hash + state) atomically, before the
    // tx hits the wire, so a crash-resume re-derives the identical txid.
    let mut db = pool.begin().await.map_err(DbError::from)?;
    payout::record_krc20_fees(&mut *db, ctx.payout_id, commit_fee, reveal_fee).await?;
    payout::record_krc20_commit_hash(&mut *db, ctx.payout_id, txid_to_hash(commit_id)).await?;
    payout::mark_krc20_commit_submitted(&mut *db, ctx.payout_id).await?;
    db.commit().await.map_err(DbError::from)?;

    client.submit_transaction(&signed.tx, false).await?;
    Ok(TransferStep::CommitBroadcast)
}

async fn handle_commit_submitted<C: KaspadClient + Sync>(
    pool: &PgPool,
    client: &C,
    ctx: &TransferCtx<'_>,
    payout_row: &Payout,
    mode: ExecutionMode,
) -> Result<TransferStep, Krc20ExecuteError> {
    let recorded = recorded_txid(
        payout_row.krc20_commit_hash.as_deref(),
        ctx.payout_id,
        "commit",
    )?;

    // Reveal-only reconstruction: the redeem script + P2SH address it spends.
    // The reveal fee was frozen when the commit was recorded, so the reveal
    // return (and txid) re-derives identically.
    let reveal_plan = PlannedCommitReveal::reveal_only(
        &ctx.xonly,
        &ctx.inscription,
        ctx.commit_amount_sompi,
        ctx.reveal_fee()?,
    )?;
    let commit_addr = commit_address(&reveal_plan.redeem_script, ctx.prefix)?;
    let commit_outpoint = TransactionOutpoint {
        transaction_id: recorded,
        index: COMMIT_P2SH_OUTPUT_INDEX,
    };

    let virtual_daa = client.virtual_daa_score().await?;
    let p2sh_utxos = client.treasury_utxos(&commit_addr).await?;
    let on_chain = p2sh_utxos.iter().find(|s| s.outpoint == commit_outpoint);

    match on_chain {
        Some(s) if is_spendable(s.entry.block_daa_score, s.entry.is_coinbase, virtual_daa) => {
            broadcast_reveal(pool, client, ctx, &reveal_plan, commit_outpoint, mode).await
        }
        // On chain but not yet matured — wait.
        Some(_) => Ok(TransferStep::CommitPending),
        None => {
            if client.transaction_in_mempool(recorded).await? {
                return Ok(TransferStep::CommitPending);
            }
            if matches!(mode, ExecutionMode::DryRun) {
                return Ok(TransferStep::NoChange);
            }
            // Neither on chain nor in mempool: crash-before-broadcast (or a
            // dropped mempool entry). Rebuild from live UTXOs with the frozen
            // fees; only re-broadcast if it reproduces the recorded txid —
            // otherwise treasury UTXOs drifted and a new commit would be a
            // distinct spend.
            let utxos = fetch_spendable_utxos(pool, client, ctx.treasury_address).await?;
            let plan = plan_from_utxos(ctx, &utxos, ctx.frozen_policy()?)?;
            let rebuilt = commit_txid(&plan, &ctx.treasury_script)?;
            if rebuilt != recorded {
                return Err(Krc20ExecuteError::CommitDrift {
                    payout_id: ctx.payout_id,
                    recorded: recorded.to_string(),
                    rebuilt: rebuilt.to_string(),
                });
            }
            let signed = sign_commit(&plan, &ctx.treasury_script, ctx.secret)?;
            client.submit_transaction(&signed.tx, false).await?;
            Ok(TransferStep::CommitRebroadcast)
        }
    }
}

async fn handle_reveal_submitted<C: KaspadClient + Sync>(
    pool: &PgPool,
    client: &C,
    ctx: &TransferCtx<'_>,
    payout_row: &Payout,
    mode: ExecutionMode,
) -> Result<TransferStep, Krc20ExecuteError> {
    let reveal_recorded = recorded_txid(
        payout_row.krc20_reveal_hash.as_deref(),
        ctx.payout_id,
        "reveal",
    )?;

    let virtual_daa = client.virtual_daa_score().await?;
    let treasury_utxos = client.treasury_utxos(ctx.treasury_address).await?;
    // The reveal's return coin lands at the treasury bearing the reveal txid.
    let on_chain_daa = treasury_utxos
        .iter()
        .find(|s| s.outpoint.transaction_id == reveal_recorded)
        .map(|s| s.entry.block_daa_score);
    let in_mempool = if on_chain_daa.is_some() {
        false
    } else {
        client.transaction_in_mempool(reveal_recorded).await?
    };

    let state = classify_confirmation(ConfirmationInputs {
        virtual_daa_score: virtual_daa,
        in_mempool,
        change_block_daa_score: on_chain_daa,
        // The reveal return coin is held back from consolidation until this
        // payout settles (see `payout::in_flight_spend_tx_hashes`), so the live
        // observation above is sufficient and no recorded fallback is needed.
        recorded_accept_daa: None,
    });

    match state {
        ConfirmationState::Confirmed => {
            if matches!(mode, ExecutionMode::DryRun) {
                return Ok(TransferStep::NoChange);
            }
            payout::mark_krc20_completed(pool, ctx.payout_id).await?;
            Ok(TransferStep::Completed)
        }
        ConfirmationState::Accepted | ConfirmationState::Pending => Ok(TransferStep::RevealPending),
        ConfirmationState::Unknown => {
            if matches!(mode, ExecutionMode::DryRun) {
                return Ok(TransferStep::NoChange);
            }
            // Reveal absent from chain and mempool: crash-before-broadcast.
            // Rebuild deterministically from the recorded commit outpoint.
            let commit_recorded = recorded_txid(
                payout_row.krc20_commit_hash.as_deref(),
                ctx.payout_id,
                "commit",
            )?;
            let commit_outpoint = TransactionOutpoint {
                transaction_id: commit_recorded,
                index: COMMIT_P2SH_OUTPUT_INDEX,
            };
            let reveal_plan = PlannedCommitReveal::reveal_only(
                &ctx.xonly,
                &ctx.inscription,
                ctx.commit_amount_sompi,
                ctx.reveal_fee()?,
            )?;
            let rebuilt = reveal_txid(&reveal_plan, commit_outpoint, &ctx.treasury_script);
            if rebuilt != reveal_recorded {
                return Err(Krc20ExecuteError::RevealDrift {
                    payout_id: ctx.payout_id,
                    recorded: reveal_recorded.to_string(),
                    rebuilt: rebuilt.to_string(),
                });
            }
            let signed = sign_reveal(
                &reveal_plan,
                commit_outpoint,
                &ctx.treasury_script,
                ctx.secret,
            )?;
            client.submit_transaction(&signed.tx, false).await?;
            Ok(TransferStep::RevealRebroadcast)
        }
    }
}

/// Record the reveal intent atomically, then broadcast it.
async fn broadcast_reveal<C: KaspadClient + Sync>(
    pool: &PgPool,
    client: &C,
    ctx: &TransferCtx<'_>,
    reveal_plan: &PlannedCommitReveal,
    commit_outpoint: TransactionOutpoint,
    mode: ExecutionMode,
) -> Result<TransferStep, Krc20ExecuteError> {
    let signed = sign_reveal(
        reveal_plan,
        commit_outpoint,
        &ctx.treasury_script,
        ctx.secret,
    )?;
    let reveal_id = signed.txid();

    if matches!(mode, ExecutionMode::DryRun) {
        return Ok(TransferStep::NoChange);
    }

    let mut db = pool.begin().await.map_err(DbError::from)?;
    payout::record_krc20_reveal_hash(&mut *db, ctx.payout_id, txid_to_hash(reveal_id)).await?;
    payout::mark_krc20_reveal_submitted(&mut *db, ctx.payout_id).await?;
    db.commit().await.map_err(DbError::from)?;

    client.submit_transaction(&signed.tx, false).await?;
    Ok(TransferStep::RevealBroadcast)
}

// ---- helpers --------------------------------------------------------

/// Fetch the treasury's live UTXO set, filtered to mature/spendable coins, with
/// the change/return coins of not-yet-terminal payouts held back.
///
/// Confirmation (KAS and KRC-20 alike) detects acceptance from a payout's
/// treasury change/return coin, so funding a commit from it before that payout
/// settles would strand the payout at a non-terminal status forever — the same
/// hazard consolidation and KAS payouts guard against (see
/// [`payout::in_flight_spend_tx_hashes`]). Applied to both the fresh-funding and
/// resume-rebuild paths so a resumed commit reproduces its recorded txid.
async fn fetch_spendable_utxos<C: KaspadClient + Sync>(
    pool: &PgPool,
    client: &C,
    treasury_address: &Address,
) -> Result<Vec<TreasuryUtxo>, Krc20ExecuteError> {
    let virtual_daa = client.virtual_daa_score().await?;
    let protected: HashSet<[u8; 32]> = payout::in_flight_spend_tx_hashes(pool)
        .await?
        .into_iter()
        .filter_map(|h| <[u8; 32]>::try_from(h.as_slice()).ok())
        .collect();
    let snapshots = client.treasury_utxos(treasury_address).await?;
    Ok(snapshots
        .into_iter()
        .filter(|s| is_spendable(s.entry.block_daa_score, s.entry.is_coinbase, virtual_daa))
        .filter(|s| !protected.contains(&s.outpoint.transaction_id.as_bytes()))
        .map(TreasuryUtxoSnapshot::into_treasury_utxo)
        .collect())
}

/// Plan a commit/reveal against the supplied spendable treasury UTXO set under
/// the given fee policy ([`Krc20FeePolicy::Adaptive`] for a fresh transfer,
/// [`Krc20FeePolicy::Frozen`] to deterministically replay an in-flight one).
fn plan_from_utxos(
    ctx: &TransferCtx<'_>,
    utxos: &[TreasuryUtxo],
    fee_policy: Krc20FeePolicy,
) -> Result<PlannedCommitReveal, Krc20ExecuteError> {
    let cfg = CommitRevealConfig {
        commit_amount_sompi: ctx.commit_amount_sompi,
        fee_policy,
    };
    let evaluator = MassEvaluator::mainnet();
    let plan = plan_commit_reveal(
        &evaluator,
        utxos,
        &ctx.treasury_script,
        &ctx.xonly,
        &ctx.inscription,
        &cfg,
    )?;
    Ok(plan)
}

/// Guard against inscription/config drift: the P2SH the plan pays must equal
/// the address persisted when the transfer was first planned.
fn verify_p2sh(ctx: &TransferCtx<'_>, plan: &PlannedCommitReveal) -> Result<(), Krc20ExecuteError> {
    let rebuilt = commit_address(&plan.redeem_script, ctx.prefix)?.to_string();
    if rebuilt != ctx.p2sh_address {
        return Err(Krc20ExecuteError::InscriptionMismatch {
            payout_id: ctx.payout_id,
            stored: ctx.p2sh_address.clone(),
            rebuilt,
        });
    }
    Ok(())
}

const fn txid_to_hash(id: TransactionId) -> BlockHash {
    BlockHash::from_bytes(id.as_bytes())
}

fn recorded_txid(
    hash: Option<&[u8]>,
    payout_id: i64,
    kind: &'static str,
) -> Result<TransactionId, Krc20ExecuteError> {
    let bytes = hash.ok_or(Krc20ExecuteError::MissingHash { payout_id, kind })?;
    let arr: [u8; 32] = bytes
        .try_into()
        .map_err(|_| Krc20ExecuteError::MalformedHash { payout_id })?;
    Ok(TransactionId::from_bytes(arr))
}
