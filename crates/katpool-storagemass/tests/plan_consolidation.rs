//! Consolidation-planner tests: mass safety, smallest-first ordering, disjoint
//! slices, input/tx bounds, and the no-dust / fee-covered guarantees.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::cast_possible_truncation,
    clippy::indexing_slicing,
    clippy::integer_division
)]

use std::collections::HashSet;
use std::str::FromStr;

use kaspa_consensus_core::tx::{
    PopulatedTransaction, ScriptPublicKey, ScriptVec, TransactionId, TransactionOutpoint, UtxoEntry,
};
use katpool_storagemass::{
    FeeRate, MIN_CONSOLIDATION_INPUTS, MassEvaluator, TreasuryUtxo, build_populated,
    plan_consolidation,
};

fn empty_script() -> ScriptPublicKey {
    ScriptPublicKey::new(0, ScriptVec::from_slice(&[]))
}

fn sample_outpoint(index: u32) -> TransactionOutpoint {
    let tx_id =
        TransactionId::from_str("880eb9819a31821d9d2399e2f35e2433b72637e393d71ecc9b8d0250f49153c3")
            .unwrap();
    TransactionOutpoint {
        transaction_id: tx_id,
        index,
    }
}

fn treasury_utxo(index: u32, amount: u64) -> TreasuryUtxo {
    TreasuryUtxo {
        outpoint: sample_outpoint(index),
        entry: UtxoEntry {
            amount,
            script_public_key: empty_script(),
            block_daa_score: 0,
            is_coinbase: false,
            covenant_id: None,
        },
    }
}

/// Many small coinbase-sized coins (~3.9 KAS) with distinct outpoints.
fn fragmented(count: u32) -> Vec<TreasuryUtxo> {
    (0..count)
        .map(|i| treasury_utxo(i, 390_000_000 + u64::from(i)))
        .collect()
}

fn assert_all_batches_mass_valid(
    evaluator: &MassEvaluator,
    batches: &[katpool_storagemass::PlannedBatch],
) {
    for batch in batches {
        assert!(batch.payouts.is_empty(), "consolidation has no recipients");
        let (tx, entries) = build_populated(
            &batch.inputs,
            &[],
            &empty_script(),
            batch.change_amount_sompi,
        );
        let populated = PopulatedTransaction::new(&tx, entries);
        let mass = evaluator.evaluate_populated(&populated).unwrap();
        assert_eq!(mass, batch.mass, "recorded mass matches the planned shape");
        assert!(
            mass.fits_independently(evaluator.standard_tx_mass_limit()),
            "batch must fit the mempool standard mass limit (broadcastable)"
        );
        // Single output equals the gross input sum (executor reserves the fee).
        let input_sum: u64 = batch.inputs.iter().map(|u| u.entry.amount).sum();
        assert_eq!(input_sum, batch.change_amount_sompi);
        assert!(batch.change_amount_sompi > 0, "merged output is never zero");
    }
}

#[test]
fn empty_below_minimum_inputs() {
    let evaluator = MassEvaluator::mainnet();
    let script = empty_script();
    // Zero and one input cannot make net progress.
    assert!(plan_consolidation(&evaluator, vec![], &script, &FeeRate::ZERO, 350, 4).is_empty());
    let one = vec![treasury_utxo(0, 500_000_000)];
    assert!(plan_consolidation(&evaluator, one, &script, &FeeRate::ZERO, 350, 4).is_empty());
}

#[test]
fn empty_when_per_tx_cap_below_minimum() {
    let evaluator = MassEvaluator::mainnet();
    let script = empty_script();
    let utxos = fragmented(10);
    // max_inputs_per_tx of 1 can never satisfy MIN_CONSOLIDATION_INPUTS.
    assert!(plan_consolidation(&evaluator, utxos, &script, &FeeRate::ZERO, 1, 4).is_empty());
}

#[test]
fn empty_when_max_txs_zero() {
    let evaluator = MassEvaluator::mainnet();
    let script = empty_script();
    let utxos = fragmented(10);
    assert!(plan_consolidation(&evaluator, utxos, &script, &FeeRate::ZERO, 350, 0).is_empty());
}

#[test]
fn packs_within_input_and_tx_bounds() {
    let evaluator = MassEvaluator::mainnet();
    let script = empty_script();
    let utxos = fragmented(100);

    let batches = plan_consolidation(&evaluator, utxos, &script, &FeeRate::ZERO, 10, 3);
    assert_eq!(batches.len(), 3, "max_txs caps the batch count");
    for batch in &batches {
        assert!(batch.inputs.len() <= 10, "max_inputs_per_tx respected");
        assert!(batch.inputs.len() >= MIN_CONSOLIDATION_INPUTS);
    }
    assert_all_batches_mass_valid(&evaluator, &batches);
}

#[test]
fn smallest_first_ordering_across_batches() {
    let evaluator = MassEvaluator::mainnet();
    let script = empty_script();
    // Shuffle amounts so the planner must sort: descending input order.
    let utxos: Vec<_> = (0..20)
        .map(|i| treasury_utxo(i, 1_000_000_000 - u64::from(i)))
        .collect();

    let batches = plan_consolidation(&evaluator, utxos, &script, &FeeRate::ZERO, 5, 100);
    // Flatten in batch order and assert non-decreasing amounts (smallest-first).
    let mut prev = 0u64;
    for batch in &batches {
        for u in &batch.inputs {
            assert!(u.entry.amount >= prev, "inputs consumed smallest-first");
            prev = u.entry.amount;
        }
    }
    assert_all_batches_mass_valid(&evaluator, &batches);
}

#[test]
fn batches_use_disjoint_inputs() {
    let evaluator = MassEvaluator::mainnet();
    let script = empty_script();
    let utxos = fragmented(50);

    let batches = plan_consolidation(&evaluator, utxos, &script, &FeeRate::ZERO, 7, 100);
    let mut seen: HashSet<(TransactionId, u32)> = HashSet::new();
    for batch in &batches {
        for u in &batch.inputs {
            assert!(
                seen.insert((u.outpoint.transaction_id, u.outpoint.index)),
                "no UTXO appears in two batches"
            );
        }
    }
}

#[test]
fn drops_trailing_remainder_below_minimum() {
    let evaluator = MassEvaluator::mainnet();
    let script = empty_script();
    // cap = 3, 7 inputs → [3,3] consumed, trailing single input dropped.
    let utxos = fragmented(7);
    let batches = plan_consolidation(&evaluator, utxos, &script, &FeeRate::ZERO, 3, 100);
    assert_eq!(batches.len(), 2);
    let used: usize = batches.iter().map(|b| b.inputs.len()).sum();
    assert_eq!(used, 6, "the trailing 1-input remainder is not emitted");
}

#[test]
fn mass_cap_binds_when_input_limit_is_huge() {
    let evaluator = MassEvaluator::mainnet();
    let script = empty_script();
    // Far more inputs than a single tx can hold; effectively unlimited cap.
    let utxos = fragmented(2_000);

    let batches = plan_consolidation(&evaluator, utxos, &script, &FeeRate::ZERO, usize::MAX, 1);
    assert_eq!(batches.len(), 1);
    let cap = batches[0].inputs.len();
    assert!(
        cap > MIN_CONSOLIDATION_INPUTS,
        "packs many inputs, got {cap}"
    );

    // The cap is the true ceiling: one more input of the same shape overflows
    // the mempool standard mass limit (the bound the planner sizes against).
    let over: Vec<_> = (0..=(cap as u32))
        .map(|i| treasury_utxo(i, 390_000_000))
        .collect();
    let input_sum: u64 = over.iter().map(|u| u.entry.amount).sum();
    let (tx, entries) = build_populated(&over, &[], &script, input_sum);
    let populated = PopulatedTransaction::new(&tx, entries);
    let mass = evaluator.evaluate_populated(&populated).unwrap();
    assert!(
        !mass.fits_independently(evaluator.standard_tx_mass_limit()),
        "cap+1 inputs must exceed the mempool standard mass limit"
    );
    assert_all_batches_mass_valid(&evaluator, &batches);
}

#[test]
fn non_zero_fee_rate_keeps_inputs_covering_the_fee() {
    let evaluator = MassEvaluator::mainnet();
    let script = empty_script();
    let utxos = fragmented(40);

    let fee_rate = FeeRate::from_feerate(1.0);
    let batches = plan_consolidation(&evaluator, utxos, &script, &fee_rate, 8, 100);
    assert!(!batches.is_empty());
    for batch in &batches {
        let input_sum: u64 = batch.inputs.iter().map(|u| u.entry.amount).sum();
        let fee = fee_rate.fee_for(&batch.mass);
        assert!(input_sum > fee, "the merged coin survives the reserved fee");
        // Recorded change is the gross sum; the executor nets the fee out.
        assert_eq!(input_sum, batch.change_amount_sompi);
    }
    assert_all_batches_mass_valid(&evaluator, &batches);
}
