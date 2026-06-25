//! Greedy, mass-aware payout batch planner.
//!
//! Offline planning re-injects each batch's change as a virtual treasury UTXO
//! so one on-chain coin can fund many mass-limited transactions in a single
//! cycle plan. `payout-kas` must refresh the live UTXO set from kaspad
//! before signing each batch and swap virtual outpoints for confirmed change.

use kaspa_consensus_core::tx::{
    PopulatedTransaction, ScriptPublicKey, TransactionId, TransactionOutpoint, UtxoEntry,
};

use crate::evaluator::{MassEvaluator, TxMass};
use crate::fee::{FeeRate, is_change_dust};
use crate::tx_build::build_populated;
use crate::types::{
    PLANNING_VIRTUAL_TXID_BYTES, PayoutRecipient, PlanBatchesResult, PlannedBatch, TreasuryUtxo,
};

/// Heuristic from `docs/kips.md` §5.1: keep output count modest per input.
const MAX_OUTPUTS_PER_INPUT: usize = 10;

/// Partition recipients, sort funding set, and greedily pack mass-valid batches.
///
/// `fee_rate` reserves the on-chain network fee out of each batch's change so
/// the resulting transaction clears kaspad's minimum-relay-fee check. Pass
/// [`FeeRate::ZERO`] for shape-only planning that must not alter amounts.
#[must_use]
pub fn plan_batches(
    evaluator: &MassEvaluator,
    mut utxos: Vec<TreasuryUtxo>,
    recipients: Vec<PayoutRecipient>,
    change_script: &ScriptPublicKey,
    fee_rate: &FeeRate,
) -> PlanBatchesResult {
    let (mut payable, deferred_below_floor) = partition_by_floor(recipients);
    sort_utxos_desc(&mut utxos);
    sort_recipients_desc(&mut payable);

    let mut batches = Vec::new();
    while !payable.is_empty() && !utxos.is_empty() {
        let Some(batch) = build_one_batch(evaluator, &utxos, &payable, change_script, fee_rate)
        else {
            break;
        };
        remove_consumed(&mut utxos, &batch.inputs);
        remove_paid(&mut payable, &batch.payouts);
        if batch.change_amount_sompi > 0
            && let Ok(batch_index) = u32::try_from(batches.len())
        {
            utxos.push(planning_change_utxo(
                batch_index,
                batch.change_amount_sompi,
                change_script,
            ));
            sort_utxos_desc(&mut utxos);
        }
        batches.push(batch);
    }

    PlanBatchesResult {
        batches,
        deferred_below_floor,
        unpaid: payable,
    }
}

fn partition_by_floor(
    recipients: Vec<PayoutRecipient>,
) -> (Vec<PayoutRecipient>, Vec<PayoutRecipient>) {
    let floor = crate::MIN_PAYOUT_OUTPUT_SOMPI;
    let mut payable = Vec::new();
    let mut deferred = Vec::new();
    for rec in recipients {
        if rec.amount_sompi >= floor {
            payable.push(rec);
        } else {
            deferred.push(rec);
        }
    }
    (payable, deferred)
}

fn sort_utxos_desc(utxos: &mut [TreasuryUtxo]) {
    utxos.sort_by(|a, b| b.entry.amount.cmp(&a.entry.amount));
}

fn sort_recipients_desc(recipients: &mut [PayoutRecipient]) {
    recipients.sort_by(|a, b| b.amount_sompi.cmp(&a.amount_sompi));
}

fn build_one_batch(
    evaluator: &MassEvaluator,
    utxos: &[TreasuryUtxo],
    recipients: &[PayoutRecipient],
    change_script: &ScriptPublicKey,
    fee_rate: &FeeRate,
) -> Option<PlannedBatch> {
    let max_inputs = utxos.len();
    let mut input_count = 1;

    while input_count <= max_inputs {
        let inputs = utxos.get(..input_count)?;
        let input_sum: u64 = inputs.iter().map(|u| u.entry.amount).sum();

        let selected = greedy_recipients_for_inputs(
            evaluator,
            inputs,
            input_sum,
            recipients,
            change_script,
            fee_rate,
        )?;

        if selected.is_empty() {
            input_count += 1;
            continue;
        }

        let payout_sum: u64 = selected
            .iter()
            .filter_map(|&idx| recipients.get(idx).map(|r| r.amount_sompi))
            .sum();
        if payout_sum > input_sum {
            input_count += 1;
            continue;
        }

        let payout_refs: Vec<&PayoutRecipient> = selected
            .iter()
            .filter_map(|&idx| recipients.get(idx))
            .collect();
        if payout_refs.len() != selected.len() {
            input_count += 1;
            continue;
        }

        // Size the fee from the pre-fee shape (mass is insensitive to the
        // change *value*, only its presence), then reserve it from change.
        let gross_change = input_sum - payout_sum;
        let Some(gross_mass) =
            evaluate_shape(evaluator, inputs, &payout_refs, change_script, gross_change)
        else {
            input_count += 1;
            continue;
        };
        let fee = if fee_rate.reserves_fee() {
            fee_rate.fee_for(&gross_mass)
        } else {
            0
        };
        if payout_sum.saturating_add(fee) > input_sum {
            // Not enough to cover payouts plus the network fee; widen inputs.
            input_count += 1;
            continue;
        }
        let change_amount = input_sum - payout_sum - fee;

        // Drop a zero or dust change output (its value is absorbed into the
        // fee); kaspad rejects dust outputs. Re-measure the no-change shape so
        // the recorded mass matches what is actually signed.
        let drop_change = fee_rate.reserves_fee()
            && (change_amount == 0 || is_change_dust(change_amount, change_script));
        let (final_change, mass) = if drop_change {
            if let Some(mass) = evaluate_shape(evaluator, inputs, &payout_refs, change_script, 0) {
                (0, mass)
            } else {
                input_count += 1;
                continue;
            }
        } else {
            // A kept change output: reserving the fee only lowers its value,
            // which does not move mass, so reuse the measured shape.
            (change_amount, gross_mass)
        };

        let payouts: Vec<PayoutRecipient> = selected
            .iter()
            .filter_map(|&idx| recipients.get(idx).cloned())
            .collect();
        return Some(PlannedBatch {
            inputs: inputs.to_vec(),
            payouts,
            change_amount_sompi: final_change,
            mass,
        });
    }

    None
}

/// Greedily grow a recipient set (largest-first order) while mass fits.
fn greedy_recipients_for_inputs(
    evaluator: &MassEvaluator,
    inputs: &[TreasuryUtxo],
    input_sum: u64,
    recipients: &[PayoutRecipient],
    change_script: &ScriptPublicKey,
    fee_rate: &FeeRate,
) -> Option<Vec<usize>> {
    let max_outputs = inputs.len().saturating_mul(MAX_OUTPUTS_PER_INPUT).max(1);

    let mut selected: Vec<usize> = Vec::new();
    for (idx, rec) in recipients.iter().enumerate() {
        if selected.len() >= max_outputs {
            break;
        }
        let candidate_sum: u64 = selected
            .iter()
            .filter_map(|&i| recipients.get(i).map(|r| r.amount_sompi))
            .sum::<u64>()
            .saturating_add(rec.amount_sompi);
        if candidate_sum > input_sum {
            continue;
        }
        let mut candidate = selected.clone();
        candidate.push(idx);
        let payout_refs: Vec<&PayoutRecipient> = candidate
            .iter()
            .filter_map(|&i| recipients.get(i))
            .collect();
        if payout_refs.len() != candidate.len() {
            continue;
        }
        let change = input_sum - candidate_sum;
        let Some(mass) = evaluate_shape(evaluator, inputs, &payout_refs, change_script, change)
        else {
            continue;
        };
        // Only accept this recipient if the inputs still cover the network fee
        // for the resulting shape; otherwise the batch could not be broadcast.
        let fee = if fee_rate.reserves_fee() {
            fee_rate.fee_for(&mass)
        } else {
            0
        };
        if candidate_sum.saturating_add(fee) <= input_sum {
            selected = candidate;
        }
    }

    if selected.is_empty() {
        None
    } else {
        Some(selected)
    }
}

fn evaluate_shape(
    evaluator: &MassEvaluator,
    inputs: &[TreasuryUtxo],
    payouts: &[&PayoutRecipient],
    change_script: &ScriptPublicKey,
    change_amount: u64,
) -> Option<TxMass> {
    let (tx, entries) = build_populated(inputs, payouts, change_script, change_amount);
    let populated = PopulatedTransaction::new(&tx, entries);
    let mass = evaluator.evaluate_populated(&populated).ok()?;
    mass.fits_independently(evaluator.block_mass_limit())
        .then_some(mass)
}

/// Synthetic change coin for the next planning iteration (not broadcastable).
fn planning_change_utxo(
    batch_index: u32,
    amount_sompi: u64,
    change_script: &ScriptPublicKey,
) -> TreasuryUtxo {
    let transaction_id = TransactionId::from_bytes(PLANNING_VIRTUAL_TXID_BYTES);
    TreasuryUtxo {
        outpoint: TransactionOutpoint {
            transaction_id,
            index: batch_index,
        },
        entry: UtxoEntry {
            amount: amount_sompi,
            script_public_key: change_script.clone(),
            block_daa_score: 0,
            is_coinbase: false,
            covenant_id: None,
        },
    }
}

fn remove_consumed(utxos: &mut Vec<TreasuryUtxo>, consumed: &[TreasuryUtxo]) {
    utxos.retain(|u| !consumed.iter().any(|c| c.outpoint == u.outpoint));
}

fn remove_paid(recipients: &mut Vec<PayoutRecipient>, paid: &[PayoutRecipient]) {
    recipients.retain(|r| !paid.iter().any(|p| p.id == r.id));
}
