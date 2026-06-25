//! Parity tests: `katpool_storagemass` matches `kaspa_consensus_core::mass`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::cast_possible_truncation
)]

use std::str::FromStr;

use kaspa_consensus_core::{
    mass::MassCalculator,
    subnets::SubnetworkId,
    tx::{
        PopulatedTransaction, ScriptPublicKey, ScriptVec, Transaction, TransactionId,
        TransactionInput, TransactionOutpoint, TransactionOutput, UtxoEntry,
    },
};
use katpool_storagemass::{MAINNET_MAX_BLOCK_MASS, MassEvaluator};

fn sample_tx_from_amounts(ins: &[u64], outs: &[u64]) -> (Transaction, Vec<UtxoEntry>) {
    let script_pub_key = ScriptVec::from_slice(&[]);
    let prev_tx_id =
        TransactionId::from_str("880eb9819a31821d9d2399e2f35e2433b72637e393d71ecc9b8d0250f49153c3")
            .unwrap();
    let tx = Transaction::new(
        0,
        (0..ins.len())
            .map(|i| {
                TransactionInput::new(
                    TransactionOutpoint {
                        transaction_id: prev_tx_id,
                        index: i as u32,
                    },
                    vec![],
                    0,
                    0,
                )
            })
            .collect(),
        outs.iter()
            .copied()
            .map(|value| {
                TransactionOutput::new(value, ScriptPublicKey::new(0, script_pub_key.clone()))
            })
            .collect(),
        1_615_462_089_000,
        SubnetworkId::from_bytes([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
        0,
        vec![],
    );
    let entries: Vec<UtxoEntry> = ins
        .iter()
        .copied()
        .map(|amount| UtxoEntry {
            amount,
            script_public_key: ScriptPublicKey::new(0, script_pub_key.clone()),
            block_daa_score: 0,
            is_coinbase: false,
            covenant_id: None,
        })
        .collect();
    (tx, entries)
}

#[test]
fn mainnet_block_mass_matches_consensus_params() {
    assert_eq!(MAINNET_MAX_BLOCK_MASS, 500_000);
}

#[test]
fn evaluator_matches_mass_calculator_directly() {
    let evaluator = MassEvaluator::mainnet();
    let calculator = MassCalculator::new_with_consensus_params(
        &kaspa_consensus_core::config::params::MAINNET_PARAMS,
    );

    let cases: &[(&[u64], &[u64])] = &[
        (
            &[100_000_000_000, 200_000_000_000],
            &[300_000_000_000, 300_000_000_000],
        ),
        (
            &[350_000_000_000, 500_000_000_000],
            &[200_000_000_000, 200_000_000_000, 400_000_000_000],
        ),
        (
            &[10_000_000_000, 10_000_000_000],
            &[5_000_000_000, 15_000_000_000],
        ),
    ];

    for (ins, outs) in cases {
        let (tx, entries) = sample_tx_from_amounts(ins, outs);
        let populated = PopulatedTransaction::new(&tx, entries);
        let got = evaluator.evaluate_populated(&populated).unwrap();
        let non = calculator.calc_non_contextual_masses(populated.tx);
        let ctx = calculator.calc_contextual_masses(&populated).unwrap();
        assert_eq!(got.compute_mass, non.compute_mass);
        assert_eq!(got.transient_mass, non.transient_mass);
        assert_eq!(got.storage_mass, ctx.storage_mass);
    }
}

#[test]
fn symmetric_two_by_two_has_zero_storage_mass() {
    let evaluator = MassEvaluator::mainnet();
    let (tx, entries) = sample_tx_from_amounts(&[100, 200], &[100, 200]);
    let populated = PopulatedTransaction::new(&tx, entries);
    let mass = evaluator.evaluate_populated(&populated).unwrap();
    assert_eq!(mass.storage_mass, 0);
    assert!(mass.fits_independently(MAINNET_MAX_BLOCK_MASS));
}
