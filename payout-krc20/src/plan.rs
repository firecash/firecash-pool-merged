//! Mass-aware KRC-20 commit/reveal planner.
//!
//! A KRC-20 NACHO transfer is two transactions (see [ADR-0015],
//! `docs/kips.md` §5.2):
//!
//! - **commit** — funds a P2SH output whose redeem script is the kasplex
//!   inscription envelope, plus change back to the treasury;
//! - **reveal** — spends that single P2SH output (exposing the inscription
//!   in its signature script) and returns the funds, minus the reveal fee,
//!   to the treasury. Exactly **one input, one output** keeps it small.
//!
//! Both transactions must independently satisfy every KIP-9/KIP-13 mass
//! against `max_block_mass`. The reveal is the interesting one: its
//! `transient_storage_mass` is driven by the redeem-script-and-data path in
//! the signature script, so — unlike the KAS planner, which evaluates
//! unsigned shapes — this planner sizes the signature scripts to their
//! **signed** length before evaluating. The 32-byte Schnorr signature push
//! is [`STANDARD_SIGNATURE_SCRIPT_LEN`] bytes (rusty-kaspa
//! `wallet::tx::mass::SIGNATURE_SIZE`); the reveal additionally carries the
//! canonical push of the full redeem script.
//!
//! [ADR-0015]: ../../../docs/decisions/0015-krc20-inscription-envelope.md

use kaspa_consensus_core::{
    subnets::SUBNETWORK_ID_NATIVE,
    tx::{
        PopulatedTransaction, ScriptPublicKey, Transaction, TransactionId, TransactionInput,
        TransactionOutpoint, TransactionOutput, UtxoEntry,
    },
};
use katpool_storagemass::{
    FeeRate, MIN_PAYOUT_OUTPUT_SOMPI, MassEvaluationError, MassEvaluator, TreasuryUtxo, TxMass,
    is_change_dust,
};

use crate::inscription::{
    InscriptionError, Krc20Transfer, build_transfer_inscription, commit_script_public_key,
    reveal_signature_script,
};

/// Signed length of a standard Schnorr P2PK signature script, in bytes:
/// `OP_DATA_65 (1) + 64-byte signature + 1 sighash byte`. Matches
/// rusty-kaspa `wallet::tx::mass::SIGNATURE_SIZE`.
pub const STANDARD_SIGNATURE_SCRIPT_LEN: usize = 66;

/// Default amount locked into the commit P2SH output (`0.2 KAS`).
///
/// Spent in full by the reveal and returned to the treasury minus the
/// reveal fee, so it is a transient lock, not a cost. Must exceed
/// `reveal_fee + MIN_PAYOUT_OUTPUT_SOMPI`.
pub const DEFAULT_COMMIT_AMOUNT_SOMPI: u64 = 20_000_000;

/// Per-input sigop count: one `OP_CHECKSIG` per standard or P2SH input.
const SIG_OP_COUNT_PER_INPUT: u8 = 1;

/// How the commit/reveal network fees are sized.
///
/// kaspad rejects any transaction below the mass-based minimum relay fee, so
/// the fee must be derived from each transaction's mass — never a fixed
/// constant. The commit `change` and reveal `return` values (and therefore
/// both txids, which are recorded *before* broadcast) depend on the fee, so a
/// crash-resume must reproduce the identical fee:
///
/// - [`Krc20FeePolicy::Adaptive`] sizes the fee from the live node fee-rate
///   (floored at the relay minimum) on the **first** plan of a transfer.
/// - [`Krc20FeePolicy::Frozen`] replays the fees persisted at that first plan
///   so every later reconstruction (reveal build, drift check, commit
///   re-broadcast) re-derives a bit-identical transaction.
#[derive(Debug, Clone, Copy)]
pub enum Krc20FeePolicy {
    /// Size both fees from the live fee-rate, floored at the relay minimum.
    Adaptive(FeeRate),
    /// Replay the exact fees frozen at first execution.
    Frozen {
        /// Frozen commit network fee (sompi).
        commit_fee_sompi: u64,
        /// Frozen reveal network fee (sompi).
        reveal_fee_sompi: u64,
    },
}

impl Krc20FeePolicy {
    /// The reveal fee under this policy for a reveal of the given mass.
    fn reveal_fee(&self, reveal_mass: &TxMass) -> u64 {
        match self {
            Self::Adaptive(rate) => rate.fee_for(reveal_mass),
            Self::Frozen {
                reveal_fee_sompi, ..
            } => *reveal_fee_sompi,
        }
    }

    /// The commit fee under this policy for a commit of the given mass.
    fn commit_fee(&self, commit_mass: &TxMass) -> u64 {
        match self {
            Self::Adaptive(rate) => rate.fee_for(commit_mass),
            Self::Frozen {
                commit_fee_sompi, ..
            } => *commit_fee_sompi,
        }
    }
}

/// Amount locked into the commit P2SH and the policy that sizes the fees.
#[derive(Debug, Clone, Copy)]
pub struct CommitRevealConfig {
    /// Amount locked into the commit P2SH output (returned by the reveal).
    pub commit_amount_sompi: u64,
    /// How the commit/reveal fees are sized.
    pub fee_policy: Krc20FeePolicy,
}

/// A mass-valid KRC-20 commit/reveal pair for one NACHO transfer.
#[derive(Debug, Clone)]
pub struct PlannedCommitReveal {
    /// The inscription redeem script (commit P2SH preimage; reveal exposes it).
    pub redeem_script: Vec<u8>,
    /// The P2SH script public key the commit pays to.
    pub commit_script_public_key: ScriptPublicKey,
    /// Amount locked into the commit P2SH output (the reveal spends it in full).
    pub commit_amount_sompi: u64,
    /// Treasury inputs the commit consumes.
    pub commit_inputs: Vec<TreasuryUtxo>,
    /// Change returned to the treasury by the commit (0 when folded to fee).
    pub commit_change_sompi: u64,
    /// Resolved commit network fee (sompi) — persist to freeze the shape.
    pub commit_fee_sompi: u64,
    /// The commit transaction's three masses (signed-size accurate).
    pub commit_mass: TxMass,
    /// Amount the reveal returns to the treasury (`commit_amount − reveal_fee`).
    pub reveal_return_sompi: u64,
    /// Resolved reveal network fee (sompi) — persist to freeze the shape.
    pub reveal_fee_sompi: u64,
    /// The reveal transaction's three masses (includes the redeem-script push).
    pub reveal_mass: TxMass,
}

impl PlannedCommitReveal {
    /// Reconstruct only the reveal-relevant fields (redeem script, P2SH spk,
    /// commit amount, reveal return) for **resuming** a transfer whose commit
    /// is already on chain.
    ///
    /// No treasury UTXOs are consulted and no mass is recomputed — the commit
    /// funding and mass were validated when the transfer was first planned
    /// (M5.3); resume only needs to re-sign the reveal of the existing commit
    /// output. `commit_inputs`/`commit_change_sompi` are therefore empty/zero
    /// and the mass fields are zeroed; do **not** use a `reveal_only` plan for
    /// commit signing or mass decisions.
    ///
    /// # Errors
    ///
    /// [`PlanError::Inscription`] if the envelope cannot be built, or
    /// [`PlanError::RevealBelowFloor`] if `commit_amount − reveal_fee` is
    /// below the dust floor.
    pub fn reveal_only(
        xonly_pubkey: &[u8; 32],
        transfer: &Krc20Transfer,
        commit_amount_sompi: u64,
        reveal_fee_sompi: u64,
    ) -> Result<Self, PlanError> {
        let reveal_return_sompi = commit_amount_sompi
            .checked_sub(reveal_fee_sompi)
            .filter(|r| *r >= MIN_PAYOUT_OUTPUT_SOMPI)
            .ok_or_else(|| PlanError::RevealBelowFloor {
                return_sompi: commit_amount_sompi.saturating_sub(reveal_fee_sompi),
                floor_sompi: MIN_PAYOUT_OUTPUT_SOMPI,
            })?;
        let redeem_script = build_transfer_inscription(xonly_pubkey, transfer)?;
        let commit_script_public_key = commit_script_public_key(&redeem_script);
        let zero_mass = TxMass {
            compute_mass: 0,
            storage_mass: 0,
            transient_mass: 0,
        };
        Ok(Self {
            redeem_script,
            commit_script_public_key,
            commit_amount_sompi,
            commit_inputs: Vec::new(),
            commit_change_sompi: 0,
            commit_fee_sompi: 0,
            reveal_return_sompi,
            reveal_fee_sompi,
            commit_mass: zero_mass,
            reveal_mass: zero_mass,
        })
    }
}

/// Reasons a commit/reveal pair cannot be planned.
#[derive(Debug, thiserror::Error)]
pub enum PlanError {
    /// The inscription envelope could not be built.
    #[error("inscription: {0}")]
    Inscription(#[from] InscriptionError),

    /// Treasury UTXOs cannot cover `commit_amount + commit_fee`.
    #[error("insufficient treasury funds: need {needed_sompi} sompi, have {available_sompi}")]
    InsufficientFunds {
        /// Sompi required (`commit_amount + commit_fee`).
        needed_sompi: u64,
        /// Sompi available across the supplied treasury UTXOs.
        available_sompi: u64,
    },

    /// `commit_amount − reveal_fee` is below the dust floor, so the reveal
    /// output would be unspendable / non-standard.
    #[error("reveal return {return_sompi} sompi below floor {floor_sompi}")]
    RevealBelowFloor {
        /// The computed reveal return.
        return_sompi: u64,
        /// [`MIN_PAYOUT_OUTPUT_SOMPI`].
        floor_sompi: u64,
    },

    /// The commit transaction exceeds the block mass limit on some component.
    #[error("commit mass exceeds block limit {limit}: {mass:?}")]
    CommitMassExceeded {
        /// The offending masses.
        mass: TxMass,
        /// The block mass limit.
        limit: u64,
    },

    /// The reveal transaction exceeds the block mass limit on some component.
    #[error("reveal mass exceeds block limit {limit}: {mass:?}")]
    RevealMassExceeded {
        /// The offending masses.
        mass: TxMass,
        /// The block mass limit.
        limit: u64,
    },

    /// Consensus could not compute mass for the planned shape.
    #[error("mass evaluation: {0}")]
    MassEval(#[from] MassEvaluationError),
}

/// Plans the commit/reveal pair for a single KRC-20 transfer and verifies
/// both transactions fit every mass independently.
///
/// `treasury_script` is the treasury's P2PK script (for commit change and
/// the reveal return). `xonly_pubkey` is the treasury's 32-byte Schnorr key
/// bound into the inscription.
///
/// # Errors
///
/// See [`PlanError`]: bad inscription, underfunded commit, sub-floor reveal
/// return, either transaction over the mass limit, or an incomputable mass.
pub fn plan_commit_reveal(
    evaluator: &MassEvaluator,
    treasury_utxos: &[TreasuryUtxo],
    treasury_script: &ScriptPublicKey,
    xonly_pubkey: &[u8; 32],
    transfer: &Krc20Transfer,
    cfg: &CommitRevealConfig,
) -> Result<PlannedCommitReveal, PlanError> {
    let redeem_script = build_transfer_inscription(xonly_pubkey, transfer)?;
    let commit_spk = commit_script_public_key(&redeem_script);

    // ---- reveal: 1 input (commit P2SH) → 1 output (treasury return) -----
    // The commit txid is unknown until the commit is signed; the outpoint
    // value does not affect mass (fixed 32+4 bytes), so a placeholder outpoint
    // suffices. Likewise the return *value* does not move mass, so size the
    // mass with a placeholder, derive the reveal fee from it, then set the
    // real return (`commit_amount − reveal_fee`).
    let commit_p2sh_outpoint = TransactionOutpoint {
        transaction_id: TransactionId::from_bytes([0u8; 32]),
        index: 0,
    };
    let reveal_sig_script = reveal_signature_script(
        redeem_script.clone(),
        vec![0u8; STANDARD_SIGNATURE_SCRIPT_LEN],
    )?;
    let reveal_tx = build_tx(
        vec![(commit_p2sh_outpoint, reveal_sig_script)],
        vec![TransactionOutput::new(
            cfg.commit_amount_sompi,
            treasury_script.clone(),
        )],
    );
    let reveal_entry = UtxoEntry {
        amount: cfg.commit_amount_sompi,
        script_public_key: commit_spk.clone(),
        block_daa_score: 0,
        is_coinbase: false,
        covenant_id: None,
    };
    let reveal_mass = evaluate(evaluator, &reveal_tx, vec![reveal_entry])?;
    if !reveal_mass.fits_independently(evaluator.block_mass_limit()) {
        return Err(PlanError::RevealMassExceeded {
            mass: reveal_mass,
            limit: evaluator.block_mass_limit(),
        });
    }
    let reveal_fee_sompi = cfg.fee_policy.reveal_fee(&reveal_mass);
    let reveal_return_sompi = cfg
        .commit_amount_sompi
        .checked_sub(reveal_fee_sompi)
        .filter(|r| *r >= MIN_PAYOUT_OUTPUT_SOMPI)
        .ok_or_else(|| PlanError::RevealBelowFloor {
            return_sompi: cfg.commit_amount_sompi.saturating_sub(reveal_fee_sompi),
            floor_sompi: MIN_PAYOUT_OUTPUT_SOMPI,
        })?;

    // ---- commit: lock `commit_amount` into the P2SH, reserve the network
    // fee out of treasury change, widening the funding set until the inputs
    // cover `commit_amount + fee`. -----------------------------------------
    let commit = fund_commit(
        evaluator,
        treasury_utxos,
        &commit_spk,
        treasury_script,
        cfg.commit_amount_sompi,
        &cfg.fee_policy,
    )?;
    if !commit.mass.fits_independently(evaluator.block_mass_limit()) {
        return Err(PlanError::CommitMassExceeded {
            mass: commit.mass,
            limit: evaluator.block_mass_limit(),
        });
    }

    Ok(PlannedCommitReveal {
        redeem_script,
        commit_script_public_key: commit_spk,
        commit_amount_sompi: cfg.commit_amount_sompi,
        commit_inputs: commit.inputs,
        commit_change_sompi: commit.change_sompi,
        commit_fee_sompi: commit.fee_sompi,
        commit_mass: commit.mass,
        reveal_return_sompi,
        reveal_fee_sompi,
        reveal_mass,
    })
}

/// A funded commit shape: the selected inputs plus the resolved fee, change,
/// and mass for the transaction that will actually be signed.
struct FundedCommit {
    inputs: Vec<TreasuryUtxo>,
    change_sompi: u64,
    fee_sompi: u64,
    mass: TxMass,
}

/// Fund `commit_amount + fee`, reserving the network fee out of change.
///
/// Mirrors the KAS planner: size the fee from the pre-fee (change-present)
/// shape — mass is insensitive to the change *value*, only its presence —
/// then drop a zero/dust change output into the fee (re-measuring the
/// no-change shape so the recorded mass matches what is signed). The
/// largest-first funding set is widened until the inputs cover the locked
/// amount plus the resulting fee. Under [`Krc20FeePolicy::Frozen`] the same
/// arithmetic with the persisted fee reproduces the original shape exactly.
fn fund_commit(
    evaluator: &MassEvaluator,
    treasury_utxos: &[TreasuryUtxo],
    commit_spk: &ScriptPublicKey,
    treasury_script: &ScriptPublicKey,
    commit_amount_sompi: u64,
    fee_policy: &Krc20FeePolicy,
) -> Result<FundedCommit, PlanError> {
    let mut sorted: Vec<TreasuryUtxo> = treasury_utxos.to_vec();
    sorted.sort_by(|a, b| b.entry.amount.cmp(&a.entry.amount));
    let available: u64 = sorted.iter().map(|u| u.entry.amount).sum();

    let mut input_count = 1;
    while input_count <= sorted.len() {
        let Some(inputs) = sorted.get(..input_count) else {
            break;
        };
        let input_sum: u64 = inputs.iter().map(|u| u.entry.amount).sum();
        // The inputs must at least cover the locked amount before a fee can be
        // reserved from the remainder.
        if input_sum <= commit_amount_sompi {
            input_count += 1;
            continue;
        }

        let gross_change = input_sum - commit_amount_sompi;
        let gross_mass = commit_mass(
            evaluator,
            inputs,
            commit_spk,
            commit_amount_sompi,
            treasury_script,
            gross_change,
        )?;
        let fee = fee_policy.commit_fee(&gross_mass);
        if commit_amount_sompi.saturating_add(fee) > input_sum {
            input_count += 1;
            continue;
        }
        let change = input_sum - commit_amount_sompi - fee;

        // Fold a zero/dust change output into the fee (kaspad rejects dust);
        // the no-change shape is re-measured so the recorded mass matches the
        // signed transaction. The leftover is absorbed, so the effective fee
        // is `input_sum − commit_amount` — which a frozen replay reproduces.
        if change == 0 || is_change_dust(change, treasury_script) {
            let mass = commit_mass(
                evaluator,
                inputs,
                commit_spk,
                commit_amount_sompi,
                treasury_script,
                0,
            )?;
            return Ok(FundedCommit {
                inputs: inputs.to_vec(),
                change_sompi: 0,
                fee_sompi: input_sum - commit_amount_sompi,
                mass,
            });
        }
        return Ok(FundedCommit {
            inputs: inputs.to_vec(),
            change_sompi: change,
            fee_sompi: fee,
            mass: gross_mass,
        });
    }

    // Report the fee the full funding set would owe, for an actionable error.
    let needed_sompi = available
        .checked_sub(commit_amount_sompi)
        .and_then(|gross_change| {
            commit_mass(
                evaluator,
                &sorted,
                commit_spk,
                commit_amount_sompi,
                treasury_script,
                gross_change,
            )
            .ok()
        })
        .map_or(commit_amount_sompi, |m| {
            commit_amount_sompi.saturating_add(fee_policy.commit_fee(&m))
        });
    Err(PlanError::InsufficientFunds {
        needed_sompi,
        available_sompi: available,
    })
}

/// Evaluate the commit transaction's mass for the given inputs and change.
fn commit_mass(
    evaluator: &MassEvaluator,
    inputs: &[TreasuryUtxo],
    commit_spk: &ScriptPublicKey,
    commit_amount_sompi: u64,
    treasury_script: &ScriptPublicKey,
    change_sompi: u64,
) -> Result<TxMass, MassEvaluationError> {
    let mut outputs = vec![TransactionOutput::new(
        commit_amount_sompi,
        commit_spk.clone(),
    )];
    if change_sompi > 0 {
        outputs.push(TransactionOutput::new(
            change_sompi,
            treasury_script.clone(),
        ));
    }
    let tx = build_tx(
        inputs
            .iter()
            .map(|u| (u.outpoint, vec![0u8; STANDARD_SIGNATURE_SCRIPT_LEN]))
            .collect(),
        outputs,
    );
    let entries: Vec<UtxoEntry> = inputs.iter().map(|u| u.entry.clone()).collect();
    evaluate(evaluator, &tx, entries)
}

/// Builds an unsigned-shape transaction whose inputs already carry the
/// signed-length signature scripts (so transient mass is accurate).
fn build_tx(
    inputs: Vec<(TransactionOutpoint, Vec<u8>)>,
    outputs: Vec<TransactionOutput>,
) -> Transaction {
    let tx_inputs: Vec<TransactionInput> = inputs
        .into_iter()
        .map(|(previous_outpoint, signature_script)| {
            TransactionInput::new(
                previous_outpoint,
                signature_script,
                0,
                SIG_OP_COUNT_PER_INPUT,
            )
        })
        .collect();
    Transaction::new(0, tx_inputs, outputs, 0, SUBNETWORK_ID_NATIVE, 0, vec![])
}

/// Evaluates the three masses for `tx` populated with `entries`.
fn evaluate(
    evaluator: &MassEvaluator,
    tx: &Transaction,
    entries: Vec<UtxoEntry>,
) -> Result<TxMass, MassEvaluationError> {
    let populated = PopulatedTransaction::new(tx, entries);
    evaluator.evaluate_populated(&populated)
}
