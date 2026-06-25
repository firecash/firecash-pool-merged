//! Network fee policy for payout transactions.
//!
//! The planner must reserve a real on-chain fee, because kaspad's mempool
//! rejects any transaction whose fee is below the minimum relay fee
//! (`RejectInsufficientFee`) or whose change output is dust (`RejectDust`).
//! Both rules are mirrored verbatim from rusty-kaspa
//! `mining/src/mempool/check_transaction_standard.rs` (tag `v2.0.0`; the
//! post-Toccata `100_000` sompi/kg floor and `max(compute, transient)` fee-mass
//! rule are unchanged from `tn10-toc3`) so the offline planner reserves exactly
//! what the live node will require:
//!
//! - `minimum_required_transaction_relay_fee(mass) = (mass * min_relay) / 1000`,
//!   floored at `min_relay` when the product underflows to zero.
//! - an output is dust iff `value * 1000 / (3 * (size + 148)) < min_relay`,
//!   where `size` is the output's estimated serialized size.
//!
//! `min_relay` is kaspad's `minimum_relay_transaction_fee`, default
//! `DEFAULT_MINIMUM_RELAY_TRANSACTION_FEE = 100_000` sompi/kg.

use kaspa_consensus_core::mass::transaction_output_estimated_serialized_size;
use kaspa_consensus_core::tx::{ScriptPublicKey, TransactionOutput};

use crate::evaluator::TxMass;

/// kaspad's default `minimum_relay_transaction_fee` in sompi per kilogram,
/// from `mining/src/mempool/config.rs::DEFAULT_MINIMUM_RELAY_TRANSACTION_FEE`.
///
/// Mirrored (not imported) because the mempool crate is not a dependency — the
/// fee rule is enforced by the remote node, and we reproduce it offline.
pub const MIN_RELAY_TX_FEE_SOMPI_PER_KG: u64 = 100_000;

/// Minimum input serialized size kaspad assumes when scoring an output for
/// dust (a typical p2pk redeeming input), from `is_transaction_output_dust`.
const DUST_INPUT_SERIALIZED_SIZE: u64 = 148;

/// Network fee policy: a `sompi/gram` fee-rate plus the mempool relay floor.
///
/// `feerate_sompi_per_gram` comes from kaspad's `get_fee_estimate`
/// (`RpcFeerateBucket.feerate`); the fee for a transaction is
/// `feerate * effective_mass`, floored at the minimum relay fee derived from
/// the transaction's compute mass. `effective_mass` is the max over compute,
/// storage, and transient mass — the same quantity kaspad uses to order the
/// block template — so the resulting fee/mass ratio satisfies the threshold on
/// every mass dimension.
#[derive(Debug, Clone, Copy)]
pub struct FeeRate {
    feerate_sompi_per_gram: f64,
    min_relay_sompi_per_kg: u64,
}

impl FeeRate {
    /// No-fee policy: reserves nothing and never reshapes a batch.
    ///
    /// For shape-only planning and unit tests that assert raw packing. Never
    /// use on the live payout path — a zero-fee transaction is rejected by
    /// kaspad's mempool (`RejectInsufficientFee`).
    pub const ZERO: Self = Self {
        feerate_sompi_per_gram: 0.0,
        min_relay_sompi_per_kg: 0,
    };

    /// Build a live fee policy from a node-reported `sompi/gram` fee-rate,
    /// flooring every fee at kaspad's default minimum relay fee.
    ///
    /// Negative or non-finite rates collapse to the relay floor.
    #[must_use]
    pub fn from_feerate(feerate_sompi_per_gram: f64) -> Self {
        let feerate = if feerate_sompi_per_gram.is_finite() && feerate_sompi_per_gram > 0.0 {
            feerate_sompi_per_gram
        } else {
            0.0
        };
        Self {
            feerate_sompi_per_gram: feerate,
            min_relay_sompi_per_kg: MIN_RELAY_TX_FEE_SOMPI_PER_KG,
        }
    }

    /// Whether this policy reserves a fee (and so may reshape change to avoid
    /// dust). False only for [`FeeRate::ZERO`].
    #[must_use]
    pub const fn reserves_fee(&self) -> bool {
        self.feerate_sompi_per_gram > 0.0 || self.min_relay_sompi_per_kg > 0
    }

    /// Fee in sompi to reserve for a transaction with the given masses.
    ///
    /// The feerate is a `sompi/gram` ratio that only kaspad supplies as `f64`,
    /// so the estimate is computed in floating point and then saturated to an
    /// integer sompi amount with finite/positive guards.
    #[must_use]
    #[allow(
        clippy::float_arithmetic,
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    pub fn fee_for(&self, mass: &TxMass) -> u64 {
        let scaled = self.feerate_sompi_per_gram * mass.effective() as f64;
        let estimate = if scaled.is_finite() && scaled > 0.0 {
            // Payout masses keep this far below 2^53, so the cast is exact;
            // clamp anyway to stay total.
            scaled.ceil().min(u64::MAX as f64) as u64
        } else {
            0
        };
        estimate.max(self.minimum_relay_fee(mass.compute_mass))
    }

    /// Mirror of kaspad `minimum_required_transaction_relay_fee`.
    ///
    /// The `/ 1000` is consensus integer division (sompi/kg → sompi), copied
    /// verbatim from the node so our floor matches its check bit-for-bit.
    #[allow(clippy::integer_division)]
    const fn minimum_relay_fee(&self, compute_mass: u64) -> u64 {
        let fee = compute_mass.saturating_mul(self.min_relay_sompi_per_kg) / 1000;
        if fee == 0 {
            self.min_relay_sompi_per_kg
        } else {
            fee
        }
    }
}

/// Whether a treasury change output of `value` sompi would be rejected as dust.
///
/// Mirror of kaspad `is_transaction_output_dust` for a spendable output: the
/// network cost to redeem the coin exceeds 1/3 of the minimum relay fee.
#[must_use]
#[allow(clippy::integer_division)]
pub fn is_change_dust(value_sompi: u64, change_script: &ScriptPublicKey) -> bool {
    let output = TransactionOutput::new(value_sompi, change_script.clone());
    let serialized = transaction_output_estimated_serialized_size(&output)
        .saturating_add(DUST_INPUT_SERIALIZED_SIZE);
    // value * 1000 / (3 * serialized) < min_relay  ⇒ dust. u128 avoids overflow;
    // the integer division mirrors kaspad's dust check exactly.
    let lhs = u128::from(value_sompi).saturating_mul(1000) / (3u128 * u128::from(serialized));
    lhs < u128::from(MIN_RELAY_TX_FEE_SOMPI_PER_KG)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mass(compute: u64, storage: u64, transient: u64) -> TxMass {
        TxMass {
            compute_mass: compute,
            storage_mass: storage,
            transient_mass: transient,
        }
    }

    #[test]
    fn zero_policy_reserves_nothing() {
        assert!(!FeeRate::ZERO.reserves_fee());
        assert_eq!(FeeRate::ZERO.fee_for(&mass(5_000, 5_000, 5_000)), 0);
    }

    #[test]
    fn relay_floor_uses_compute_mass() {
        // feerate 0 ⇒ fee is the relay floor = compute_mass * 100_000 / 1000.
        let fr = FeeRate::from_feerate(0.0);
        assert!(fr.reserves_fee());
        assert_eq!(fr.fee_for(&mass(3_000, 9_000, 1_000)), 3_000 * 100);
    }

    #[test]
    fn feerate_applies_to_effective_mass_and_floors_at_relay() {
        // High feerate dominates the relay floor; effective mass is the max.
        let fr = FeeRate::from_feerate(2.0);
        // effective = 9_000; estimate = 18_000; relay floor = 3_000*100 = 300_000.
        assert_eq!(fr.fee_for(&mass(3_000, 9_000, 1_000)), 300_000);
        // Now make the feerate estimate exceed the floor.
        let fr = FeeRate::from_feerate(200.0);
        // estimate = 200 * 9_000 = 1_800_000 > 300_000.
        assert_eq!(fr.fee_for(&mass(3_000, 9_000, 1_000)), 1_800_000);
    }

    #[test]
    fn negative_and_nan_feerate_collapse_to_floor() {
        for bad in [-1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let fr = FeeRate::from_feerate(bad);
            assert_eq!(fr.fee_for(&mass(2_000, 0, 0)), 2_000 * 100);
        }
    }
}
