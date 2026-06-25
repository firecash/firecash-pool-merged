//! Transaction assembly + Schnorr signing for KAS payout batches.
//!
//! Bridges a mass-validated [`PlannedBatch`] (from `katpool-storagemass`) to a
//! consensus-native, signed [`Transaction`] ready for kaspad submission.
//!
//! Two invariants make the executor crash-safe and never double-pay:
//!
//! - **Deterministic txid.** Kaspa's transaction id hashes everything *except*
//!   the signature scripts, so [`batch_txid`] computes the on-chain id from the
//!   unsigned structure. The executor records that id on the payout rows
//!   *before* it signs/submits; a restart rebuilds the identical id.
//! - **Verify before submit.** [`sign_batch`] runs the full txscript engine on
//!   every input, so a malformed signature can never leave this process.

use kaspa_consensus_core::constants::TX_VERSION;
use kaspa_consensus_core::hashing::sighash::SigHashReusedValuesUnsync;
use kaspa_consensus_core::sign::sign;
use kaspa_consensus_core::subnets::SUBNETWORK_ID_NATIVE;
use kaspa_consensus_core::tx::{
    PopulatedTransaction, ScriptPublicKey, SignableTransaction, Transaction, TransactionId,
    TransactionInput, TransactionOutput, UtxoEntry, VerifiableTransaction,
};
use kaspa_txscript::caches::Cache;
use kaspa_txscript::engine_context::EngineContext;
use kaspa_txscript::{EngineFlags, TxScriptEngine};
use katpool_secrets::TreasurySecret;
use katpool_storagemass::PlannedBatch;
use secp256k1::Keypair;

/// Errors from assembling, signing, or verifying a payout transaction.
#[derive(Debug, thiserror::Error)]
pub enum SignError {
    /// A planning-only virtual UTXO reached the signer. The executor must
    /// refresh live treasury UTXOs and re-plan before signing.
    #[error("planning-virtual UTXO cannot be signed; refresh live treasury UTXOs first")]
    VirtualInput,

    /// The batch carried no inputs — nothing to sign.
    #[error("batch has no inputs")]
    EmptyInputs,

    /// The treasury secret is not a valid secp256k1 key.
    #[error("invalid treasury key")]
    Key(#[from] secp256k1::Error),

    /// Post-sign script verification failed for an input.
    #[error("signature verification failed for input {input}: {reason}")]
    Verify {
        /// Index of the input that failed verification.
        input: usize,
        /// Human-readable script-engine error.
        reason: String,
    },
}

/// A fully signed, finalized payout transaction and the UTXO entries that
/// fund it (kept for verification and for the RPC submission path).
#[derive(Debug, Clone)]
pub struct SignedBatch {
    /// The signed, finalized consensus transaction.
    pub tx: Transaction,
    /// The funding UTXO entries, one per input (input order).
    pub entries: Vec<UtxoEntry>,
}

impl SignedBatch {
    /// On-chain transaction id (stable across signing — sig scripts excluded).
    #[must_use]
    pub fn txid(&self) -> TransactionId {
        self.tx.id()
    }
}

/// Build the unsigned transaction + funding entries for a batch.
///
/// Outputs are the batch payouts in order, followed by a single change output
/// to `treasury_script` when `change_amount_sompi > 0`. Uses the native
/// subnetwork and current `TX_VERSION` (unlike the mass-test builder, which is
/// shape-only).
fn build_unsigned(
    batch: &PlannedBatch,
    treasury_script: &ScriptPublicKey,
) -> Result<(Transaction, Vec<UtxoEntry>), SignError> {
    if batch.inputs.is_empty() {
        return Err(SignError::EmptyInputs);
    }
    if batch
        .inputs
        .iter()
        .any(katpool_storagemass::TreasuryUtxo::is_planning_virtual)
    {
        return Err(SignError::VirtualInput);
    }

    let inputs: Vec<TransactionInput> = batch
        .inputs
        .iter()
        .map(|u| TransactionInput::new(u.outpoint, vec![], 0, 1))
        .collect();

    let mut outputs: Vec<TransactionOutput> = batch
        .payouts
        .iter()
        .map(|p| TransactionOutput::new(p.amount_sompi, p.script_public_key.clone()))
        .collect();

    if batch.change_amount_sompi > 0 {
        outputs.push(TransactionOutput::new(
            batch.change_amount_sompi,
            treasury_script.clone(),
        ));
    }

    let entries: Vec<UtxoEntry> = batch.inputs.iter().map(|u| u.entry.clone()).collect();
    let tx = Transaction::new_non_finalized(
        TX_VERSION,
        inputs,
        outputs,
        0,
        SUBNETWORK_ID_NATIVE,
        0,
        vec![],
    );
    Ok((tx, entries))
}

/// Compute the on-chain txid for a batch without signing it.
///
/// Used by the executor to record the payout rows' `tx_hash` *before* it
/// signs and broadcasts, so a crash mid-broadcast is recoverable.
pub fn batch_txid(
    batch: &PlannedBatch,
    treasury_script: &ScriptPublicKey,
) -> Result<TransactionId, SignError> {
    let (mut tx, _entries) = build_unsigned(batch, treasury_script)?;
    tx.finalize();
    Ok(tx.id())
}

/// Assemble, sign, finalize, and fully verify a payout batch.
///
/// Every input is signed with the treasury key (Schnorr, `SIG_HASH_ALL`) and
/// then re-executed through the txscript engine; an invalid signature is a
/// hard error, never a submitted transaction.
pub fn sign_batch(
    batch: &PlannedBatch,
    treasury_script: &ScriptPublicKey,
    secret: &TreasurySecret,
) -> Result<SignedBatch, SignError> {
    let (unsigned, entries) = build_unsigned(batch, treasury_script)?;
    let keypair = Keypair::from_seckey_slice(secp256k1::SECP256K1, secret.expose_secret())?;

    let signable = SignableTransaction::with_entries(unsigned, entries.clone());
    let signed = sign(signable, keypair);
    let mut tx = signed.tx;
    tx.finalize();

    let signed = SignedBatch { tx, entries };
    verify_signed(&signed)?;
    Ok(signed)
}

/// Run the full txscript engine over every input of a signed batch.
///
/// This is the same verification kaspad performs; running it locally guarantees
/// we never broadcast a transaction that would be rejected for a bad signature.
pub fn verify_signed(signed: &SignedBatch) -> Result<(), SignError> {
    let verifiable = PopulatedTransaction::new(&signed.tx, signed.entries.clone());
    let reused = SigHashReusedValuesUnsync::new();
    let sig_cache: Cache<_, bool> = Cache::new(signed.entries.len() as u64);
    // Mirror the post-Toccata consensus engine (kaspa v2.0.0): covenants are
    // active on every network we target (post-Toccata tn10 / mainnet), so the
    // self-check validates under the exact rules kaspad applies. `EngineFlags`
    // `Default` sets `sigop_script_units: Gram(1000)`, which equals the
    // consensus `mass_per_sig_op` (1000) on all networks — see
    // `tx_validation_in_utxo_context::check_scripts`. Standard P2PK spends carry
    // no covenant binding, hence the default (empty) covenant context from
    // `EngineContext::new` is sufficient.
    let ctx = EngineContext::new(&sig_cache).with_reused(&reused);
    let flags = EngineFlags {
        covenants_enabled: true,
        ..Default::default()
    };
    for (idx, (input, entry)) in verifiable.populated_inputs().enumerate() {
        TxScriptEngine::from_transaction_input(&verifiable, input, idx, entry, ctx, flags)
            .execute()
            .map_err(|e| SignError::Verify {
                input: idx,
                reason: e.to_string(),
            })?;
    }
    Ok(())
}
