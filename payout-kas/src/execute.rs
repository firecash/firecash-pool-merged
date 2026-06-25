//! Executor: bind a planned KAS cycle to live chain state and drive it
//! through sign → submit → confirm, idempotently and crash-safely.
//!
//! ## Ordering contract (no double-pay)
//!
//! For each batch the executor:
//! 1. **Signs in memory** (no external effect; fully verified via the script
//!    engine in [`crate::signer::sign_batch`]).
//! 2. **Records intent** — marks every payout row `submitted` with the
//!    deterministic txid (which excludes signature scripts, so it is stable
//!    across restarts) in one DB transaction, committed *before* broadcast.
//! 3. **Broadcasts** to kaspad.
//!
//! A crash or transport error after step 2 leaves rows `submitted` with a
//! txid; [`confirm_cycle`] only advances a payout on a *positive* on-chain
//! signal and never auto-fails, so funds are never re-sent. The same signed
//! transaction can be safely re-broadcast (kaspad dedups by txid) by operator
//! tooling (M4.8).
//!
//! Only `planned` rows are ever signed ([`crate::CycleState::pending`]), so a
//! resumed cycle cannot re-pay a recipient already on the wire.

use std::collections::{HashMap, HashSet};

use kaspa_addresses::Address;
use kaspa_consensus_core::tx::{PopulatedTransaction, ScriptPublicKey, TransactionId};
use kaspa_txscript::pay_to_address_script;
use katpool_db::DbError;
use katpool_db::repo::payout::{self, Payout};
use katpool_db::repo::wallet;
use katpool_domain::BlockHash;
use katpool_secrets::TreasurySecret;
use katpool_storagemass::{
    FeeRate, MassEvaluationError, MassEvaluator, PayoutRecipient, PlannedBatch, TreasuryUtxo,
    is_change_dust, plan_batches,
};
use sqlx::PgPool;
use tracing::{error, warn};

use crate::client::KaspadClient;
use crate::confirm::{ConfirmationInputs, ConfirmationState, classify_confirmation, is_spendable};
use crate::signer::{SignError, SignedBatch, sign_batch};

/// Errors from executing a cycle against live chain state.
#[derive(Debug, thiserror::Error)]
pub enum ExecuteError {
    /// Database failure.
    #[error(transparent)]
    Db(#[from] DbError),

    /// kaspad RPC failure.
    #[error(transparent)]
    Kaspad(#[from] crate::client::KaspadError),

    /// Transaction assembly/signing failure.
    #[error(transparent)]
    Sign(#[from] SignError),

    /// Mass evaluation of a signed batch failed (invalid shape).
    #[error(transparent)]
    Mass(#[from] MassEvaluationError),

    /// A recipient's stored address could not be parsed.
    #[error("invalid recipient address for wallet {wallet_id}: {reason}")]
    RecipientAddress {
        /// Offending wallet id.
        wallet_id: i64,
        /// Parse error detail.
        reason: String,
    },

    /// A payout amount was not representable as a positive u64.
    #[error("payout {payout_id} has non-representable amount {amount}")]
    Amount {
        /// Offending payout id.
        payout_id: i64,
        /// Raw signed amount.
        amount: i64,
    },

    /// A recipient id produced by the planner did not map back to a payout row.
    #[error("planner returned unknown recipient id {0}")]
    UnknownRecipient(String),
}

/// Whether to broadcast for real or rehearse without touching the network/DB.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    /// Sign, record intent, and broadcast.
    Live,
    /// Sign and verify only; do not record intent or broadcast.
    DryRun,
}

impl ExecutionMode {
    /// True for [`ExecutionMode::DryRun`].
    #[must_use]
    pub const fn is_dry_run(self) -> bool {
        matches!(self, Self::DryRun)
    }
}

/// Outcome of [`broadcast_cycle`].
#[derive(Debug, Clone, Default)]
pub struct ExecutionReport {
    /// Number of mass-valid batches the planner produced.
    pub planned_batches: usize,
    /// Spendable treasury UTXOs fed to the planner.
    pub spendable_utxos: usize,
    /// txids successfully submitted (or that would be, in dry-run).
    pub submitted_txids: Vec<TransactionId>,
    /// Number of payout rows marked submitted (0 in dry-run).
    pub submitted_payouts: usize,
    /// Recipients held below the dust floor.
    pub deferred_below_floor: usize,
    /// Recipients the live UTXO set could not fund this run.
    pub unpaid: usize,
    /// Non-fatal per-batch submit errors (txid: message).
    pub submit_errors: Vec<String>,
}

/// Outcome of [`confirm_cycle`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ConfirmReport {
    /// Rows advanced to `accepted` this pass.
    pub accepted: usize,
    /// Rows advanced to `confirmed` this pass.
    pub confirmed: usize,
    /// Rows still pending in the mempool.
    pub pending: usize,
    /// Rows with no observable chain/mempool state (left untouched).
    pub unknown: usize,
}

fn treasury_script_of(address: &Address) -> ScriptPublicKey {
    pay_to_address_script(address)
}

const fn txid_to_block_hash(txid: TransactionId) -> BlockHash {
    BlockHash::from_bytes(txid.as_bytes())
}

/// Plan against live treasury UTXOs, sign, record intent, and broadcast every
/// `planned` recipient in `cycle`.
///
/// Recipients already on the wire are excluded by [`crate::CycleState::pending`].
/// In [`ExecutionMode::DryRun`] the batches are signed and verified but neither
/// recorded nor broadcast — the basis of the M4.8 rehearsal.
pub async fn broadcast_cycle<C: KaspadClient>(
    pool: &PgPool,
    client: &C,
    secret: &TreasurySecret,
    treasury_address: &Address,
    cycle: &crate::CycleState,
    mode: ExecutionMode,
) -> Result<ExecutionReport, ExecuteError> {
    let pending = cycle.pending();
    let mut report = ExecutionReport::default();
    if pending.is_empty() {
        return Ok(report);
    }

    let treasury_script = treasury_script_of(treasury_address);

    // Map planner recipient-id (the payout row id, as text) back to the row.
    let mut by_id: HashMap<String, &Payout> = HashMap::with_capacity(pending.len());
    let mut recipients: Vec<PayoutRecipient> = Vec::with_capacity(pending.len());
    for p in &pending {
        let amount = u64::try_from(p.amount_sompi).map_err(|_| ExecuteError::Amount {
            payout_id: p.id,
            amount: p.amount_sompi,
        })?;
        let w = wallet::get_by_id(pool, p.wallet_id).await?;
        let address =
            Address::try_from(w.address.as_str()).map_err(|e| ExecuteError::RecipientAddress {
                wallet_id: p.wallet_id.0,
                reason: e.to_string(),
            })?;
        let id = p.id.to_string();
        recipients.push(PayoutRecipient {
            id: id.clone(),
            amount_sompi: amount,
            script_public_key: pay_to_address_script(&address),
        });
        by_id.insert(id, *p);
    }

    // Live treasury UTXOs, filtered to mature/spendable coins. Also hold back
    // the change coins of payouts that are not yet terminal: confirmation
    // detects acceptance from a payout's treasury change coin, so spending it
    // here (to fund another recipient) before that payout settles would strand
    // it at `submitted` forever — the same hazard consolidation guards against
    // (see `payout::in_flight_spend_tx_hashes`).
    let virtual_daa = client.virtual_daa_score().await?;
    let protected: HashSet<[u8; 32]> = payout::in_flight_spend_tx_hashes(pool)
        .await?
        .into_iter()
        .filter_map(|h| <[u8; 32]>::try_from(h.as_slice()).ok())
        .collect();
    let snapshots = client.treasury_utxos(treasury_address).await?;
    let utxos: Vec<TreasuryUtxo> = snapshots
        .into_iter()
        .filter(|s| is_spendable(s.entry.block_daa_score, s.entry.is_coinbase, virtual_daa))
        .filter(|s| !protected.contains(&s.outpoint.transaction_id.as_bytes()))
        .map(crate::client::TreasuryUtxoSnapshot::into_treasury_utxo)
        .collect();
    report.spendable_utxos = utxos.len();

    // Reserve a real network fee out of change so the batch clears kaspad's
    // minimum-relay-fee check. A fee-estimate RPC failure is non-fatal: fall
    // back to the relay-minimum floor (feerate 0) so payouts still go out.
    let fee_rate = match client.fee_estimate_sompi_per_gram().await {
        Ok(feerate) => FeeRate::from_feerate(feerate),
        Err(e) => {
            warn!(error = %e, "fee-estimate RPC failed; using minimum relay fee floor");
            FeeRate::from_feerate(0.0)
        }
    };

    let evaluator = MassEvaluator::mainnet();
    let plan = plan_batches(&evaluator, utxos, recipients, &treasury_script, &fee_rate);
    report.planned_batches = plan.batches.len();
    report.deferred_below_floor = plan.deferred_below_floor.len();
    report.unpaid = plan.unpaid.len();

    if mode == ExecutionMode::Live && !plan.batches.is_empty() {
        payout::mark_cycle_broadcasting(pool, cycle.cycle.id).await?;
    }

    for batch in &plan.batches {
        // Resolve this batch's payout rows up front (also validates mapping).
        let mut batch_payout_ids: Vec<i64> = Vec::with_capacity(batch.payouts.len());
        for r in &batch.payouts {
            let row = by_id
                .get(&r.id)
                .ok_or_else(|| ExecuteError::UnknownRecipient(r.id.clone()))?;
            batch_payout_ids.push(row.id);
        }

        // Sign first (in-memory, verified) — no external effect on failure —
        // sizing the fee from the *signed* transaction's exact mass so it can
        // never diverge from what kaspad's mempool charges. The recorded txid
        // is that of the transaction actually broadcast.
        let signed =
            sign_batch_with_exact_fee(batch, &treasury_script, secret, &fee_rate, &evaluator)?;
        let txid = signed.txid();

        if mode.is_dry_run() {
            report.submitted_txids.push(txid);
            continue;
        }

        // Record intent for the whole batch atomically, BEFORE broadcast.
        let tx_hash = txid_to_block_hash(txid);
        let mut db_tx = pool.begin().await.map_err(DbError::from)?;
        for &payout_id in &batch_payout_ids {
            payout::mark_payout_submitted(&mut *db_tx, payout_id, tx_hash).await?;
        }
        db_tx.commit().await.map_err(DbError::from)?;
        report.submitted_payouts += batch_payout_ids.len();

        match client.submit_transaction(&signed.tx, false).await {
            Ok(_accepted) => report.submitted_txids.push(txid),
            Err(e) => {
                // The payout rows are already marked `submitted` (intent), but
                // the broadcast was rejected — surface it loudly so a stuck
                // cycle is never silent. confirm_cycle never auto-confirms, so
                // funds are safe and the batch can be re-broadcast by ops.
                error!(%txid, payouts = batch_payout_ids.len(), error = %e,
                    "payout broadcast rejected by kaspad");
                report.submit_errors.push(format!("{txid}: {e}"));
            }
        }
    }

    Ok(report)
}

/// Sign a batch, sizing its fee from the **signed** transaction's exact mass.
///
/// The offline planner reserves an estimated fee for batch packing, but the
/// authoritative fee must match what kaspad's mempool charges for the exact
/// bytes it validates. We sign once to obtain the true mass, recompute the fee
/// (`feerate × effective_mass`, floored at the minimum relay fee), fold the
/// difference back into the treasury change output, and re-sign if the change
/// moved. A change output that would fall to dust is dropped into the fee.
///
/// Shared with the consolidation engine ([`crate::consolidate`]), whose N→1
/// self-send batches (`payouts: []`, change to the treasury) take the same
/// exact-fee path so a merged coin never underprices its mass.
pub(crate) fn sign_batch_with_exact_fee(
    batch: &PlannedBatch,
    treasury_script: &ScriptPublicKey,
    secret: &TreasurySecret,
    fee_rate: &FeeRate,
    evaluator: &MassEvaluator,
) -> Result<SignedBatch, ExecuteError> {
    let provisional = sign_batch(batch, treasury_script, secret)?;
    if !fee_rate.reserves_fee() {
        return Ok(provisional);
    }

    let populated = PopulatedTransaction::new(&provisional.tx, provisional.entries.clone());
    let mass = evaluator.evaluate_populated(&populated)?;
    let fee = fee_rate.fee_for(&mass);

    let input_sum: u64 = batch.inputs.iter().map(|u| u.entry.amount).sum();
    let payout_sum: u64 = batch.payouts.iter().map(|p| p.amount_sompi).sum();

    // Cannot cover payouts + fee from this batch's inputs: leave the provisional
    // tx as-is so the broadcast surfaces the rejection loudly rather than
    // silently shipping a malformed change. (Fee ≪ any payout, so unreachable
    // in practice given the planner reserves room.)
    let Some(mut change) = input_sum
        .checked_sub(payout_sum)
        .and_then(|rem| rem.checked_sub(fee))
    else {
        return Ok(provisional);
    };
    if change != 0 && is_change_dust(change, treasury_script) {
        change = 0;
    }
    if change == batch.change_amount_sompi {
        return Ok(provisional);
    }

    let adjusted = PlannedBatch {
        inputs: batch.inputs.clone(),
        payouts: batch.payouts.clone(),
        change_amount_sompi: change,
        mass: batch.mass,
    };
    Ok(sign_batch(&adjusted, treasury_script, secret)?)
}

/// Poll chain state for every in-flight payout in `cycle` and advance
/// `submitted → accepted → confirmed`. Idempotent and never auto-fails.
///
/// Acceptance is detected from the treasury **change** coin bearing the payout
/// txid (which also yields the accepting DAA score for maturity). For batches
/// without change, mempool presence keeps the payout `submitted`; absence is
/// reported as `unknown` and left for operator reconciliation (M4.8).
pub async fn confirm_cycle<C: KaspadClient>(
    pool: &PgPool,
    client: &C,
    treasury_address: &Address,
    cycle: &crate::CycleState,
) -> Result<ConfirmReport, ExecuteError> {
    use katpool_db::repo::payout::PayoutStatus;

    let in_flight: Vec<&Payout> = cycle
        .payouts
        .iter()
        .filter(|p| {
            matches!(p.status, PayoutStatus::Submitted | PayoutStatus::Accepted)
                && p.tx_hash.is_some()
        })
        .collect();
    let mut report = ConfirmReport::default();
    if in_flight.is_empty() {
        return Ok(report);
    }

    let virtual_daa = client.virtual_daa_score().await?;
    let snapshots = client.treasury_utxos(treasury_address).await?;
    // txid bytes → block_daa_score of an on-chain coin this tx created (change).
    let change_daa: HashMap<[u8; 32], u64> = snapshots
        .iter()
        .map(|s| {
            (
                s.outpoint.transaction_id.as_bytes(),
                s.entry.block_daa_score,
            )
        })
        .collect();

    for p in in_flight {
        let Some(hash_bytes) = p.tx_hash.as_ref() else {
            continue;
        };
        let Ok(txid_bytes): Result<[u8; 32], _> = hash_bytes.as_slice().try_into() else {
            continue;
        };
        let on_chain = change_daa.get(&txid_bytes).copied();
        // Accepting height recorded by an earlier pass; lets us confirm by depth
        // even when the change coin has since been spent (e.g. consolidated).
        let recorded_accept_daa = p.accepted_daa_score.and_then(|v| u64::try_from(v).ok());
        let in_mempool = if on_chain.is_some() || recorded_accept_daa.is_some() {
            false
        } else {
            client
                .transaction_in_mempool(TransactionId::from_bytes(txid_bytes))
                .await?
        };

        let state = classify_confirmation(ConfirmationInputs {
            virtual_daa_score: virtual_daa,
            in_mempool,
            change_block_daa_score: on_chain,
            recorded_accept_daa,
        });
        match state {
            ConfirmationState::Accepted => {
                // Persist the accepting height first-write-wins, so a later pass
                // can confirm by depth after the change coin is gone.
                let accept_daa = on_chain
                    .or(recorded_accept_daa)
                    .and_then(|v| i64::try_from(v).ok())
                    .unwrap_or(0);
                payout::mark_payout_accepted(pool, p.id, accept_daa).await?;
                report.accepted += 1;
            }
            ConfirmationState::Confirmed => {
                payout::mark_payout_confirmed(pool, p.id).await?;
                report.confirmed += 1;
            }
            ConfirmationState::Pending => report.pending += 1,
            ConfirmationState::Unknown => report.unknown += 1,
        }
    }

    Ok(report)
}
