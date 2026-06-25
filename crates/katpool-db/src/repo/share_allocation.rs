//! Share-allocation aggregate — per-wallet PROP allocation of a matured
//! coinbase reward.
//!
//! Inserted by the accountant once a coinbase UTXO credited to the pool
//! address reaches consensus coinbase-maturity and its exact reward is
//! known (see [`crate::repo::coinbase_reward`]). The schema's
//! `share_allocation_balance` CHECK constraint enforces the equation
//! `gross = pool_fee + nacho_accrual + net_payout` — every sompi
//! accounted for, no rounding leakage.

use chrono::{DateTime, Utc};
use sqlx::PgExecutor;

use crate::DbError;
use crate::repo::{CoinbaseRewardId, WalletId};

/// Postgres-enum-backed wallet tier.
///
/// Mirrors `accountant::WalletTier` byte-for-byte; defined here
/// (rather than re-imported from the accountant crate) so the
/// db crate keeps zero accountant-side dependencies and the
/// allocation insert code can take a tier value without forming
/// a circular dep. The accountant converts between the two
/// representations explicitly at the call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(type_name = "wallet_tier", rename_all = "snake_case")]
pub enum DbWalletTier {
    /// Default tier.
    Standard,
    /// NACHO NFT holder OR ≥ 100M NACHO holder.
    Elite,
}

impl DbWalletTier {
    /// Stable lowercase string suitable for log fields.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Elite => "elite",
        }
    }
}

/// One row of the `share_allocation` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ShareAllocation {
    /// Synthetic primary key.
    pub id: i64,
    /// FK to `coinbase_reward.id`.
    pub coinbase_reward_id: CoinbaseRewardId,
    /// FK to `wallet.id`.
    pub wallet_id: WalletId,
    /// This wallet's PROP weight in the share window.
    pub weight: f64,
    /// Total weight across every wallet in the share window.
    pub window_total: f64,
    /// `round(miner_reward_sompi * weight / window_total)`.
    pub gross_share_sompi: i64,
    /// Pool revenue from `gross` (net of the NACHO rebate).
    pub pool_fee_sompi: i64,
    /// NACHO rebate accrued from `gross`, denominated in sompi
    /// (converted to NACHO tokens at krc-20 payout-cycle time).
    pub nacho_accrual_sompi: i64,
    /// `gross - pool_fee - nacho_accrual`. Sums to the wallet's
    /// pending KAS payout for this block.
    pub net_payout_sompi: i64,
    /// Computation timestamp.
    pub computed_at: DateTime<Utc>,
    /// Topline-fee basis points applied at compute time. Audit
    /// trail — operator changes to `KATPOOL_FEE_TOPLINE_BPS` do
    /// not affect already-allocated rows.
    pub applied_topline_bps: i16,
    /// Rebate basis points applied at compute time.
    pub applied_rebate_bps: i16,
    /// Tier classification used at compute time.
    pub applied_tier: DbWalletTier,
}

/// A single allocation row to insert. Used by [`insert_batch`].
#[derive(Debug, Clone)]
pub struct NewAllocation {
    /// FK to `wallet.id`.
    pub wallet_id: WalletId,
    /// PROP weight.
    pub weight: f64,
    /// Total weight in the share window.
    pub window_total: f64,
    /// Gross sompi attributable.
    pub gross_share_sompi: i64,
    /// Pool revenue.
    pub pool_fee_sompi: i64,
    /// NACHO accrual in sompi.
    pub nacho_accrual_sompi: i64,
    /// Net KAS payout.
    pub net_payout_sompi: i64,
    /// Topline bps that produced this row.
    pub applied_topline_bps: i16,
    /// Rebate bps that produced this row.
    pub applied_rebate_bps: i16,
    /// Tier classification used.
    pub applied_tier: DbWalletTier,
}

impl NewAllocation {
    /// Sanity-check the balance equation before sending to the DB.
    /// The schema CHECK constraint enforces it server-side, but
    /// failing on the client side surfaces the error closer to the
    /// computation logic where the bug actually lives.
    #[must_use]
    pub const fn is_balanced(&self) -> bool {
        self.gross_share_sompi
            == self.pool_fee_sompi + self.nacho_accrual_sompi + self.net_payout_sompi
    }
}

/// Insert every allocation for a coinbase reward in one round-trip.
///
/// Uses postgres `UNNEST(...)` to flatten the per-wallet arrays into
/// rows — one SQL statement, transactional, idempotent w.r.t. the
/// `(coinbase_reward_id, wallet_id)` UNIQUE constraint. Caller is
/// responsible for filtering duplicates before calling.
pub async fn insert_batch<'e, E>(
    executor: E,
    coinbase_reward_id: CoinbaseRewardId,
    rows: &[NewAllocation],
) -> Result<u64, DbError>
where
    E: PgExecutor<'e>,
{
    if rows.is_empty() {
        return Ok(0);
    }
    for r in rows {
        if !r.is_balanced() {
            return Err(DbError::Config {
                message: format!(
                    "allocation for wallet {} is unbalanced: gross={} fee={} accrual={} net={}",
                    r.wallet_id.0,
                    r.gross_share_sompi,
                    r.pool_fee_sompi,
                    r.nacho_accrual_sompi,
                    r.net_payout_sompi
                ),
            });
        }
    }

    let wallet_ids: Vec<i64> = rows.iter().map(|r| r.wallet_id.0).collect();
    let weights: Vec<f64> = rows.iter().map(|r| r.weight).collect();
    let window_totals: Vec<f64> = rows.iter().map(|r| r.window_total).collect();
    let gross: Vec<i64> = rows.iter().map(|r| r.gross_share_sompi).collect();
    let fee: Vec<i64> = rows.iter().map(|r| r.pool_fee_sompi).collect();
    let accrual: Vec<i64> = rows.iter().map(|r| r.nacho_accrual_sompi).collect();
    let net: Vec<i64> = rows.iter().map(|r| r.net_payout_sompi).collect();
    let topline_bps: Vec<i16> = rows.iter().map(|r| r.applied_topline_bps).collect();
    let rebate_bps: Vec<i16> = rows.iter().map(|r| r.applied_rebate_bps).collect();
    let tiers: Vec<DbWalletTier> = rows.iter().map(|r| r.applied_tier).collect();

    let result = sqlx::query(
        "INSERT INTO share_allocation
            (coinbase_reward_id, wallet_id, weight, window_total,
             gross_share_sompi, pool_fee_sompi, nacho_accrual_sompi, net_payout_sompi,
             applied_topline_bps, applied_rebate_bps, applied_tier)
         SELECT $1, *
           FROM UNNEST($2::bigint[], $3::float8[], $4::float8[],
                       $5::bigint[], $6::bigint[], $7::bigint[], $8::bigint[],
                       $9::smallint[], $10::smallint[], $11::wallet_tier[])",
    )
    .bind(coinbase_reward_id.0)
    .bind(&wallet_ids)
    .bind(&weights)
    .bind(&window_totals)
    .bind(&gross)
    .bind(&fee)
    .bind(&accrual)
    .bind(&net)
    .bind(&topline_bps)
    .bind(&rebate_bps)
    .bind(&tiers)
    .execute(executor)
    .await?;

    Ok(result.rows_affected())
}

/// Every allocation for a coinbase reward.
pub async fn list_for_reward<'e, E: PgExecutor<'e>>(
    executor: E,
    coinbase_reward_id: CoinbaseRewardId,
) -> Result<Vec<ShareAllocation>, DbError> {
    sqlx::query_as::<_, ShareAllocation>(
        "SELECT id, coinbase_reward_id, wallet_id, weight, window_total,
                gross_share_sompi, pool_fee_sompi, nacho_accrual_sompi, net_payout_sompi,
                computed_at, applied_topline_bps, applied_rebate_bps, applied_tier
           FROM share_allocation
          WHERE coinbase_reward_id = $1
          ORDER BY weight DESC",
    )
    .bind(coinbase_reward_id.0)
    .fetch_all(executor)
    .await
    .map_err(DbError::from)
}

/// Recent allocations for one wallet.
pub async fn list_for_wallet<'e, E: PgExecutor<'e>>(
    executor: E,
    wallet_id: WalletId,
    limit: i64,
) -> Result<Vec<ShareAllocation>, DbError> {
    sqlx::query_as::<_, ShareAllocation>(
        "SELECT id, coinbase_reward_id, wallet_id, weight, window_total,
                gross_share_sompi, pool_fee_sompi, nacho_accrual_sompi, net_payout_sompi,
                computed_at, applied_topline_bps, applied_rebate_bps, applied_tier
           FROM share_allocation
          WHERE wallet_id = $1
          ORDER BY computed_at DESC
          LIMIT $2",
    )
    .bind(wallet_id.0)
    .bind(limit)
    .fetch_all(executor)
    .await
    .map_err(DbError::from)
}

/// The tier applied to this wallet's most-recent allocation, or `None`
/// if the wallet has never been allocated.
///
/// This is the ground truth behind the Phase 6 `/full_rebate/:address`
/// endpoint: it reports the tier that was actually applied to the
/// wallet's money (the persisted `applied_tier`), not a live classifier
/// opinion — deterministic and honest on every network, including tn10
/// where NACHO does not exist (ADR-0021).
pub async fn latest_applied_tier_for_wallet<'e, E: PgExecutor<'e>>(
    executor: E,
    wallet_id: WalletId,
) -> Result<Option<DbWalletTier>, DbError> {
    sqlx::query_scalar::<_, DbWalletTier>(
        "SELECT applied_tier
           FROM share_allocation
          WHERE wallet_id = $1
          ORDER BY computed_at DESC, id DESC
          LIMIT 1",
    )
    .bind(wallet_id.0)
    .fetch_optional(executor)
    .await
    .map_err(DbError::from)
}

/// Total `net_payout_sompi` owed to a wallet across all allocations.
/// The Phase 3 accountant uses this as the planned-payout balance.
pub async fn pending_balance_for_wallet<'e, E: PgExecutor<'e>>(
    executor: E,
    wallet_id: WalletId,
) -> Result<i64, DbError> {
    let total: Option<i64> = sqlx::query_scalar(
        "SELECT sum(net_payout_sompi)::bigint
           FROM share_allocation
          WHERE wallet_id = $1",
    )
    .bind(wallet_id.0)
    .fetch_one(executor)
    .await?;
    Ok(total.unwrap_or(0))
}
