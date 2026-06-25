//! Treasury UTXO consolidation ("Maintain" layer, `docs/kips.md` §5.3).
//!
//! A single-leader periodic loop that compounds the treasury's many small
//! spendable coins into a few large ones via mass-valid N→1 self-sends,
//! keeping the spendable UTXO count under a configurable ceiling. This both
//! unblocks large single-recipient KAS payouts (which cannot be sourced from
//! thousands of ~3.9-KAS coinbase fragments within the per-transaction mass
//! limit) and prevents the fragmentation from recurring.
//!
//! It shares the treasury-spend advisory lock
//! ([`katpool_idempotency::TREASURY_SPEND_LOCK_NAMESPACE`]) with the KAS and
//! KRC-20 payout engines, so at most one treasury spender acts at a time and
//! two engines can never select the same UTXO. Consolidation is naturally
//! idempotent against live chain state: broadcast inputs vanish from the next
//! snapshot and the merged output reappears once it matures, so no DB cycle
//! or per-row bookkeeping is needed (unlike payouts, which must never re-pay a
//! recipient).

mod engine;

pub use engine::{
    ConsolidationEngine, ConsolidationEngineConfig, ConsolidationError, ConsolidationTickOutcome,
    ConsolidationTickReport,
};
