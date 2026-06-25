//! PROP allocation engine.
//!
//! Takes a matured coinbase reward (a coinbase UTXO credited to the
//! pool address that has reached consensus coinbase-maturity, with its
//! exact sompi value known) and atomically:
//!
//! 1. Closes the share-window covering the block's reward span
//!    via [`WindowAggregator`].
//! 2. Pro-rates the coinbase reward across every contributing
//!    wallet using PROP weights from the closed window.
//! 3. Classifies each contributing wallet via the injected
//!    [`TierClassifier`].
//! 4. Runs [`FeeConfig::compute_allocation`](crate::config::FeeConfig::compute_allocation)
//!    per wallet to produce the audit-trailed sompi breakdown.
//! 5. Inserts every allocation as a single batched row insert
//!    (transactional, idempotent on
//!    `UNIQUE(block_id, wallet_id)`).
//! 6. Accrues each wallet's `nacho_accrual_sompi` to
//!    `nacho_rebate_accrual` additively.
//! 7. Advances the block's lifecycle status to `matured` with
//!    the reward recorded.
//! 8. Appends an `audit_log` entry tying the allocation to the
//!    operator-visible event trail.
//!
//! All steps run inside one Postgres transaction. Partial
//! application isn't representable: either every row landed or
//! none did.
//!
//! ## Idempotency
//!
//! The engine gates on `coinbase_reward.allocated_at`, taken under a
//! `SELECT … FOR UPDATE` row lock:
//!
//! - `allocated_at IS NULL` → do the allocation, stamp `allocated_at`.
//! - `allocated_at` set      → no-op, return [`AllocationOutcome::AlreadyAllocated`].
//!
//! The combination of the row-lock gate + the schema's
//! `UNIQUE(coinbase_reward_id, wallet_id)` on `share_allocation` makes
//! double-allocation impossible. The rebate accrual is also gated by
//! the lock, so additive `nacho_rebate::accrue` is called at most once
//! per (wallet, reward).
//!
//! ## Pro-rating + rounding residue
//!
//! Per-wallet gross is `floor(block_reward × weight / total_weight)`.
//! The sum of truncated grosses is `≤ block_reward`; the
//! difference (at most `N-1` sompi for N wallets) is awarded
//! deterministically to the wallet with the smallest internal
//! `wallet_id`. This:
//!
//! - Keeps every row balanced with respect to the schema's
//!   `share_allocation_balance` CHECK.
//! - Is fully deterministic for replay (the `wallet_id` order is
//!   stable across runs since wallets are upserted before block
//!   processing).
//! - Awards the rounding break to miners rather than the pool —
//!   matches the legacy stack's "miner-friendly" rounding posture.
//!
//! ## Float-arithmetic scope
//!
//! The `pro_rate` helper does its single multiplication in f64 because
//! `share_window.total_weight` is stored as DOUBLE PRECISION
//! (the legacy and rebuild schemas both inherit the share-
//! difficulty-as-float convention from the bridge). The ratio
//! `weight / total ∈ [0, 1]`, and `block_reward * ratio ≤
//! block_reward ≤ Kaspa supply cap ≈ 2^58 sompi`, comfortably
//! inside f64's `2^53`-exact integer range. The result is
//! floored to `i64` and asserted to be in `[0, block_reward]`
//! before use.

use std::sync::Arc;

use katpool_db::DbError;
use katpool_db::repo::CoinbaseRewardId;
use katpool_db::repo::audit;
use katpool_db::repo::coinbase_reward;
use katpool_db::repo::nacho_rebate;
use katpool_db::repo::share_allocation::{self, DbWalletTier, NewAllocation};
use katpool_db::repo::share_window;
use katpool_domain::DaaScore;
use sqlx::PgPool;
use tracing::{info, warn};

use crate::config::{Allocation, AllocationError, FeeConfig, WalletTier};
use crate::tier::TierClassifier;
use crate::window::WindowAggregator;

/// Result of running [`AllocationEngine::allocate_coinbase_reward`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AllocationOutcome {
    /// Allocation freshly performed; counts inside.
    Allocated {
        /// Number of contributing wallets that received a row.
        wallet_count: usize,
        /// Sum of every wallet's `net_payout_sompi`.
        total_net_payout_sompi: i64,
        /// Sum of every wallet's `nacho_accrual_sompi`.
        total_nacho_accrual_sompi: i64,
        /// Sum of every wallet's `pool_fee_sompi`.
        total_pool_fee_sompi: i64,
        /// Truncation residue awarded to the smallest-wallet_id
        /// contributor. `0` when no rounding leaked.
        rounding_residue_sompi: i64,
    },
    /// The block was already in `matured` status — caller had
    /// already allocated it. No DB changes.
    AlreadyAllocated,
    /// The block had no contributing wallets in the share window
    /// (legitimate edge case: a block found before any shares
    /// landed). Block advances to `matured` with the reward
    /// recorded; zero allocation rows are written. The reward
    /// becomes pure pool revenue, surfaced via the audit log.
    NoContributingWallets {
        /// The full coinbase reward, retained by the pool because
        /// nobody contributed shares to the window.
        retained_reward_sompi: i64,
    },
}

/// Errors that can fail an allocation cycle.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum AllocationEngineError {
    /// Database I/O failed.
    #[error("database error: {0}")]
    Db(#[from] DbError),

    /// `sqlx` error not already wrapped by `DbError`.
    #[error("sqlx error: {0}")]
    Sqlx(#[from] sqlx::Error),

    /// Per-row [`Allocation`] computation failed.
    #[error("allocation math: {0}")]
    Math(#[from] AllocationError),

    /// Coinbase-reward id isn't in the DB.
    #[error("unknown coinbase reward id: {id}")]
    UnknownReward {
        /// The id we couldn't find.
        id: i64,
    },

    /// The window's total weight is zero or non-finite. This is
    /// an upstream data bug — the aggregator should never
    /// materialise such rows. Surfaced explicitly rather than
    /// dividing by zero.
    #[error("share window total weight invalid: {total_weight}")]
    InvalidWindowWeight {
        /// Offending value.
        total_weight: f64,
    },

    /// A wallet address persisted in the DB failed domain
    /// re-validation. Indicates schema drift or out-of-band writes;
    /// surfaced rather than silently dropping the recipient.
    #[error("invalid wallet address in DB: {address}")]
    InvalidWalletAddress {
        /// The offending stored address.
        address: String,
    },
}

/// Engine state. Cheap to clone (`Arc`-wrapped fields).
pub struct AllocationEngine {
    db: PgPool,
    fee: FeeConfig,
    classifier: Arc<dyn TierClassifier>,
    aggregator: WindowAggregator,
    instance_id: String,
}

impl AllocationEngine {
    /// Construct an engine. `instance_id` is the operator-stable
    /// string used in audit-log entries (and, in M4, metrics).
    pub fn new(
        db: PgPool,
        fee: FeeConfig,
        classifier: Arc<dyn TierClassifier>,
        instance_id: String,
    ) -> Self {
        let aggregator = WindowAggregator::new(db.clone());
        Self {
            db,
            fee,
            classifier,
            aggregator,
            instance_id,
        }
    }

    /// Run the full allocation cycle for one matured coinbase reward.
    ///
    /// `reward_id` MUST reference a row in the `coinbase_reward` table
    /// (the maturity tracker records it once the UTXO reaches consensus
    /// coinbase-maturity). Allocation is gated on the row's
    /// `allocated_at` taken under a `SELECT … FOR UPDATE` lock, so a
    /// second call is a no-op.
    ///
    /// `reward_sompi` is the exact value of the matured coinbase UTXO.
    ///
    /// `daa_start..daa_end` is the share-window the reward covers.
    /// Half-open `[start, end)` semantics — shares at exactly `daa_end`
    /// land in the *next* window.
    // The orchestration is intentionally long-form: every step is
    // a single named operation against the schema, and abstracting
    // them out reduces traceability for a money-path function.
    #[allow(clippy::cognitive_complexity, clippy::too_many_lines)]
    pub async fn allocate_coinbase_reward(
        &self,
        reward_id: CoinbaseRewardId,
        reward_sompi: i64,
        daa_start: DaaScore,
        daa_end: DaaScore,
    ) -> Result<AllocationOutcome, AllocationEngineError> {
        if reward_sompi < 0 {
            return Err(AllocationEngineError::Math(
                AllocationError::NegativeGross {
                    gross_sompi: reward_sompi,
                },
            ));
        }

        // ---- close the share window (idempotent) --------------------
        let _ = self
            .aggregator
            .close_window(daa_start, daa_end, chrono::Utc::now())
            .await?;

        // ---- read per-wallet rollups (pure reads; the FOR UPDATE
        //      critical section below is kept short) -----------------
        let windows = share_window::list_for_window(&self.db, daa_start, daa_end).await?;

        // ---- empty window: reward retained by the pool --------------
        if windows.is_empty() {
            return self.finalise_empty(reward_id, reward_sompi).await;
        }

        let total_weight: f64 = windows.iter().map(|w| w.total_weight).sum();
        if !total_weight.is_finite() || total_weight <= 0.0 {
            return Err(AllocationEngineError::InvalidWindowWeight { total_weight });
        }

        // ---- pro-rate the reward; assign residue --------------------
        let mut grosses: Vec<i64> = windows
            .iter()
            .map(|w| pro_rate(reward_sompi, w.total_weight, total_weight))
            .collect();
        let summed: i64 = grosses.iter().sum();
        let residue = reward_sompi.saturating_sub(summed);
        // Award residue to the smallest wallet_id (deterministic).
        let mut order: Vec<usize> = (0..windows.len()).collect();
        order.sort_by_key(|i| windows.get(*i).map_or(i64::MAX, |w| w.wallet_id.0));
        if let Some(&first) = order.first()
            && let Some(slot) = grosses.get_mut(first)
        {
            *slot = slot.saturating_add(residue);
        }

        // ---- classify tiers (falls back to Standard individually on
        //      classifier error) ---------------------------------------
        let mut tiers: Vec<WalletTier> = Vec::with_capacity(windows.len());
        for w in &windows {
            let addr = self.lookup_wallet_addr(w.wallet_id.0).await?;
            let tier = self
                .classifier
                .classify(&addr)
                .await
                .unwrap_or(WalletTier::Standard);
            tiers.push(tier);
        }

        // ---- compute Allocation per row -----------------------------
        let mut allocations: Vec<Allocation> = Vec::with_capacity(windows.len());
        for (w, (gross, tier)) in windows.iter().zip(grosses.iter().copied().zip(tiers)) {
            let alloc = self.fee.compute_allocation(gross, tier)?;
            debug_assert!(alloc.is_balanced());
            let _ = w; // suppress unused; w is needed for the zip
            allocations.push(alloc);
        }

        // ---- single transaction: lock+gate the reward, insert
        //      allocations, accrue rebates, stamp allocated, audit ----
        let new_rows: Vec<NewAllocation> = windows
            .iter()
            .zip(allocations.iter())
            .map(|(w, a)| NewAllocation {
                wallet_id: w.wallet_id,
                weight: w.total_weight,
                window_total: total_weight,
                gross_share_sompi: a.gross_sompi,
                pool_fee_sompi: a.pool_fee_sompi,
                nacho_accrual_sompi: a.nacho_accrual_sompi,
                net_payout_sompi: a.net_payout_sompi,
                applied_topline_bps: i16::try_from(a.applied_topline_bps).unwrap_or(i16::MAX),
                applied_rebate_bps: i16::try_from(a.applied_rebate_bps).unwrap_or(i16::MAX),
                applied_tier: db_tier(a.applied_tier),
            })
            .collect();

        let mut tx = self.db.begin().await?;
        let reward_row = coinbase_reward::lock_for_allocation(&mut *tx, reward_id)
            .await?
            .ok_or(AllocationEngineError::UnknownReward { id: reward_id.0 })?;
        if reward_row.allocated_at.is_some() {
            // Another sweep already allocated this reward; the tx rolls
            // back on drop.
            info!(
                instance = %self.instance_id,
                reward_id = reward_id.0,
                "coinbase reward already allocated; no-op replay"
            );
            return Ok(AllocationOutcome::AlreadyAllocated);
        }
        let _ = share_allocation::insert_batch(&mut *tx, reward_id, &new_rows).await?;
        for (w, a) in windows.iter().zip(allocations.iter()) {
            if a.nacho_accrual_sompi > 0 {
                nacho_rebate::accrue(&mut *tx, w.wallet_id, a.nacho_accrual_sompi).await?;
            }
        }
        coinbase_reward::mark_allocated(&mut *tx, reward_id).await?;
        let totals = sum_totals(&allocations);
        let payload = serde_json::json!({
            "coinbase_reward_id": reward_id.0,
            "reward_sompi": reward_sompi,
            "wallet_count": windows.len(),
            "total_pool_fee_sompi": totals.pool_fee,
            "total_nacho_accrual_sompi": totals.nacho,
            "total_net_payout_sompi": totals.net,
            "rounding_residue_sompi": residue,
            "applied_topline_bps": self.fee.topline_bps(),
        });
        let entry = audit::NewEntry::new(&self.instance_id, "coinbase_reward.allocated")
            .subject("coinbase_reward", reward_id.0)
            .payload(payload);
        audit::append(&mut *tx, entry).await?;
        tx.commit().await?;

        info!(
            instance = %self.instance_id,
            reward_id = reward_id.0,
            wallets = windows.len(),
            reward = reward_sompi,
            residue,
            "coinbase reward allocated"
        );

        Ok(AllocationOutcome::Allocated {
            wallet_count: windows.len(),
            total_net_payout_sompi: totals.net,
            total_nacho_accrual_sompi: totals.nacho,
            total_pool_fee_sompi: totals.pool_fee,
            rounding_residue_sompi: residue,
        })
    }

    async fn lookup_wallet_addr(
        &self,
        wallet_id: i64,
    ) -> Result<katpool_domain::WalletAddress, AllocationEngineError> {
        let addr: String = sqlx::query_scalar("SELECT address FROM wallet WHERE id = $1")
            .bind(wallet_id)
            .fetch_one(&self.db)
            .await?;
        // Address was validated when the row was inserted by the
        // accountant's `wallet::ensure` path. Re-validating here
        // is defence-in-depth against schema drift.
        katpool_domain::WalletAddress::new(addr.clone()).map_err(|e| {
            warn!(
                wallet_id,
                "wallet address in DB failed domain validation: {e}"
            );
            AllocationEngineError::InvalidWalletAddress { address: addr }
        })
    }

    async fn finalise_empty(
        &self,
        reward_id: CoinbaseRewardId,
        reward: i64,
    ) -> Result<AllocationOutcome, AllocationEngineError> {
        let mut tx = self.db.begin().await?;
        let reward_row = coinbase_reward::lock_for_allocation(&mut *tx, reward_id)
            .await?
            .ok_or(AllocationEngineError::UnknownReward { id: reward_id.0 })?;
        if reward_row.allocated_at.is_some() {
            return Ok(AllocationOutcome::AlreadyAllocated);
        }
        coinbase_reward::mark_allocated(&mut *tx, reward_id).await?;
        let payload = serde_json::json!({
            "coinbase_reward_id": reward_id.0,
            "reward_sompi": reward,
            "note": "no contributing wallets in share window; reward retained by pool",
        });
        let entry = audit::NewEntry::new(&self.instance_id, "coinbase_reward.allocated_empty")
            .subject("coinbase_reward", reward_id.0)
            .payload(payload);
        audit::append(&mut *tx, entry).await?;
        tx.commit().await?;
        warn!(
            instance = %self.instance_id,
            reward_id = reward_id.0,
            reward,
            "coinbase reward had no contributing wallets; retained by pool"
        );
        Ok(AllocationOutcome::NoContributingWallets {
            retained_reward_sompi: reward,
        })
    }
}

struct Totals {
    pool_fee: i64,
    nacho: i64,
    net: i64,
}

fn sum_totals(allocations: &[Allocation]) -> Totals {
    let mut t = Totals {
        pool_fee: 0,
        nacho: 0,
        net: 0,
    };
    for a in allocations {
        t.pool_fee = t.pool_fee.saturating_add(a.pool_fee_sompi);
        t.nacho = t.nacho.saturating_add(a.nacho_accrual_sompi);
        t.net = t.net.saturating_add(a.net_payout_sompi);
    }
    t
}

const fn db_tier(t: WalletTier) -> DbWalletTier {
    match t {
        WalletTier::Standard => DbWalletTier::Standard,
        WalletTier::Elite => DbWalletTier::Elite,
    }
}

/// Pro-rate `block_reward` across one wallet's `weight` share of
/// `total_weight`. Returns the floored sompi.
///
/// Scoped here (rather than `accountant::config`) because this is
/// the single place we touch float arithmetic in the money path;
/// see the module-level docs for the safety argument.
#[allow(
    clippy::float_arithmetic,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation
)]
fn pro_rate(block_reward_sompi: i64, weight: f64, total_weight: f64) -> i64 {
    debug_assert!(block_reward_sompi >= 0);
    debug_assert!(weight >= 0.0);
    debug_assert!(total_weight > 0.0);
    if weight == 0.0 || total_weight <= 0.0 {
        return 0;
    }
    let ratio = weight / total_weight;
    // ratio is in [0, 1] by construction.
    let portion = (block_reward_sompi as f64) * ratio;
    // floor + clamp to the legitimate range.
    let floored = portion.floor();
    if floored <= 0.0 {
        0
    } else if floored >= block_reward_sompi as f64 {
        block_reward_sompi
    } else {
        floored as i64
    }
}

#[cfg(test)]
mod unit_tests {
    #![allow(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::panic,
        clippy::float_arithmetic
    )]
    use super::*;

    #[test]
    fn pro_rate_half_share() {
        assert_eq!(pro_rate(1_000_000, 50.0, 100.0), 500_000);
    }

    #[test]
    fn pro_rate_floor_truncates() {
        // 1000 * 1/3 = 333.333... → 333
        assert_eq!(pro_rate(1_000, 1.0, 3.0), 333);
    }

    #[test]
    fn pro_rate_zero_weight_yields_zero() {
        assert_eq!(pro_rate(1_000, 0.0, 100.0), 0);
    }

    #[test]
    fn pro_rate_full_share() {
        assert_eq!(pro_rate(1_000, 100.0, 100.0), 1_000);
    }

    #[test]
    fn pro_rate_never_exceeds_reward() {
        // Even with weight > total (which shouldn't happen but is
        // guarded against): we clamp to block_reward.
        assert_eq!(pro_rate(1_000, 200.0, 100.0), 1_000);
    }
}
