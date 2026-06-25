//! Native Rust KRC-20 NACHO rebate engine.
//!
//! Implements the kasplex inscription envelope
//! (`<x-only pubkey> OP_CHECKSIG OP_FALSE OP_IF "kasplex" OP_0 <json>
//! OP_ENDIF`) via rusty-kaspa's [`kaspa_txscript::script_builder::ScriptBuilder`]
//! and runs the commit/reveal flow against the embedded kaspad.
//! Floor-price quotes are fetched via direct HTTPS to `api.kaspa.com` with
//! a circuit breaker; no Puppeteer, no headless browser, no Bun.
//!
//! # Status
//!
//! - **M5.1**: pure, deterministic inscription primitives
//!   ([`build_transfer_inscription`], [`commit_address`],
//!   [`reveal_signature_script`]) — byte-for-byte compatible with the live
//!   production transfer and the kasplex indexer (see ADR-0015).
//! - **M5.2**: KAS→NACHO payout conversion ([`nacho_base_units`]
//!   over an exact fixed-point [`FloorPrice`], no payout-time multiplier —
//!   the tier rebate is already in `accrued_sompi`) and the floor-price
//!   quote source ([`FloorPriceSource`] + [`CoinGeckoFloorPrice`], deriving
//!   KAS-per-NACHO from `CoinGecko` USD spot) guarded by a fail-closed
//!   [`CircuitBreaker`] (see ADR-0016).
//! - **M5.3**: the mass-aware commit/reveal planner
//!   ([`plan_commit_reveal`]) — sizes signature scripts to their signed
//!   length so the reveal's `transient_storage_mass` (driven by the
//!   redeem-script push) is evaluated accurately against `max_block_mass`.
//! - **M5.4a**: the commit/reveal [`sign`]er — standard signing for the
//!   commit, manual P2SH-redeem-path Schnorr signing for the reveal, each
//!   fully re-verified through the txscript engine, with deterministic txids
//!   for record-before-broadcast.
//! - **M5.4b**: the restart-safe [`execute`]or state machine
//!   (`pending → commit_submitted → reveal_submitted → completed`) — reusing
//!   the Phase 4 KAS [`KaspadClient`](payout_kas::KaspadClient) + confirmation
//!   policy, recording each txid before broadcast and refusing to broadcast a
//!   divergent commit on UTXO drift.
//! - **M5.5a**: the KRC-20 [`cycle`] state machine
//!   ([`plan_krc20_cycle`] / [`resume_or_plan_krc20_cycle`] /
//!   [`credit_completed_transfers`] / [`reconcile_krc20_cycle_status`]) —
//!   selects + converts eligible NACHO rebates into commit/reveal transfers,
//!   credits a confirmed reveal back to `nacho_rebate.paid_sompi` exactly
//!   once, and folds transfer statuses into the cycle status.
//! - **M5.5b (this milestone)**: the single-leader periodic [`engine`]
//!   ([`Krc20PayoutEngine`]) that drives one cycle per DAA window through
//!   plan → settle → credit → reconcile, guarded by a Postgres advisory lock
//!   (reusing the Phase 4 KAS engine shape + DAA windowing) and safe-by-default
//!   (dry-run records and broadcasts nothing).
//! - **M5.6**: dry-run rehearsal acceptance evidence archived under
//!   `payout-evidence/` (one-shot tool retired post-sign-off).

#![cfg_attr(not(test), warn(missing_docs))]

pub mod cycle;
pub mod engine;
pub mod execute;
pub mod inscription;
pub mod plan;
pub mod quote;
pub mod rebate;
pub mod sign;

pub use cycle::{
    CreditReport, DEFAULT_CYCLE_LIMIT, Krc20CycleError, Krc20CycleParams, Krc20CycleState,
    credit_completed_transfers, fail_krc20_transfer, plan_krc20_cycle,
    reconcile_krc20_cycle_status, resume_or_plan_krc20_cycle,
};
pub use engine::{
    Krc20EngineError, Krc20PayoutEngine, Krc20PayoutEngineConfig, Krc20TickOutcome, Krc20TickReport,
};
pub use execute::{
    Krc20ExecuteError, SettleReport, TransferStep, advance_transfer, settle_pending,
};
pub use inscription::{
    InscriptionError, KASPLEX_TAG, KRC20_PROTOCOL, Krc20Transfer, build_transfer_inscription,
    commit_address, commit_script_public_key, reveal_signature_script,
};
pub use plan::{
    CommitRevealConfig, DEFAULT_COMMIT_AMOUNT_SOMPI, Krc20FeePolicy, PlanError,
    PlannedCommitReveal, STANDARD_SIGNATURE_SCRIPT_LEN, plan_commit_reveal,
};
pub use quote::{
    BreakeredSource, CircuitBreaker, CircuitState, CoinGeckoFloorPrice, DEFAULT_HTTP_TIMEOUT,
    DEFAULT_KASPA_COIN_ID, DEFAULT_NACHO_COIN_ID, DEFAULT_QUOTE_BASE, DEFAULT_QUOTE_TICKER,
    FloorPriceSource, QuoteError, derive_floor_price, parse_simple_price_response,
};
pub use rebate::{
    DEFAULT_MIN_NACHO_BASE_UNITS, DEFAULT_MIN_PENDING_SOMPI, FloorPrice, RebateError, is_payable,
    nacho_base_units,
};
pub use sign::{
    COMMIT_P2SH_OUTPUT_INDEX, SignError, SignedCommit, SignedReveal, commit_txid, reveal_txid,
    sign_commit, sign_reveal,
};

/// Crate version constant.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
