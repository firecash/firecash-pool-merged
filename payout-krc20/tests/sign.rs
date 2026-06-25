//! Deterministic, chain-free tests for the KRC-20 commit/reveal signer.
//!
//! The headline guarantee: the reveal's manual P2SH-redeem-path signature
//! verifies through the *same* txscript engine kaspad runs — proving the
//! commit/reveal pair would be accepted on chain — and the deterministic
//! txids are stable for record-before-broadcast.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use kaspa_addresses::{Address, Prefix, Version};
use kaspa_consensus_core::tx::{ScriptPublicKey, TransactionId, TransactionOutpoint, UtxoEntry};
use kaspa_txscript::pay_to_address_script;
use katpool_secrets::{TreasurySecret, from_hex};
use katpool_storagemass::{FeeRate, MassEvaluator, PLANNING_VIRTUAL_TXID_BYTES, TreasuryUtxo};
use payout_krc20::{
    CommitRevealConfig, DEFAULT_COMMIT_AMOUNT_SOMPI, Krc20FeePolicy, Krc20Transfer,
    PlannedCommitReveal, SignError, commit_txid, plan_commit_reveal, reveal_txid, sign_commit,
    sign_reveal,
};
use secp256k1::Keypair;

const RECIPIENT: &str = "kaspatest:qqkq3vz9j8m8k0r2c8x4n5p6w7s9t0u1v2x3y4z5a6b7c8d9e0f";

/// A deterministic, valid (non-zero) treasury secret.
const TREASURY_HEX: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
/// A different valid secret, for the key-mismatch test.
const OTHER_HEX: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";

fn secret(hex: &str) -> TreasurySecret {
    from_hex(hex).expect("valid key hex")
}

fn xonly_of(secret: &TreasurySecret) -> [u8; 32] {
    let kp = Keypair::from_seckey_slice(secp256k1::SECP256K1, secret.expose_secret()).unwrap();
    kp.x_only_public_key().0.serialize()
}

fn treasury_script(xonly: &[u8; 32]) -> ScriptPublicKey {
    pay_to_address_script(&Address::new(Prefix::Testnet, Version::PubKey, xonly))
}

fn utxo(script: &ScriptPublicKey, index: u32, amount: u64) -> TreasuryUtxo {
    TreasuryUtxo {
        outpoint: TransactionOutpoint {
            transaction_id: TransactionId::from_bytes([7u8; 32]),
            index,
        },
        entry: UtxoEntry {
            amount,
            script_public_key: script.clone(),
            block_daa_score: 0,
            is_coinbase: false,
            covenant_id: None,
        },
    }
}

fn transfer() -> Krc20Transfer {
    Krc20Transfer::new("NACHO", "273972602739", RECIPIENT)
}

/// Build a real plan funded by `treasury_script`, ready to sign.
fn build_plan(xonly: &[u8; 32]) -> (PlannedCommitReveal, ScriptPublicKey) {
    let eval = MassEvaluator::mainnet();
    let cfg = CommitRevealConfig {
        commit_amount_sompi: DEFAULT_COMMIT_AMOUNT_SOMPI,
        fee_policy: Krc20FeePolicy::Adaptive(FeeRate::from_feerate(0.0)),
    };
    let script = treasury_script(xonly);
    let utxos = vec![utxo(&script, 0, 5_000_000_000)]; // 50 KAS
    let plan = plan_commit_reveal(&eval, &utxos, &script, xonly, &transfer(), &cfg).unwrap();
    (plan, script)
}

#[test]
fn commit_signs_and_verifies_through_the_engine() {
    let secret = secret(TREASURY_HEX);
    let xonly = xonly_of(&secret);
    let (plan, script) = build_plan(&xonly);

    let signed = sign_commit(&plan, &script, &secret).expect("commit signs + verifies");
    // Output 0 is the P2SH commit output.
    assert_eq!(
        signed.tx.outputs[0].script_public_key,
        plan.commit_script_public_key
    );
    assert_eq!(signed.tx.outputs[0].value, plan.commit_amount_sompi);
    // Deterministic txid is stable pre/post-sign (sig scripts excluded).
    assert_eq!(commit_txid(&plan, &script).unwrap(), signed.txid());
}

#[test]
fn reveal_redeem_path_signs_and_verifies_through_the_engine() {
    let secret = secret(TREASURY_HEX);
    let xonly = xonly_of(&secret);
    let (plan, script) = build_plan(&xonly);

    let commit = sign_commit(&plan, &script, &secret).unwrap();
    let outpoint = commit.commit_outpoint();

    // The crux: a manual P2SH-redeem-path signature that the engine accepts.
    let reveal = sign_reveal(&plan, outpoint, &script, &secret).expect("reveal signs + verifies");

    // One input (the commit P2SH output), one output (treasury return).
    assert_eq!(reveal.tx.inputs.len(), 1);
    assert_eq!(reveal.tx.outputs.len(), 1);
    assert_eq!(reveal.tx.outputs[0].value, plan.reveal_return_sompi);
    assert_eq!(reveal.tx.outputs[0].script_public_key, script);
    assert_eq!(reveal.tx.inputs[0].previous_outpoint, outpoint);

    // Signature script is `<OP_DATA_65 sig> <pushed redeem script>`.
    let sig_script = &reveal.tx.inputs[0].signature_script;
    assert_eq!(sig_script[0], 65u8, "leads with OP_DATA_65 sig push");
    assert!(
        sig_script.ends_with(&plan.redeem_script),
        "ends with the pushed redeem script"
    );

    // Deterministic txid stable pre/post-sign.
    assert_eq!(reveal_txid(&plan, outpoint, &script), reveal.txid());
}

#[test]
fn reveal_with_wrong_key_is_rejected_by_verification() {
    // Inscription bound to the treasury key; reveal signed with a different
    // key → OP_CHECKSIG fails in the engine → hard error, never broadcast.
    let treasury = secret(TREASURY_HEX);
    let xonly = xonly_of(&treasury);
    let (plan, script) = build_plan(&xonly);
    let commit = sign_commit(&plan, &script, &treasury).unwrap();
    let outpoint = commit.commit_outpoint();

    let wrong = secret(OTHER_HEX);
    let err = sign_reveal(&plan, outpoint, &script, &wrong)
        .expect_err("mismatched key must fail verification");
    assert!(
        matches!(err, SignError::Verify { input: 0, .. }),
        "got {err:?}"
    );
}

#[test]
fn empty_commit_inputs_are_rejected() {
    let secret = secret(TREASURY_HEX);
    let xonly = xonly_of(&secret);
    let (mut plan, script) = build_plan(&xonly);
    plan.commit_inputs.clear();

    let err = sign_commit(&plan, &script, &secret).expect_err("no inputs");
    assert!(matches!(err, SignError::EmptyInputs), "got {err:?}");
    assert!(matches!(
        commit_txid(&plan, &script),
        Err(SignError::EmptyInputs)
    ));
}

#[test]
fn planning_virtual_input_is_rejected() {
    let secret = secret(TREASURY_HEX);
    let xonly = xonly_of(&secret);
    let (mut plan, script) = build_plan(&xonly);
    // Swap in a planning-only virtual coin — must never be signed/broadcast.
    plan.commit_inputs = vec![TreasuryUtxo {
        outpoint: TransactionOutpoint {
            transaction_id: TransactionId::from_bytes(PLANNING_VIRTUAL_TXID_BYTES),
            index: 0,
        },
        entry: UtxoEntry {
            amount: 5_000_000_000,
            script_public_key: script.clone(),
            block_daa_score: 0,
            is_coinbase: false,
            covenant_id: None,
        },
    }];

    let err = sign_commit(&plan, &script, &secret).expect_err("virtual input");
    assert!(matches!(err, SignError::VirtualInput), "got {err:?}");
}
