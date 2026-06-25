//! Build unsigned `Transaction` + populated entries for mass evaluation.

use kaspa_consensus_core::{
    subnets::SubnetworkId,
    tx::{ScriptPublicKey, Transaction, TransactionInput, TransactionOutput, UtxoEntry},
};

use crate::types::{PayoutRecipient, TreasuryUtxo};

/// Standard subnetwork id used by the consensus mass tests (non-coinbase).
const fn standard_subnetwork() -> SubnetworkId {
    SubnetworkId::from_bytes([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])
}

/// Serialized length of a signed Schnorr P2PK `signature_script`, from
/// rusty-kaspa `consensus/core/src/sign.rs`: `OP_DATA_65` (1 byte) + 64-byte
/// signature + 1 sighash-type byte. The planner must charge this per input so
/// the estimated compute/transient mass matches the *signed* transaction kaspad
/// actually validates — otherwise the reserved fee underprices the real mass
/// and the broadcast is rejected for insufficient fee.
pub const SIGNED_P2PK_SIG_SCRIPT_LEN: usize = 66;

/// Construct a populated transaction for mass checking.
///
/// Inputs carry a placeholder `signature_script` of `SIGNED_P2PK_SIG_SCRIPT_LEN`
/// bytes so compute and transient mass match the signed transaction; the bytes
/// are never broadcast (the executor re-signs from the unsigned structure).
#[must_use]
pub fn build_populated(
    inputs: &[TreasuryUtxo],
    payouts: &[&PayoutRecipient],
    change_script: &ScriptPublicKey,
    change_amount_sompi: u64,
) -> (Transaction, Vec<UtxoEntry>) {
    let tx_inputs: Vec<TransactionInput> = inputs
        .iter()
        .map(|u| {
            // sig_op_count = 1 mirrors the signer's per-input mass commitment
            // (one OP_CHECKSIG per P2PK spend); v0 compute mass charges
            // `sig_op_count * GRAMS_PER_SIGOP_COUNT_UNIT`, so a count of 0 would
            // under-price the signed transaction's mass.
            TransactionInput::new(u.outpoint, vec![0u8; SIGNED_P2PK_SIG_SCRIPT_LEN], 0, 1)
        })
        .collect();

    let mut outputs: Vec<TransactionOutput> = payouts
        .iter()
        .map(|p| TransactionOutput::new(p.amount_sompi, p.script_public_key.clone()))
        .collect();

    if change_amount_sompi > 0 {
        outputs.push(TransactionOutput::new(
            change_amount_sompi,
            change_script.clone(),
        ));
    }

    let tx = Transaction::new(0, tx_inputs, outputs, 0, standard_subnetwork(), 0, vec![]);

    let entries: Vec<UtxoEntry> = inputs.iter().map(|u| u.entry.clone()).collect();
    (tx, entries)
}
