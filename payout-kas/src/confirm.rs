//! Pure confirmation + maturity policy (no I/O), so the rules are unit-tested
//! independently of any node.

/// DAA-score depth after which an accepted KAS payout is treated as settled.
///
/// Conservative relative to the ~10-block non-coinbase finality kaspad uses
/// for spendability; tunable per network in M4.7.
pub const KAS_PAYOUT_CONFIRMATION_DAA: u64 = 100;

/// Confirmations a non-coinbase treasury coin needs before it is spendable.
pub const NON_COINBASE_MATURITY_DAA: u64 = 10;

/// Confirmations a coinbase treasury coin (mining reward) needs before
/// it is spendable.
///
/// Matches Kaspa consensus `coinbase_maturity` for mainnet and
/// testnet-10: `BPS(10) × COINBASE_MATURITY_SECONDS(100) = 1000`
/// DAA-score depth. Spending a coinbase UTXO shallower than this is
/// rejected by consensus, so selecting one for a payout would produce
/// an invalid transaction.
pub const COINBASE_MATURITY_DAA: u64 = 1000;

/// Derived state of a submitted payout transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmationState {
    /// Still in the mempool, awaiting block inclusion.
    Pending,
    /// Included on chain (a change/output coin exists) but below the
    /// confirmation depth.
    Accepted,
    /// Included and matured past [`KAS_PAYOUT_CONFIRMATION_DAA`].
    Confirmed,
    /// Neither in the mempool nor observably on chain. The executor makes **no**
    /// state change for this — it is resolved by a later pass or operator
    /// reconciliation (M4.8), never auto-failed, so funds can't be re-sent.
    Unknown,
}

/// Reads gathered for one submitted transaction, fed to [`classify_confirmation`].
#[derive(Debug, Clone, Copy)]
pub struct ConfirmationInputs {
    /// Current virtual DAA score.
    pub virtual_daa_score: u64,
    /// Whether the txid is still in the mempool.
    pub in_mempool: bool,
    /// `block_daa_score` of an on-chain coin produced by this tx (the treasury
    /// change output), if one is observed *this pass*; `None` otherwise.
    pub change_block_daa_score: Option<u64>,
    /// Accepting DAA score durably recorded from a *previous* pass (the change
    /// coin's height, persisted the first time it was seen). Lets confirmation
    /// advance by depth even after the change coin has been spent, so a swept
    /// change output can never strand an already-accepted payout. `None` until
    /// acceptance has been observed once.
    pub recorded_accept_daa: Option<u64>,
}

/// Fold raw chain reads into a [`ConfirmationState`].
///
/// An accepting DAA score — observed live this pass (`change_block_daa_score`)
/// or durably recorded from an earlier pass (`recorded_accept_daa`) — is
/// authoritative: the tx is at least Accepted, and Confirmed once matured past
/// the depth. The recorded score wins even when the change coin is no longer
/// observable, so spending it cannot regress an accepted payout to Unknown.
/// Without any accepting score, mempool presence means Pending and absence is
/// deliberately Unknown (not Failed).
#[must_use]
pub const fn classify_confirmation(inputs: ConfirmationInputs) -> ConfirmationState {
    // Prefer the durably-recorded height; fall back to the live observation.
    let accept_daa = match inputs.recorded_accept_daa {
        Some(daa) => Some(daa),
        None => inputs.change_block_daa_score,
    };
    match accept_daa {
        Some(daa) => {
            if inputs.virtual_daa_score >= daa.saturating_add(KAS_PAYOUT_CONFIRMATION_DAA) {
                ConfirmationState::Confirmed
            } else {
                ConfirmationState::Accepted
            }
        }
        None if inputs.in_mempool => ConfirmationState::Pending,
        None => ConfirmationState::Unknown,
    }
}

/// Whether a treasury coin is spendable now, by coinbase status and maturity.
#[must_use]
pub const fn is_spendable(block_daa_score: u64, is_coinbase: bool, virtual_daa_score: u64) -> bool {
    let needed = if is_coinbase {
        COINBASE_MATURITY_DAA
    } else {
        NON_COINBASE_MATURITY_DAA
    };
    virtual_daa_score >= block_daa_score.saturating_add(needed)
}

#[cfg(test)]
mod tests {
    use super::{
        ConfirmationInputs, ConfirmationState, KAS_PAYOUT_CONFIRMATION_DAA, classify_confirmation,
        is_spendable,
    };

    fn inputs(virtual_daa: u64, in_mempool: bool, change: Option<u64>) -> ConfirmationInputs {
        ConfirmationInputs {
            virtual_daa_score: virtual_daa,
            in_mempool,
            change_block_daa_score: change,
            recorded_accept_daa: None,
        }
    }

    #[test]
    fn in_mempool_without_chain_signal_is_pending() {
        assert_eq!(
            classify_confirmation(inputs(1_000, true, None)),
            ConfirmationState::Pending
        );
    }

    #[test]
    fn absent_everywhere_is_unknown_not_failed() {
        assert_eq!(
            classify_confirmation(inputs(1_000, false, None)),
            ConfirmationState::Unknown
        );
    }

    #[test]
    fn on_chain_below_depth_is_accepted() {
        let v = 1_000;
        let change = v - 1; // only 1 DAA deep
        assert_eq!(
            classify_confirmation(inputs(v, false, Some(change))),
            ConfirmationState::Accepted
        );
    }

    #[test]
    fn on_chain_past_depth_is_confirmed() {
        let change = 1_000;
        let v = change + KAS_PAYOUT_CONFIRMATION_DAA;
        assert_eq!(
            classify_confirmation(inputs(v, false, Some(change))),
            ConfirmationState::Confirmed
        );
        // Chain signal wins even if still in mempool views.
        assert_eq!(
            classify_confirmation(inputs(v, true, Some(change))),
            ConfirmationState::Confirmed
        );
    }

    #[test]
    fn recorded_score_confirms_even_after_change_coin_is_spent() {
        // Change coin gone this pass and not in mempool — would be Unknown — but
        // a previously-recorded accepting height past depth still Confirms.
        let recorded = 1_000;
        let v = recorded + KAS_PAYOUT_CONFIRMATION_DAA;
        let i = ConfirmationInputs {
            virtual_daa_score: v,
            in_mempool: false,
            change_block_daa_score: None,
            recorded_accept_daa: Some(recorded),
        };
        assert_eq!(classify_confirmation(i), ConfirmationState::Confirmed);
    }

    #[test]
    fn recorded_score_below_depth_stays_accepted_not_unknown() {
        let recorded = 1_000;
        let i = ConfirmationInputs {
            virtual_daa_score: recorded + 1, // only 1 DAA past acceptance
            in_mempool: false,
            change_block_daa_score: None,
            recorded_accept_daa: Some(recorded),
        };
        assert_eq!(classify_confirmation(i), ConfirmationState::Accepted);
    }

    #[test]
    fn coinbase_needs_deeper_maturity_than_change() {
        // 50 deep: change (non-coinbase, needs 10) spendable; coinbase (1000) not.
        assert!(is_spendable(0, false, 50));
        assert!(!is_spendable(0, true, 50));
        // Still immature one short of consensus coinbase maturity.
        assert!(!is_spendable(0, true, 999));
        // Spendable exactly at the 1000-DAA coinbase-maturity depth.
        assert!(is_spendable(0, true, 1000));
    }
}
