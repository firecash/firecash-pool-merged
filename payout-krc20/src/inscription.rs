//! Kasplex KRC-20 inscription primitives: the commit/reveal envelope.
//!
//! KRC-20 operations are inscribed on Kaspa via a two-transaction
//! commit/reveal flow. The *commit* transaction pays a P2SH output whose
//! redeem script embeds the operation as canonical data pushes inside a
//! never-executed `OP_FALSE OP_IF … OP_ENDIF` block; the *reveal*
//! transaction spends that output, exposing the redeem script — and thus
//! the inscription — on-chain for the kasplex indexer to read.
//!
//! # Envelope layout
//!
//! ```text
//! <32-byte x-only pubkey> OP_CHECKSIG OP_FALSE OP_IF
//!     "kasplex" OP_0 <json> OP_ENDIF
//! ```
//!
//! The `<pubkey> OP_CHECKSIG` prefix makes the script spendable by the
//! holder of the matching Schnorr key (the reveal signature satisfies it).
//! Everything after `OP_FALSE OP_IF` is dead code the engine never runs,
//! but kaspad still requires it to parse as canonical pushes — which is
//! how the data rides on-chain.
//!
//! # Provenance of the exact bytes
//!
//! This layout is byte-for-byte identical to the live `katpool-payment`
//! production transfer (`src/trxs/krc20/krc20Transfer.ts`), which is
//! currently settling NACHO rebates on-chain and is therefore accepted by
//! the kasplex indexer. It matches the rusty-kaspa WASM `ScriptBuilder`
//! convention used across the KRC-20 ecosystem (a single `addI64(0)` push,
//! Schnorr `OP_CHECKSIG` with an x-only pubkey — *not* `OP_CHECKSIG_ECDSA`,
//! and *not* the `OP_1 OP_0 OP_0` marker triplet some prose specs describe;
//! see ADR-0014).

use kaspa_addresses::{Address, Prefix};
use kaspa_consensus_core::tx::ScriptPublicKey;
use kaspa_txscript::{
    extract_script_pub_key_address,
    opcodes::codes::{OpCheckSig, OpEndIf, OpFalse, OpIf},
    pay_to_script_hash_script, pay_to_script_hash_signature_script,
    script_builder::ScriptBuilder,
};
use serde::Serialize;

/// The kasplex protocol tag that prefixes every inscription payload.
pub const KASPLEX_TAG: &[u8] = b"kasplex";

/// The kasplex protocol identifier carried in the `p` field of every op.
pub const KRC20_PROTOCOL: &str = "krc-20";

/// Errors from assembling a KRC-20 inscription.
#[derive(Debug, thiserror::Error)]
pub enum InscriptionError {
    /// The redeem script exceeded the canonical script-size limit.
    #[error("script builder: {0}")]
    ScriptBuilder(#[from] kaspa_txscript::script_builder::ScriptBuilderError),

    /// The P2SH script public key could not be decoded back into an address.
    #[error("p2sh address: {0}")]
    Address(String),

    /// The op could not be serialised to its JSON payload.
    #[error("json serialisation: {0}")]
    Json(#[from] serde_json::Error),
}

/// A KRC-20 `transfer` operation, serialised to the canonical kasplex JSON
/// payload `{"p":"krc-20","op":"transfer","tick":..,"amt":..,"to":..}`.
///
/// Field order is part of the on-chain bytes: the JSON is pushed verbatim,
/// so `p, op, tick, amt, to` must serialise in exactly that order (compact,
/// no whitespace) to match the indexer's expectation and the production
/// pool. `amt` is the integer token amount in base units (decimals already
/// applied) as a decimal string, mirroring `kaspaToSompi`-style handling.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Krc20Transfer {
    /// Protocol identifier; always [`KRC20_PROTOCOL`].
    pub p: &'static str,
    /// Operation; always `"transfer"`.
    pub op: &'static str,
    /// Token ticker, e.g. `"NACHO"`.
    pub tick: String,
    /// Amount in integer base units (decimals applied) as a decimal string.
    pub amt: String,
    /// Recipient Kaspa address string, e.g. `"kaspa:qr…"`.
    pub to: String,
}

impl Krc20Transfer {
    /// Builds a `transfer` op with the protocol/op fields fixed.
    #[must_use]
    pub fn new(tick: impl Into<String>, amt: impl Into<String>, to: impl Into<String>) -> Self {
        Self {
            p: KRC20_PROTOCOL,
            op: "transfer",
            tick: tick.into(),
            amt: amt.into(),
            to: to.into(),
        }
    }

    /// Serialises to the canonical compact JSON payload pushed on-chain.
    ///
    /// # Errors
    ///
    /// Returns the underlying `serde_json` error. Not reachable in practice
    /// (the value is a fixed set of string fields), but propagated rather
    /// than panicked per the crate's no-`expect` policy.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

/// Builds the commit redeem script (the inscription envelope) for a KRC-20
/// `transfer`, bound to the treasury's 32-byte x-only Schnorr public key.
///
/// The returned bytes are the redeem script: hash it into a P2SH output
/// with [`commit_script_public_key`] for the commit transaction, and reveal
/// it with [`reveal_signature_script`] when spending that output.
///
/// # Errors
///
/// Returns [`InscriptionError::ScriptBuilder`] only if the payload is large
/// enough to exceed the canonical script-size limit (not reachable for a
/// single transfer op).
pub fn build_transfer_inscription(
    xonly_pubkey: &[u8; 32],
    transfer: &Krc20Transfer,
) -> Result<Vec<u8>, InscriptionError> {
    let json = transfer.to_json()?;
    let script = ScriptBuilder::new()
        .add_data(xonly_pubkey)?
        .add_op(OpCheckSig)?
        .add_op(OpFalse)?
        .add_op(OpIf)?
        .add_data(KASPLEX_TAG)?
        .add_i64(0)?
        .add_data(json.as_bytes())?
        .add_op(OpEndIf)?
        .drain();
    Ok(script)
}

/// Returns the P2SH script public key (blake2b-256 of the redeem script)
/// that the commit transaction must pay to.
#[must_use]
pub fn commit_script_public_key(redeem_script: &[u8]) -> ScriptPublicKey {
    pay_to_script_hash_script(redeem_script)
}

/// Derives the P2SH commit address for the given redeem script and network.
///
/// # Errors
///
/// Returns [`InscriptionError::Address`] if the derived P2SH script public
/// key cannot be decoded into an address (not reachable for a well-formed
/// P2SH script).
pub fn commit_address(redeem_script: &[u8], prefix: Prefix) -> Result<Address, InscriptionError> {
    let spk = commit_script_public_key(redeem_script);
    extract_script_pub_key_address(&spk, prefix)
        .map_err(|e| InscriptionError::Address(e.to_string()))
}

/// Builds the reveal-transaction signature script `<sig> <redeem_script>`
/// that satisfies the commit P2SH output, exposing the inscription on-chain.
///
/// # Errors
///
/// Returns [`InscriptionError::ScriptBuilder`] if the redeem script is too
/// large to embed as a canonical data push (not reachable for a transfer).
pub fn reveal_signature_script(
    redeem_script: Vec<u8>,
    signature: Vec<u8>,
) -> Result<Vec<u8>, InscriptionError> {
    Ok(pay_to_script_hash_signature_script(
        redeem_script,
        signature,
    )?)
}
