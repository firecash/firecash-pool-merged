//! Transaction assembly + Schnorr signing for the KRC-20 commit/reveal pair.
//!
//! Bridges a mass-validated [`PlannedCommitReveal`] (from [`crate::plan`]) to
//! the two consensus-native, signed transactions kaspad submits:
//!
//! - the **commit** spends standard treasury P2PK inputs and is signed with
//!   the same [`kaspa_consensus_core::sign::sign`] path as the KAS payout
//!   engine (`payout-kas`);
//! - the **reveal** spends the commit's P2SH output, so it is signed
//!   *manually*: the Schnorr signature is computed over the spent output's
//!   script public key — exactly as the script engine recomputes it in
//!   `OP_CHECKSIG` (`calc_schnorr_signature_hash` hashes `entry.script_public_key`)
//!   — then wrapped as `<sig> <pushed redeem script>` via
//!   [`kaspa_txscript::pay_to_script_hash_signature_script`].
//!
//! Two invariants make the executor crash-safe and never double-pay, mirroring
//! `payout-kas::signer`:
//!
//! - **Deterministic txid.** Kaspa's txid hashes everything *except* the
//!   signature scripts, so [`commit_txid`] / [`reveal_txid`] compute the
//!   on-chain id from the unsigned structure. The executor records that id
//!   *before* it signs/submits; a restart with the same inputs rebuilds the
//!   identical id.
//! - **Verify before submit.** Both signers re-run the full txscript engine
//!   over every input, so a malformed signature — or a treasury key that does
//!   not match the inscription's bound pubkey — is a hard error here, never a
//!   submitted transaction.

use kaspa_consensus_core::constants::TX_VERSION;
use kaspa_consensus_core::hashing::sighash::{
    SigHashReusedValuesUnsync, calc_schnorr_signature_hash,
};
use kaspa_consensus_core::hashing::sighash_type::SIG_HASH_ALL;
use kaspa_consensus_core::sign::sign;
use kaspa_consensus_core::subnets::SUBNETWORK_ID_NATIVE;
use kaspa_consensus_core::tx::{
    PopulatedTransaction, ScriptPublicKey, SignableTransaction, Transaction, TransactionId,
    TransactionInput, TransactionOutpoint, TransactionOutput, UtxoEntry, VerifiableTransaction,
};
use kaspa_txscript::caches::Cache;
use kaspa_txscript::engine_context::EngineContext;
use kaspa_txscript::script_builder::ScriptBuilderError;
use kaspa_txscript::{EngineFlags, TxScriptEngine, pay_to_script_hash_signature_script};
use katpool_secrets::TreasurySecret;
use secp256k1::{Keypair, Message};

use crate::plan::PlannedCommitReveal;

/// Output index of the P2SH commit output within the commit transaction.
/// The planner always emits the commit output first, change (if any) second.
pub const COMMIT_P2SH_OUTPUT_INDEX: u32 = 0;

/// Output index of the treasury change output within the commit transaction.
///
/// Present only when `commit_change_sompi` is non-zero (the commit output is
/// always first, change second). The settle sweep re-injects this output as a
/// spendable coin so sibling transfers chain off it instead of colliding on
/// the same treasury UTXO.
pub const COMMIT_CHANGE_OUTPUT_INDEX: u32 = 1;

/// Errors from assembling, signing, or verifying a commit/reveal pair.
#[derive(Debug, thiserror::Error)]
pub enum SignError {
    /// The commit carried no inputs — nothing to sign.
    #[error("commit has no inputs")]
    EmptyInputs,

    /// A planning-only virtual UTXO reached the signer. The executor must
    /// refresh live treasury UTXOs and re-plan before signing.
    #[error("planning-virtual UTXO cannot be signed; refresh live treasury UTXOs first")]
    VirtualInput,

    /// The treasury secret is not a valid secp256k1 key.
    #[error("invalid treasury key")]
    Key(#[from] secp256k1::Error),

    /// The 32-byte sighash could not be wrapped as a secp256k1 message
    /// (unreachable: the digest is always 32 bytes).
    #[error("sighash is not a valid secp256k1 message")]
    Message,

    /// The reveal signature script could not be built (redeem too large).
    #[error("reveal signature script: {0}")]
    Script(#[from] ScriptBuilderError),

    /// Post-sign script verification failed for an input. For the reveal this
    /// also fires when the treasury key does not match the pubkey bound into
    /// the inscription redeem script.
    #[error("signature verification failed for input {input}: {reason}")]
    Verify {
        /// Index of the input that failed verification.
        input: usize,
        /// Human-readable script-engine error.
        reason: String,
    },
}

/// A fully signed commit transaction and its funding entries.
#[derive(Debug, Clone)]
pub struct SignedCommit {
    /// The signed, finalized commit transaction.
    pub tx: Transaction,
    /// The treasury funding UTXO entries, one per input (input order).
    pub entries: Vec<UtxoEntry>,
}

impl SignedCommit {
    /// On-chain transaction id (stable across signing — sig scripts excluded).
    #[must_use]
    pub fn txid(&self) -> TransactionId {
        self.tx.id()
    }

    /// The outpoint of the P2SH commit output the reveal must spend.
    #[must_use]
    pub fn commit_outpoint(&self) -> TransactionOutpoint {
        TransactionOutpoint {
            transaction_id: self.tx.id(),
            index: COMMIT_P2SH_OUTPUT_INDEX,
        }
    }
}

/// A fully signed reveal transaction and the single P2SH entry it spends.
#[derive(Debug, Clone)]
pub struct SignedReveal {
    /// The signed, finalized reveal transaction.
    pub tx: Transaction,
    /// The commit P2SH output this reveal consumes.
    pub entry: UtxoEntry,
}

impl SignedReveal {
    /// On-chain transaction id (stable across signing — sig scripts excluded).
    #[must_use]
    pub fn txid(&self) -> TransactionId {
        self.tx.id()
    }
}

/// Build the unsigned commit transaction + funding entries.
///
/// Output 0 is the P2SH commit output; output 1 (when present) is treasury
/// change. Uses the native subnetwork and current `TX_VERSION`.
fn build_commit_unsigned(
    plan: &PlannedCommitReveal,
    treasury_script: &ScriptPublicKey,
) -> Result<(Transaction, Vec<UtxoEntry>), SignError> {
    if plan.commit_inputs.is_empty() {
        return Err(SignError::EmptyInputs);
    }
    if plan
        .commit_inputs
        .iter()
        .any(katpool_storagemass::TreasuryUtxo::is_planning_virtual)
    {
        return Err(SignError::VirtualInput);
    }

    let inputs: Vec<TransactionInput> = plan
        .commit_inputs
        .iter()
        .map(|u| TransactionInput::new(u.outpoint, vec![], 0, 1))
        .collect();

    let mut outputs = vec![TransactionOutput::new(
        plan.commit_amount_sompi,
        plan.commit_script_public_key.clone(),
    )];
    if plan.commit_change_sompi > 0 {
        outputs.push(TransactionOutput::new(
            plan.commit_change_sompi,
            treasury_script.clone(),
        ));
    }

    let entries: Vec<UtxoEntry> = plan.commit_inputs.iter().map(|u| u.entry.clone()).collect();
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

/// Compute the on-chain commit txid without signing (for record-before-broadcast).
///
/// # Errors
///
/// [`SignError::EmptyInputs`] / [`SignError::VirtualInput`] if the planned
/// commit inputs are unusable.
pub fn commit_txid(
    plan: &PlannedCommitReveal,
    treasury_script: &ScriptPublicKey,
) -> Result<TransactionId, SignError> {
    let (mut tx, _entries) = build_commit_unsigned(plan, treasury_script)?;
    tx.finalize();
    Ok(tx.id())
}

/// Assemble, sign, finalize, and fully verify the commit transaction.
///
/// Standard P2PK inputs are signed with the treasury key (Schnorr,
/// `SIG_HASH_ALL`) via [`kaspa_consensus_core::sign::sign`], then re-executed
/// through the txscript engine.
///
/// # Errors
///
/// See [`SignError`]: unusable inputs, invalid key, or a failed post-sign
/// script verification.
pub fn sign_commit(
    plan: &PlannedCommitReveal,
    treasury_script: &ScriptPublicKey,
    secret: &TreasurySecret,
) -> Result<SignedCommit, SignError> {
    let (unsigned, entries) = build_commit_unsigned(plan, treasury_script)?;
    let keypair = Keypair::from_seckey_slice(secp256k1::SECP256K1, secret.expose_secret())?;

    let signable = SignableTransaction::with_entries(unsigned, entries.clone());
    let signed = sign(signable, keypair);
    let mut tx = signed.tx;
    tx.finalize();

    let signed = SignedCommit { tx, entries };
    verify(&signed.tx, &signed.entries)?;
    Ok(signed)
}

/// Build the unsigned reveal: one input (the commit P2SH output), one output
/// (the treasury return). The signature script is filled in by [`sign_reveal`].
fn build_reveal_unsigned(
    plan: &PlannedCommitReveal,
    commit_outpoint: TransactionOutpoint,
    treasury_script: &ScriptPublicKey,
) -> Transaction {
    let input = TransactionInput::new(commit_outpoint, vec![], 0, 1);
    let output = TransactionOutput::new(plan.reveal_return_sompi, treasury_script.clone());
    Transaction::new_non_finalized(
        TX_VERSION,
        vec![input],
        vec![output],
        0,
        SUBNETWORK_ID_NATIVE,
        0,
        vec![],
    )
}

/// The UTXO entry the reveal consumes: the commit's P2SH output.
fn reveal_entry(plan: &PlannedCommitReveal) -> UtxoEntry {
    UtxoEntry {
        amount: plan.commit_amount_sompi,
        script_public_key: plan.commit_script_public_key.clone(),
        block_daa_score: 0,
        is_coinbase: false,
        covenant_id: None,
    }
}

/// Compute the on-chain reveal txid without signing (for record-before-broadcast).
///
/// The txid excludes signature scripts, so the unsigned and signed reveal
/// share the same id; only the real `commit_outpoint` is required.
#[must_use]
pub fn reveal_txid(
    plan: &PlannedCommitReveal,
    commit_outpoint: TransactionOutpoint,
    treasury_script: &ScriptPublicKey,
) -> TransactionId {
    let mut tx = build_reveal_unsigned(plan, commit_outpoint, treasury_script);
    tx.finalize();
    tx.id()
}

/// Assemble, sign, finalize, and fully verify the reveal transaction.
///
/// The Schnorr signature is computed over the commit P2SH output's script
/// public key (matching the engine's `OP_CHECKSIG`) and wrapped as
/// `<sig> <pushed redeem script>`. Verification therefore also proves the
/// treasury key matches the pubkey bound into the inscription.
///
/// # Errors
///
/// See [`SignError`]: invalid key, message conversion, redeem-script encoding,
/// or a failed post-sign script verification (incl. key/inscription mismatch).
pub fn sign_reveal(
    plan: &PlannedCommitReveal,
    commit_outpoint: TransactionOutpoint,
    treasury_script: &ScriptPublicKey,
    secret: &TreasurySecret,
) -> Result<SignedReveal, SignError> {
    let mut tx = build_reveal_unsigned(plan, commit_outpoint, treasury_script);
    let entry = reveal_entry(plan);
    let keypair = Keypair::from_seckey_slice(secp256k1::SECP256K1, secret.expose_secret())?;

    // Sighash over the spent P2SH output's script public key — identical to
    // what the engine recomputes in OP_CHECKSIG (see module docs).
    let sig_hash = {
        let verifiable = PopulatedTransaction::new(&tx, vec![entry.clone()]);
        let reused = SigHashReusedValuesUnsync::new();
        calc_schnorr_signature_hash(&verifiable, 0, SIG_HASH_ALL, &reused)
    };
    let msg = Message::from_digest_slice(sig_hash.as_bytes().as_slice())
        .map_err(|_| SignError::Message)?;
    let sig: [u8; 64] = *keypair.sign_schnorr(msg).as_ref();
    // OP_DATA_65 <64-byte signature || SIG_HASH_ALL>, per kaspa_consensus_core::sign.
    let sig_push: Vec<u8> = std::iter::once(65u8)
        .chain(sig)
        .chain([SIG_HASH_ALL.to_u8()])
        .collect();
    let sig_script = pay_to_script_hash_signature_script(plan.redeem_script.clone(), sig_push)?;

    match tx.inputs.get_mut(0) {
        Some(input) => input.signature_script = sig_script,
        None => return Err(SignError::EmptyInputs),
    }
    tx.finalize();

    let signed = SignedReveal { tx, entry };
    verify(&signed.tx, std::slice::from_ref(&signed.entry))?;
    Ok(signed)
}

/// Run the full txscript engine over every input — the same verification
/// kaspad performs — so a bad signature can never leave this process.
fn verify(tx: &Transaction, entries: &[UtxoEntry]) -> Result<(), SignError> {
    let verifiable = PopulatedTransaction::new(tx, entries.to_vec());
    let reused = SigHashReusedValuesUnsync::new();
    let sig_cache: Cache<_, bool> = Cache::new(u64::try_from(entries.len()).unwrap_or(u64::MAX));
    // Mirror the post-Toccata consensus engine (kaspa v2.0.0): covenants are
    // active on every network we target (post-Toccata tn10 / mainnet), so the
    // self-check uses the same script-size limits and rules kaspad applies to
    // the reveal's P2SH redeem script. `EngineFlags` `Default` sets
    // `sigop_script_units: Gram(1000)`, matching the consensus `mass_per_sig_op`
    // (1000) on all networks.
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
