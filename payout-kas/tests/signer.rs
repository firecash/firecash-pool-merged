//! Deterministic signing tests (M4.6) — no network, no database.
//!
//! Builds a real mass-valid batch via `plan_batches`, signs it with a known
//! treasury key, and verifies every input through the txscript engine — the
//! same check kaspad performs.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::str::FromStr;

use kaspa_addresses::{Address, Prefix, Version};
use kaspa_consensus_core::tx::{ScriptPublicKey, TransactionId, TransactionOutpoint, UtxoEntry};
use kaspa_txscript::pay_to_address_script;
use katpool_secrets::{TreasurySecret, from_hex};
use katpool_storagemass::{
    MassEvaluator, PLANNING_VIRTUAL_TXID_BYTES, PayoutRecipient, PlannedBatch, TreasuryUtxo,
    plan_batches,
};
use payout_kas::{SignError, batch_txid, sign_batch, verify_signed};
use secp256k1::Keypair;

const TREASURY_HEX: &str = "1111111111111111111111111111111111111111111111111111111111111111";
const OTHER_HEX: &str = "2222222222222222222222222222222222222222222222222222222222222222";

fn keypair(hex: &str) -> Keypair {
    let secret = from_hex(hex).expect("valid key");
    Keypair::from_seckey_slice(secp256k1::SECP256K1, secret.expose_secret()).expect("keypair")
}

fn script_for(hex: &str) -> ScriptPublicKey {
    let xonly = keypair(hex).x_only_public_key().0.serialize();
    let addr = Address::new(Prefix::Testnet, Version::PubKey, &xonly);
    pay_to_address_script(&addr)
}

fn treasury_secret() -> TreasurySecret {
    from_hex(TREASURY_HEX).expect("valid treasury key")
}

fn funding_utxo(amount: u64, script: &ScriptPublicKey) -> TreasuryUtxo {
    let tx_id =
        TransactionId::from_str("880eb9819a31821d9d2399e2f35e2433b72637e393d71ecc9b8d0250f49153c3")
            .unwrap();
    TreasuryUtxo {
        outpoint: TransactionOutpoint {
            transaction_id: tx_id,
            index: 0,
        },
        entry: UtxoEntry {
            amount,
            script_public_key: script.clone(),
            block_daa_score: 100,
            is_coinbase: false,
            covenant_id: None,
        },
    }
}

/// Build a real, mass-valid single batch: one treasury input funding two
/// recipients, with change back to the treasury.
fn sample_batch() -> (PlannedBatch, ScriptPublicKey) {
    let treasury_script = script_for(TREASURY_HEX);
    let r1 = script_for(OTHER_HEX);
    let r2 = script_for("3333333333333333333333333333333333333333333333333333333333333333");

    let recipients = vec![
        PayoutRecipient {
            id: "miner-1".to_owned(),
            amount_sompi: 1_000_000_000,
            script_public_key: r1,
        },
        PayoutRecipient {
            id: "miner-2".to_owned(),
            amount_sompi: 2_000_000_000,
            script_public_key: r2,
        },
    ];
    let evaluator = MassEvaluator::mainnet();
    let result = plan_batches(
        &evaluator,
        vec![funding_utxo(5_000_000_000, &treasury_script)],
        recipients,
        &treasury_script,
        &katpool_storagemass::FeeRate::ZERO,
    );
    assert!(result.unpaid.is_empty(), "everything fits");
    assert!(result.deferred_below_floor.is_empty());
    assert_eq!(result.batches.len(), 1, "single batch expected");
    (result.batches.into_iter().next().unwrap(), treasury_script)
}

#[test]
fn signs_and_verifies_every_input() {
    let (batch, treasury_script) = sample_batch();
    let secret = treasury_secret();

    let signed = sign_batch(&batch, &treasury_script, &secret).expect("sign");
    // sign_batch verifies internally; assert it again explicitly.
    verify_signed(&signed).expect("verify");

    assert!(
        signed
            .tx
            .inputs
            .iter()
            .all(|i| !i.signature_script.is_empty()),
        "every input carries a signature"
    );
    // Two payouts + change output.
    assert_eq!(signed.tx.outputs.len(), 3);
}

#[test]
fn txid_is_deterministic_and_matches_signed_tx() {
    let (batch, treasury_script) = sample_batch();
    let secret = treasury_secret();

    let id_a = batch_txid(&batch, &treasury_script).expect("txid a");
    let id_b = batch_txid(&batch, &treasury_script).expect("txid b");
    assert_eq!(id_a, id_b, "pre-sign txid is stable");

    let signed = sign_batch(&batch, &treasury_script, &secret).expect("sign");
    assert_eq!(signed.txid(), id_a, "signing does not change the txid");
}

#[test]
fn rejects_planning_virtual_inputs() {
    let (mut batch, treasury_script) = sample_batch();
    batch.inputs[0].outpoint.transaction_id =
        TransactionId::from_bytes(PLANNING_VIRTUAL_TXID_BYTES);
    let secret = treasury_secret();

    let err = sign_batch(&batch, &treasury_script, &secret).expect_err("virtual input rejected");
    assert!(matches!(err, SignError::VirtualInput));
    assert!(matches!(
        batch_txid(&batch, &treasury_script).expect_err("txid rejects too"),
        SignError::VirtualInput
    ));
}

#[test]
fn wrong_key_fails_verification() {
    let (batch, treasury_script) = sample_batch();
    // Sign with a key whose pubkey does not match the funding script.
    let wrong = from_hex(OTHER_HEX).expect("valid key");

    let err = sign_batch(&batch, &treasury_script, &wrong)
        .expect_err("mismatched key must fail script verification");
    assert!(matches!(err, SignError::Verify { .. }));
}

#[test]
fn tampered_output_fails_verification() {
    let (batch, treasury_script) = sample_batch();
    let secret = treasury_secret();
    let mut signed = sign_batch(&batch, &treasury_script, &secret).expect("sign");

    // Mutating an output invalidates the sighash for every input.
    signed.tx.outputs[0].value += 1;
    let err = verify_signed(&signed).expect_err("tamper must be detected");
    assert!(matches!(err, SignError::Verify { .. }));
}
