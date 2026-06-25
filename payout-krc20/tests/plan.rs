//! Deterministic, chain-free tests for the mass-aware commit/reveal planner.
//!
//! Headline guarantees: the reveal's transient mass accounts for the
//! redeem-script push; both transactions fit `max_block_mass`; the network
//! fees are sized at (or above) the mempool relay minimum; and a frozen
//! replay of an adaptive plan reproduces the exact same shape (the property
//! that keeps recorded txids stable across a crash-resume).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use kaspa_addresses::{Address, Prefix, Version};
use kaspa_consensus_core::tx::{ScriptPublicKey, TransactionId};
use kaspa_consensus_core::tx::{TransactionOutpoint, UtxoEntry};
use kaspa_txscript::pay_to_address_script;
use katpool_storagemass::{
    FeeRate, MIN_PAYOUT_OUTPUT_SOMPI, MassEvaluator, TreasuryUtxo, is_change_dust,
};
use payout_krc20::{
    CommitRevealConfig, DEFAULT_COMMIT_AMOUNT_SOMPI, Krc20FeePolicy, Krc20Transfer, PlanError,
    plan_commit_reveal,
};

const XONLY_PK: [u8; 32] = [
    0x1b, 0x91, 0x5b, 0x4c, 0x2a, 0x77, 0x0e, 0x3f, 0x44, 0x09, 0xd8, 0x60, 0xb1, 0x22, 0x6e, 0x90,
    0xc5, 0x33, 0xaa, 0x18, 0x7d, 0x6f, 0x4e, 0x21, 0x88, 0x99, 0x10, 0x55, 0xe7, 0x3c, 0xba, 0x02,
];

const RECIPIENT: &str = "kaspatest:qqkq3vz9j8m8k0r2c8x4n5p6w7s9t0u1v2x3y4z5a6b7c8d9e0f";

/// Relay-floor adaptive policy: feerate 0 ⇒ every fee is exactly the mempool
/// minimum relay fee for the transaction's compute mass. Deterministic, so it
/// doubles as the "minimum acceptable fee" oracle in assertions.
fn relay_floor() -> Krc20FeePolicy {
    Krc20FeePolicy::Adaptive(FeeRate::from_feerate(0.0))
}

const fn cfg(commit_amount_sompi: u64, fee_policy: Krc20FeePolicy) -> CommitRevealConfig {
    CommitRevealConfig {
        commit_amount_sompi,
        fee_policy,
    }
}

fn treasury_script() -> ScriptPublicKey {
    let addr = Address::new(Prefix::Testnet, Version::PubKey, &XONLY_PK);
    pay_to_address_script(&addr)
}

fn utxo(index: u32, amount: u64) -> TreasuryUtxo {
    TreasuryUtxo {
        outpoint: TransactionOutpoint {
            transaction_id: TransactionId::from_bytes([7u8; 32]),
            index,
        },
        entry: UtxoEntry {
            amount,
            script_public_key: treasury_script(),
            block_daa_score: 0,
            is_coinbase: false,
            covenant_id: None,
        },
    }
}

fn transfer() -> Krc20Transfer {
    Krc20Transfer::new("NACHO", "273972602739", RECIPIENT)
}

#[test]
fn plans_pair_and_both_fit_independently() {
    let eval = MassEvaluator::mainnet();
    let config = cfg(DEFAULT_COMMIT_AMOUNT_SOMPI, relay_floor());
    let utxos = vec![utxo(0, 5_000_000_000)]; // 50 KAS

    let plan = plan_commit_reveal(
        &eval,
        &utxos,
        &treasury_script(),
        &XONLY_PK,
        &transfer(),
        &config,
    )
    .unwrap();

    assert!(plan.commit_mass.fits_independently(eval.block_mass_limit()));
    assert!(plan.reveal_mass.fits_independently(eval.block_mass_limit()));
    assert_eq!(
        plan.reveal_return_sompi,
        DEFAULT_COMMIT_AMOUNT_SOMPI - plan.reveal_fee_sompi
    );
    assert!(!plan.commit_inputs.is_empty());
    assert!(!plan.redeem_script.is_empty());
}

#[test]
fn fees_meet_relay_minimum_and_exceed_legacy_fixed_fee() {
    // The legacy planner used a flat 10_000-sompi fee, ~18-20× below the
    // mempool minimum relay fee — the cause of RejectInsufficientFee. Under
    // the relay floor each fee must equal `compute_mass × 100` (the mirrored
    // `minimum_required_transaction_relay_fee`), and so far exceed 10_000.
    let eval = MassEvaluator::mainnet();
    let config = cfg(DEFAULT_COMMIT_AMOUNT_SOMPI, relay_floor());
    let utxos = vec![utxo(0, 5_000_000_000)];

    let plan = plan_commit_reveal(
        &eval,
        &utxos,
        &treasury_script(),
        &XONLY_PK,
        &transfer(),
        &config,
    )
    .unwrap();

    assert_eq!(plan.commit_fee_sompi, plan.commit_mass.compute_mass * 100);
    assert_eq!(plan.reveal_fee_sompi, plan.reveal_mass.compute_mass * 100);
    assert!(plan.commit_fee_sompi > 10_000);
    assert!(plan.reveal_fee_sompi > 10_000);
}

#[test]
fn frozen_replay_reproduces_adaptive_shape() {
    // Resume determinism: re-planning with the fees an adaptive plan resolved
    // must reproduce the identical inputs, change, fees and scripts — so the
    // commit/reveal txids recorded before broadcast survive a restart.
    let eval = MassEvaluator::mainnet();
    let utxos = vec![utxo(0, 5_000_000_000)];

    let adaptive = plan_commit_reveal(
        &eval,
        &utxos,
        &treasury_script(),
        &XONLY_PK,
        &transfer(),
        &cfg(
            DEFAULT_COMMIT_AMOUNT_SOMPI,
            Krc20FeePolicy::Adaptive(FeeRate::from_feerate(31.0)),
        ),
    )
    .unwrap();

    let frozen = plan_commit_reveal(
        &eval,
        &utxos,
        &treasury_script(),
        &XONLY_PK,
        &transfer(),
        &cfg(
            DEFAULT_COMMIT_AMOUNT_SOMPI,
            Krc20FeePolicy::Frozen {
                commit_fee_sompi: adaptive.commit_fee_sompi,
                reveal_fee_sompi: adaptive.reveal_fee_sompi,
            },
        ),
    )
    .unwrap();

    assert_eq!(frozen.commit_inputs, adaptive.commit_inputs);
    assert_eq!(frozen.commit_change_sompi, adaptive.commit_change_sompi);
    assert_eq!(frozen.commit_fee_sompi, adaptive.commit_fee_sompi);
    assert_eq!(frozen.reveal_fee_sompi, adaptive.reveal_fee_sompi);
    assert_eq!(frozen.reveal_return_sompi, adaptive.reveal_return_sompi);
    assert_eq!(frozen.redeem_script, adaptive.redeem_script);
    assert_eq!(
        frozen.commit_script_public_key,
        adaptive.commit_script_public_key
    );
}

#[test]
fn reveal_transient_mass_accounts_for_redeem_script() {
    let eval = MassEvaluator::mainnet();
    let config = cfg(DEFAULT_COMMIT_AMOUNT_SOMPI, relay_floor());
    let utxos = vec![utxo(0, 5_000_000_000)];

    let plan = plan_commit_reveal(
        &eval,
        &utxos,
        &treasury_script(),
        &XONLY_PK,
        &transfer(),
        &config,
    )
    .unwrap();

    // Transient mass = serialized_size × 4, and the serialized reveal
    // embeds the full redeem-script push in its signature script. So the
    // transient mass must exceed 4× the redeem-script length — proof that
    // KIP-13's redeem-script-and-data path is counted (docs/kips.md §5.2).
    let redeem_len = plan.redeem_script.len() as u64;
    assert!(
        plan.reveal_mass.transient_mass > redeem_len * 4,
        "reveal transient {} should exceed 4×redeem {}",
        plan.reveal_mass.transient_mass,
        redeem_len * 4
    );
}

#[test]
fn underfunded_treasury_is_rejected() {
    let eval = MassEvaluator::mainnet();
    let config = cfg(DEFAULT_COMMIT_AMOUNT_SOMPI, relay_floor());
    let utxos = vec![utxo(0, 1_000)]; // far below commit_amount + fee

    let err = plan_commit_reveal(
        &eval,
        &utxos,
        &treasury_script(),
        &XONLY_PK,
        &transfer(),
        &config,
    )
    .expect_err("should be underfunded");
    assert!(
        matches!(err, PlanError::InsufficientFunds { .. }),
        "got {err:?}"
    );
}

#[test]
fn reveal_return_below_floor_is_rejected() {
    let eval = MassEvaluator::mainnet();
    // commit_amount only just above the (frozen) reveal fee → return < floor.
    let config = cfg(
        MIN_PAYOUT_OUTPUT_SOMPI + 5,
        Krc20FeePolicy::Frozen {
            commit_fee_sompi: 10_000,
            reveal_fee_sompi: 10,
        },
    );
    let utxos = vec![utxo(0, 5_000_000_000)];

    let err = plan_commit_reveal(
        &eval,
        &utxos,
        &treasury_script(),
        &XONLY_PK,
        &transfer(),
        &config,
    )
    .expect_err("should reject sub-floor reveal");
    assert!(
        matches!(err, PlanError::RevealBelowFloor { .. }),
        "got {err:?}"
    );
}

#[test]
fn dust_change_folds_into_fee() {
    let eval = MassEvaluator::mainnet();
    // Frozen commit fee so the arithmetic is exact: leftover after the locked
    // amount and fee is a genuine consensus-dust value, which kaspad would
    // reject as an output, so the planner folds it into the fee.
    let commit_fee = 250_000_u64;
    let dust = 1_000_u64; // ≪ the ~55k dust threshold for a p2pk output
    assert!(is_change_dust(dust, &treasury_script()));
    let config = cfg(
        DEFAULT_COMMIT_AMOUNT_SOMPI,
        Krc20FeePolicy::Frozen {
            commit_fee_sompi: commit_fee,
            reveal_fee_sompi: 200_000,
        },
    );
    let utxos = vec![utxo(0, DEFAULT_COMMIT_AMOUNT_SOMPI + commit_fee + dust)];

    let plan = plan_commit_reveal(
        &eval,
        &utxos,
        &treasury_script(),
        &XONLY_PK,
        &transfer(),
        &config,
    )
    .unwrap();
    assert_eq!(
        plan.commit_change_sompi, 0,
        "dust change must be folded into the fee"
    );
    // The folded leftover is absorbed: effective fee = inputs − locked amount.
    assert_eq!(plan.commit_fee_sompi, commit_fee + dust);
}

#[test]
fn healthy_change_is_returned() {
    let eval = MassEvaluator::mainnet();
    let commit_fee = 250_000_u64;
    let config = cfg(
        DEFAULT_COMMIT_AMOUNT_SOMPI,
        Krc20FeePolicy::Frozen {
            commit_fee_sompi: commit_fee,
            reveal_fee_sompi: 200_000,
        },
    );
    // Leftover large enough to clear both the dust floor and KIP-9 storage
    // mass (small outputs from a large input are penalised — anti-dust).
    let leftover = 50_000_000; // 0.5 KAS
    let utxos = vec![utxo(0, DEFAULT_COMMIT_AMOUNT_SOMPI + commit_fee + leftover)];

    let plan = plan_commit_reveal(
        &eval,
        &utxos,
        &treasury_script(),
        &XONLY_PK,
        &transfer(),
        &config,
    )
    .unwrap();
    assert_eq!(plan.commit_change_sompi, leftover);
    assert_eq!(plan.commit_fee_sompi, commit_fee);
    assert!(plan.commit_mass.fits_independently(eval.block_mass_limit()));
}

#[test]
fn selects_multiple_inputs_when_needed() {
    let eval = MassEvaluator::mainnet();
    let commit_fee = 250_000_u64;
    let config = cfg(
        DEFAULT_COMMIT_AMOUNT_SOMPI,
        Krc20FeePolicy::Frozen {
            commit_fee_sompi: commit_fee,
            reveal_fee_sompi: 200_000,
        },
    );
    let needed = DEFAULT_COMMIT_AMOUNT_SOMPI + commit_fee;
    // Two UTXOs, each individually below `needed` so both must be consumed,
    // but together comfortably covering it plus a healthy change.
    let part = needed - MIN_PAYOUT_OUTPUT_SOMPI;
    let utxos = vec![utxo(0, part), utxo(1, part)];

    let plan = plan_commit_reveal(
        &eval,
        &utxos,
        &treasury_script(),
        &XONLY_PK,
        &transfer(),
        &config,
    )
    .unwrap();
    assert_eq!(
        plan.commit_inputs.len(),
        2,
        "should consume both UTXOs to cover the commit"
    );
}
