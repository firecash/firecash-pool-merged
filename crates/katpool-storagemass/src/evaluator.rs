//! Mass evaluation using mainnet consensus parameters.

use kaspa_consensus_core::{
    config::params::MAINNET_PARAMS, mass::MassCalculator, tx::PopulatedTransaction,
};

/// Mainnet block mass limit (grams). Each of compute, storage, and
/// transient masses must be ≤ this value independently.
///
/// Post-Toccata, `BlockMassLimits` is per-dimension: compute and storage stay
/// at `500_000` while the transient limit rises to `1_000_000`. We enforce the
/// compute/storage limit uniformly across all three dimensions — it is the
/// binding constraint for payout transactions (storage mass dominates a
/// many-output payout; transient mass for our `P2PK`/`P2SH` shapes is orders of
/// magnitude below `500_000`), so treating transient conservatively at the same
/// bound never rejects a transaction we would actually build. Sourced from
/// consensus params (not hardcoded) so it tracks any future upstream change.
pub const MAINNET_MAX_BLOCK_MASS: u64 = MAINNET_PARAMS.prior_block_mass_limits.compute;

/// Mempool standard-transaction mass limit (grams).
///
/// rusty-kaspa's mempool refuses to relay or accept any transaction whose
/// compute or transient mass exceeds `MAXIMUM_STANDARD_TRANSACTION_MASS`,
/// independent of the higher consensus block mass limit. A transaction that
/// fits a block (`≤ MAINNET_MAX_BLOCK_MASS`, `500_000`) but exceeds this tighter
/// `100_000` bound is rejected at broadcast as non-standard, so any transaction
/// we intend to *submit* must respect this limit — not the block limit. The
/// value is a network-agnostic mempool constant (identical on testnet) and is
/// not exposed via consensus params, so it is pinned here.
///
/// Source: rusty-kaspa `mining/src/mempool/check_transaction_standard.rs`
/// (`MAXIMUM_STANDARD_TRANSACTION_MASS = 100_000`).
pub const MAX_STANDARD_TX_MASS: u64 = 100_000;

/// Minimum payout output per `docs/kips.md` §3 (~0.019 KAS).
pub const MIN_PAYOUT_OUTPUT_SOMPI: u64 = 1_900_000;

/// The three independent transaction masses (grams).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TxMass {
    /// Compute mass (sigops + script size component).
    pub compute_mass: u64,
    /// KIP-9 persistent storage mass.
    pub storage_mass: u64,
    /// KIP-13 transient storage mass (`serialized_size * 4`).
    pub transient_mass: u64,
}

impl TxMass {
    /// Effective mass for fee-rate / mempool ordering (not a consensus rule).
    #[must_use]
    pub fn effective(&self) -> u64 {
        self.compute_mass
            .max(self.storage_mass)
            .max(self.transient_mass)
    }

    /// Whether each mass component fits within the block limit independently.
    #[must_use]
    pub const fn fits_independently(&self, block_mass_limit: u64) -> bool {
        self.compute_mass <= block_mass_limit
            && self.storage_mass <= block_mass_limit
            && self.transient_mass <= block_mass_limit
    }
}

/// Errors from mass evaluation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MassEvaluationError {
    /// Populated input count did not match transaction inputs.
    #[error("input entry count mismatch: tx has {tx_inputs} inputs, got {entries} entries")]
    InputCountMismatch {
        /// Number of transaction inputs.
        tx_inputs: usize,
        /// Number of supplied UTXO entries.
        entries: usize,
    },
    /// Consensus could not compute storage mass (invalid tx shape).
    #[error("storage mass incomputable for this transaction shape")]
    StorageMassIncomputable,
}

/// Evaluates transaction masses against network parameters.
#[derive(Clone)]
pub struct MassEvaluator {
    calculator: MassCalculator,
    block_mass_limit: u64,
}

impl MassEvaluator {
    /// Mainnet evaluator (production payout path).
    #[must_use]
    pub fn mainnet() -> Self {
        Self {
            calculator: MassCalculator::new_with_consensus_params(&MAINNET_PARAMS),
            block_mass_limit: MAINNET_MAX_BLOCK_MASS,
        }
    }

    /// Block mass limit used by [`Self::fits_block`].
    #[must_use]
    pub const fn block_mass_limit(&self) -> u64 {
        self.block_mass_limit
    }

    /// Mempool standard-transaction mass limit ([`MAX_STANDARD_TX_MASS`]).
    ///
    /// This is the binding bound for any transaction we intend to **broadcast**:
    /// the mempool rejects anything above it as non-standard even though it
    /// would fit a block. Planners that produce relayable transactions must
    /// size against this, not [`Self::block_mass_limit`].
    #[must_use]
    pub const fn standard_tx_mass_limit(&self) -> u64 {
        MAX_STANDARD_TX_MASS
    }

    /// Evaluate all three masses for a populated (non-coinbase) transaction.
    pub fn evaluate_populated(
        &self,
        populated: &PopulatedTransaction<'_>,
    ) -> Result<TxMass, MassEvaluationError> {
        if populated.tx.inputs.len() != populated.entries.len() {
            return Err(MassEvaluationError::InputCountMismatch {
                tx_inputs: populated.tx.inputs.len(),
                entries: populated.entries.len(),
            });
        }

        let non_contextual = self.calculator.calc_non_contextual_masses(populated.tx);
        let contextual = self
            .calculator
            .calc_contextual_masses(populated)
            .ok_or(MassEvaluationError::StorageMassIncomputable)?;

        Ok(TxMass {
            compute_mass: non_contextual.compute_mass,
            transient_mass: non_contextual.transient_mass,
            storage_mass: contextual.storage_mass,
        })
    }

    /// Convenience: evaluate and check the configured block limit.
    #[must_use]
    pub fn fits_block(&self, populated: &PopulatedTransaction<'_>) -> bool {
        self.evaluate_populated(populated)
            .is_ok_and(|m| m.fits_independently(self.block_mass_limit))
    }
}
