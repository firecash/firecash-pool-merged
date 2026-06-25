//! Per-wallet read handlers, served through the (shorter-TTL) wallet cache.
//!
//! Every handler resolves the path `:address` through the domain newtype
//! and `wallet::find_by_address`; an unknown (but well-formed) address is a
//! `404`, distinct from a `400` for a malformed one.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, Query, State};
use serde_json::Value;

use katpool_db::repo::share_allocation::DbWalletTier;
use katpool_db::repo::wallet::Wallet;
use katpool_db::repo::{nacho_rebate, payout, share_reject, share_stats, wallet, worker};
use katpool_domain::WalletAddress;

use crate::error::ApiError;
use crate::handlers::pool::{bucket_token, next_cursor};
use crate::handlers::{cached_json, resolve_window, to_value};
use crate::models::{
    BalanceResponse, FullRebateResponse, HashrateHistory, HashratePointView, KasBalanceView,
    MinerPayoutView, MinerPayoutsPage, MinerProfile, NachoRebateView, RejectReasonCount,
    RejectsResponse, WorkerView, WorkersResponse,
};
use crate::money::KasAmount;
use crate::params::{self, PageParams, RangeParams, WindowParams};
use crate::state::AppState;

/// Resolve a path address to a known wallet, or the appropriate 4xx.
async fn require_wallet(state: &AppState, addr: &WalletAddress) -> Result<Wallet, ApiError> {
    wallet::find_by_address(&state.pool, addr)
        .await?
        .ok_or(ApiError::NotFound)
}

/// Build a KAS balance view for a wallet.
async fn kas_balance(
    state: &AppState,
    wallet_id: katpool_db::repo::WalletId,
) -> Result<KasBalanceView, ApiError> {
    let bal = payout::kas_payable_for_wallet(&state.pool, wallet_id).await?;
    Ok(KasBalanceView {
        allocated: KasAmount::from_sompi(bal.allocated_sompi),
        paid: KasAmount::from_sompi(bal.confirmed_paid_sompi),
        payable: KasAmount::from_sompi(bal.payable_sompi),
    })
}

/// Build a NACHO rebate view for a wallet.
async fn nacho_view(
    state: &AppState,
    wallet_id: katpool_db::repo::WalletId,
) -> Result<NachoRebateView, ApiError> {
    let view = nacho_rebate::get(&state.pool, wallet_id)
        .await?
        .map_or_else(NachoRebateView::zero, |r| NachoRebateView {
            accrued: KasAmount::from_sompi(r.accrued_sompi),
            paid: KasAmount::from_sompi(r.paid_sompi),
            pending: KasAmount::from_sompi(r.pending_sompi()),
        });
    Ok(view)
}

/// `GET /api/v1/balance/:address`.
pub async fn balance(
    State(state): State<AppState>,
    Path(address): Path<String>,
) -> Result<Json<Arc<Value>>, ApiError> {
    let addr = params::parse_address(&address)?;
    let key = format!("balance/{}", addr.as_str());
    let cache = state.wallet_cache.clone();
    cached_json(&cache, key, async move {
        let wallet = require_wallet(&state, &addr).await?;
        let resp = BalanceResponse {
            address: wallet.address.clone(),
            network: wallet.network.clone(),
            kas: kas_balance(&state, wallet.id).await?,
            nacho_rebate: nacho_view(&state, wallet.id).await?,
        };
        to_value(&resp)
    })
    .await
}

/// `GET /api/v1/miners/:address` — profile snapshot.
pub async fn profile(
    State(state): State<AppState>,
    Path(address): Path<String>,
    Query(window_params): Query<WindowParams>,
) -> Result<Json<Arc<Value>>, ApiError> {
    let addr = params::parse_address(&address)?;
    let window = params::window(&window_params)?;
    let key = format!("miners/{}?w={}", addr.as_str(), window.as_secs());
    let cache = state.wallet_cache.clone();
    cached_json(&cache, key, async move {
        let wallet = require_wallet(&state, &addr).await?;
        let w = resolve_window(window);
        let summary =
            share_stats::accepted_and_rejected_for_wallet(&state.pool, wallet.id, w.since).await?;
        let hashrate_hs =
            share_stats::hashrate_estimate_for_wallet(&state.pool, wallet.id, w.since, w.until)
                .await?;
        let workers = worker::list_for_wallet(&state.pool, wallet.id).await?;
        let resp = MinerProfile {
            address: wallet.address.clone(),
            network: wallet.network.clone(),
            first_seen_at: wallet.first_seen_at,
            last_seen_at: wallet.last_seen_at,
            window_secs: w.secs,
            accepted_shares: summary.accepted_count,
            rejected_shares: summary.rejected_count,
            hashrate_hs,
            workers_count: workers.len(),
            kas: kas_balance(&state, wallet.id).await?,
            nacho_rebate: nacho_view(&state, wallet.id).await?,
        };
        to_value(&resp)
    })
    .await
}

/// `GET /api/v1/miners/:address/workers` — per-worker breakdown.
pub async fn workers(
    State(state): State<AppState>,
    Path(address): Path<String>,
    Query(window_params): Query<WindowParams>,
) -> Result<Json<Arc<Value>>, ApiError> {
    let addr = params::parse_address(&address)?;
    let window = params::window(&window_params)?;
    let key = format!("miners/{}/workers?w={}", addr.as_str(), window.as_secs());
    let cache = state.wallet_cache.clone();
    cached_json(&cache, key, async move {
        let wallet = require_wallet(&state, &addr).await?;
        let w = resolve_window(window);
        let rows = worker::list_for_wallet(&state.pool, wallet.id).await?;
        let mut views = Vec::with_capacity(rows.len());
        for worker_row in rows {
            let accepted =
                share_stats::accepted_for_worker(&state.pool, worker_row.id, w.since).await?;
            let hashrate_hs = share_stats::hashrate_estimate_for_worker(
                &state.pool,
                worker_row.id,
                w.since,
                w.until,
            )
            .await?;
            views.push(WorkerView {
                name: worker_row.name,
                first_seen_at: worker_row.first_seen_at,
                last_seen_at: worker_row.last_seen_at,
                accepted_shares: accepted.share_count,
                hashrate_hs,
            });
        }
        to_value(&WorkersResponse {
            address: wallet.address.clone(),
            window_secs: w.secs,
            workers: views,
        })
    })
    .await
}

/// `GET /api/v1/miners/:address/hashrate/history` — per-wallet series.
pub async fn hashrate_history(
    State(state): State<AppState>,
    Path(address): Path<String>,
    Query(range_params): Query<RangeParams>,
) -> Result<Json<Arc<Value>>, ApiError> {
    let addr = params::parse_address(&address)?;
    let range = params::range(&range_params)?;
    let key = format!(
        "miners/{}/hashrate/history?from={}&to={}&b={}",
        addr.as_str(),
        range.from.timestamp(),
        range.until.timestamp(),
        range.bucket.seconds()
    );
    let cache = state.wallet_cache.clone();
    cached_json(&cache, key, async move {
        let wallet = require_wallet(&state, &addr).await?;
        let points = share_stats::hashrate_series_for_wallet(
            &state.pool,
            wallet.id,
            range.from,
            range.until,
            range.bucket.seconds(),
        )
        .await?;
        to_value(&HashrateHistory {
            from: range.from,
            to: range.until,
            bucket: bucket_token(range.bucket),
            points: points
                .into_iter()
                .map(|p| HashratePointView {
                    bucket_start: p.bucket_start,
                    hashrate_hs: p.hashrate,
                    partial: p.is_partial,
                })
                .collect(),
        })
    })
    .await
}

/// `GET /api/v1/miners/:address/payouts` — per-wallet payout history.
pub async fn payouts(
    State(state): State<AppState>,
    Path(address): Path<String>,
    Query(page_params): Query<PageParams>,
) -> Result<Json<Arc<Value>>, ApiError> {
    let addr = params::parse_address(&address)?;
    let page = params::page(&page_params)?;
    let key = format!(
        "miners/{}/payouts?l={}&before={:?}",
        addr.as_str(),
        page.limit,
        page.before_id
    );
    let cache = state.wallet_cache.clone();
    cached_json(&cache, key, async move {
        let wallet = require_wallet(&state, &addr).await?;
        let rows =
            payout::list_for_wallet_detailed(&state.pool, wallet.id, page.limit, page.before_id)
                .await?;
        let next_before = next_cursor(rows.len(), page.limit, rows.last().map(|p| p.id));
        let payouts = rows.iter().map(MinerPayoutView::from).collect();
        to_value(&MinerPayoutsPage {
            address: wallet.address.clone(),
            payouts,
            next_before,
        })
    })
    .await
}

/// `GET /api/v1/miners/:address/rejects` — per-reason reject counts.
pub async fn rejects(
    State(state): State<AppState>,
    Path(address): Path<String>,
    Query(window_params): Query<WindowParams>,
) -> Result<Json<Arc<Value>>, ApiError> {
    let addr = params::parse_address(&address)?;
    let window = params::window(&window_params)?;
    let key = format!("miners/{}/rejects?w={}", addr.as_str(), window.as_secs());
    let cache = state.wallet_cache.clone();
    cached_json(&cache, key, async move {
        let wallet = require_wallet(&state, &addr).await?;
        let w = resolve_window(window);
        let rows =
            share_reject::count_by_reason_for_wallet(&state.pool, wallet.id, w.since).await?;
        let total: i64 = rows.iter().map(|(_, count)| *count).sum();
        let by_reason = rows
            .into_iter()
            .map(|(reason, count)| RejectReasonCount::from_row(reason, count))
            .collect();
        to_value(&RejectsResponse {
            address: wallet.address.clone(),
            window_secs: w.secs,
            total,
            by_reason,
        })
    })
    .await
}

/// `GET /api/v1/full_rebate/:address` — persisted tier / full-rebate status.
pub async fn full_rebate(
    State(state): State<AppState>,
    Path(address): Path<String>,
) -> Result<Json<Arc<Value>>, ApiError> {
    let addr = params::parse_address(&address)?;
    let key = format!("full_rebate/{}", addr.as_str());
    let cache = state.wallet_cache.clone();
    cached_json(&cache, key, async move {
        let wallet = require_wallet(&state, &addr).await?;
        let tier = share_allocation_tier(&state, wallet.id).await?;
        let resp = FullRebateResponse {
            address: wallet.address.clone(),
            tier: tier.map(DbWalletTier::as_str),
            full_rebate: tier == Some(DbWalletTier::Elite),
        };
        to_value(&resp)
    })
    .await
}

/// Thin wrapper to keep the import surface tidy.
async fn share_allocation_tier(
    state: &AppState,
    wallet_id: katpool_db::repo::WalletId,
) -> Result<Option<DbWalletTier>, ApiError> {
    katpool_db::repo::share_allocation::latest_applied_tier_for_wallet(&state.pool, wallet_id)
        .await
        .map_err(ApiError::from)
}
