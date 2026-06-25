//! Treasury UTXO consolidation planner (pure, mass-aware).
//!
//! Where [`crate::plan_batches`] is recipient-driven and *minimises* inputs per
//! transaction, consolidation does the opposite: it *maximises* inputs into
//! mass-valid N→1 self-sends so that many tiny coins (e.g. ~3.9-KAS coinbase
//! rewards) collapse into a few large ones. This keeps the treasury UTXO count
//! under a ceiling and, critically, lets a later payout cycle source a large
//! single-recipient amount from few coins within the per-transaction mass limit.
//!
//! ## Why a self-send is cheap
//!
//! A consolidation transaction has exactly one output (the merged coin returned
//! to the treasury). KIP-9 relaxes storage mass for `|O| = 1`, so storage mass
//! is ~0 and **compute mass binds** — it grows only with the input count. Mass
//! is therefore a pure function of the input/output *counts* (every treasury
//! input shares one script and a fixed-length signature placeholder, see
//! [`crate::build_populated`]), so the per-transaction input ceiling is constant
//! and found once via [`max_inputs_that_fit`].
//!
//! The planner records each batch's change as the **gross** input sum; the
//! executor's exact-fee signer reserves the real network fee out of it before
//! broadcast (mirroring the payout path), so the planner never has to model the
//! fee beyond a sanity guard that the inputs exceed it.

use kaspa_consensus_core::tx::{PopulatedTransaction, ScriptPublicKey};

use crate::evaluator::{MassEvaluator, TxMass};
use crate::fee::FeeRate;
use crate::tx_build::build_populated;
use crate::types::{PlannedBatch, TreasuryUtxo};

/// Minimum inputs worth a consolidation transaction.
///
/// A 1-input self-send merely rewrites a coin while paying a fee, so it never
/// reduces the UTXO count; require at least two so every batch makes progress.
pub const MIN_CONSOLIDATION_INPUTS: usize = 2;

/// Plan mass-valid N→1 self-send batches that compound the treasury UTXO set.
///
/// `utxos` must already be filtered to mature, spendable coins by the caller.
/// Coins are merged **smallest-first** so the most fragmenting dust is retired
/// fastest. Each batch packs up to `max_inputs_per_tx` inputs, capped by the
/// real mass limit, into one self-send back to `treasury_script`; planning stops
/// after `max_txs` batches. Any trailing remainder with fewer than
/// [`MIN_CONSOLIDATION_INPUTS`] inputs is dropped (it would not reduce the
/// count). `fee_rate` is used only as a guard that a batch's inputs cover the
/// network fee; the executor reserves the exact fee at signing time.
#[must_use]
pub fn plan_consolidation(
    evaluator: &MassEvaluator,
    mut utxos: Vec<TreasuryUtxo>,
    treasury_script: &ScriptPublicKey,
    fee_rate: &FeeRate,
    max_inputs_per_tx: usize,
    max_txs: usize,
) -> Vec<PlannedBatch> {
    // Defensive: planning-virtual coins are never broadcastable and must never
    // reach the signer; the live caller passes real coins, but drop any stray.
    utxos.retain(|u| !u.is_planning_virtual());

    if max_txs == 0
        || max_inputs_per_tx < MIN_CONSOLIDATION_INPUTS
        || utxos.len() < MIN_CONSOLIDATION_INPUTS
    {
        return Vec::new();
    }

    sort_utxos_asc(&mut utxos);

    let cap = max_inputs_that_fit(evaluator, &utxos, treasury_script, max_inputs_per_tx);
    if cap < MIN_CONSOLIDATION_INPUTS {
        return Vec::new();
    }

    let mut batches = Vec::new();
    for chunk in utxos.chunks(cap) {
        if batches.len() >= max_txs {
            break;
        }
        if chunk.len() < MIN_CONSOLIDATION_INPUTS {
            // Trailing remainder too small to be worth a transaction.
            break;
        }
        let input_sum: u64 = chunk.iter().map(|u| u.entry.amount).sum();
        let Some(mass) = evaluate_self_send(evaluator, chunk, treasury_script, input_sum) else {
            // chunk.len() <= cap, so this fits by construction; skip defensively.
            continue;
        };
        // Sanity guard (the executor reserves the exact fee): the merged coin
        // must survive the fee as a non-zero output. For consolidation the input
        // sum dwarfs any fee, so this never trips in practice.
        if fee_rate.reserves_fee() && input_sum <= fee_rate.fee_for(&mass) {
            continue;
        }
        batches.push(PlannedBatch {
            inputs: chunk.to_vec(),
            payouts: Vec::new(),
            change_amount_sompi: input_sum,
            mass,
        });
    }
    batches
}

fn sort_utxos_asc(utxos: &mut [TreasuryUtxo]) {
    utxos.sort_by(|a, b| a.entry.amount.cmp(&b.entry.amount));
}

/// Largest input count (≤ `max_inputs_per_tx`) whose single-output self-send
/// still fits the mempool standard-transaction mass limit (the bound kaspad
/// enforces at broadcast).
///
/// Mass is monotonic non-decreasing in input count for the fixed one-output
/// shape, so a binary search over prefix lengths finds the ceiling with
/// `O(log n)` evaluations.
fn max_inputs_that_fit(
    evaluator: &MassEvaluator,
    utxos: &[TreasuryUtxo],
    treasury_script: &ScriptPublicKey,
    max_inputs_per_tx: usize,
) -> usize {
    let hi_bound = max_inputs_per_tx.min(utxos.len());
    if hi_bound == 0 || fits_prefix(evaluator, utxos, treasury_script, 1).is_none() {
        return 0;
    }
    let (mut lo, mut hi) = (1usize, hi_bound);
    while lo < hi {
        let mid = lo + (hi - lo).div_ceil(2);
        if fits_prefix(evaluator, utxos, treasury_script, mid).is_some() {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    lo
}

/// Whether the first `n` UTXOs form a mass-valid self-send (returns its mass).
fn fits_prefix(
    evaluator: &MassEvaluator,
    utxos: &[TreasuryUtxo],
    treasury_script: &ScriptPublicKey,
    n: usize,
) -> Option<TxMass> {
    let inputs = utxos.get(..n)?;
    let input_sum: u64 = inputs.iter().map(|u| u.entry.amount).sum();
    evaluate_self_send(evaluator, inputs, treasury_script, input_sum)
}

/// Evaluate an N→1 self-send (inputs → single treasury change output) and
/// return its mass iff every dimension fits the **mempool standard** limit.
///
/// Consolidation transactions are broadcast, so the binding bound is the
/// mempool's `standard_tx_mass_limit` (`100_000`), not the looser consensus
/// block limit (`500_000`): a tx sized only to the block limit would be planned
/// and signed but then rejected at relay as non-standard. Capping here keeps the
/// per-tx input ceiling at the largest count kaspad will actually accept.
fn evaluate_self_send(
    evaluator: &MassEvaluator,
    inputs: &[TreasuryUtxo],
    treasury_script: &ScriptPublicKey,
    change_amount: u64,
) -> Option<TxMass> {
    let (tx, entries) = build_populated(inputs, &[], treasury_script, change_amount);
    let populated = PopulatedTransaction::new(&tx, entries);
    let mass = evaluator.evaluate_populated(&populated).ok()?;
    mass.fits_independently(evaluator.standard_tx_mass_limit())
        .then_some(mass)
}
