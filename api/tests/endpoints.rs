//! End-to-end endpoint tests against a real Postgres (testcontainers).
//!
//! Builds the actual `api::app` router over a migrated, seeded database and
//! drives it via `tower::ServiceExt::oneshot` — no network, no mocks. Covers
//! status codes (200/400/404), readiness toggling, and response shape for
//! the DB-backed surface. The rate-limit (429) path is covered separately in
//! `tests/rate_limit.rs` (it needs real peer-IP connection info).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::float_arithmetic,
    clippy::indexing_slicing
)]

use std::time::Duration;

use api::{ApiConfig, AppState, ReadinessHandle};
use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::Value;
use tower::ServiceExt;

use katpool_db::repo::payout::PayoutKind;
use katpool_db::repo::share_allocation::{DbWalletTier, NewAllocation};
use katpool_db::repo::{coinbase_reward, payout, share, share_allocation, wallet, worker};
use katpool_db::{PoolConfig, build_pool, migrate};
use katpool_domain::{
    BlockHash, CorrelationId, DaaScore, ShareDifficulty, WalletAddress, WorkerName,
};
use testcontainers::ContainerAsync;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;

const KNOWN_ADDR: &str = "kaspa:qz4j8mu269z8llgcczmfukm9fan2fq822kzxu4cfukd5fqrhxpsv2zhs9jxnp";
const UNKNOWN_ADDR: &str = "kaspa:qrxd24c5w6pl2qa9k7q5e0lyepuu4r5t2f6awvxllk0a83qqfys9rhxpsv2zhs9j";

async fn seeded() -> (Router, ReadinessHandle, ContainerAsync<Postgres>) {
    let container = Postgres::default().start().await.expect("start postgres");
    let port = container.get_host_port_ipv4(5432).await.expect("port");
    let url = format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres");
    let cfg = PoolConfig {
        url,
        min_connections: 1,
        max_connections: 4,
        acquire_timeout: Duration::from_secs(10),
        idle_timeout: Duration::from_secs(60),
        max_lifetime: Duration::from_secs(300),
        statement_timeout: Duration::from_secs(30),
        application_name: "katpool-api-endpoints-test".to_owned(),
    };
    let pool = build_pool(&cfg).await.expect("pool");
    migrate::run(&pool).await.expect("migrate");

    // Seed one active miner: wallet + worker + shares + an Elite allocation +
    // a confirmed KAS payout.
    let addr = WalletAddress::new(KNOWN_ADDR).unwrap();
    let w = wallet::ensure(&pool, &addr, "mainnet").await.unwrap();
    let rig = worker::ensure(&pool, w.id, &WorkerName::new("rig-01").unwrap())
        .await
        .unwrap();
    for d in [1000.0, 2000.0] {
        share::insert_credited(
            &pool,
            w.id,
            rig.id,
            None,
            ShareDifficulty::new(d).unwrap(),
            DaaScore::new(100),
            CorrelationId::new_v4(),
        )
        .await
        .unwrap();
    }
    let (reward_id, _) = coinbase_reward::ensure(&pool, &[7u8; 32], 0, 5_000, 300)
        .await
        .unwrap();
    share_allocation::insert_batch(
        &pool,
        reward_id,
        &[NewAllocation {
            wallet_id: w.id,
            weight: 10.0,
            window_total: 10.0,
            gross_share_sompi: 1_000,
            pool_fee_sompi: 75,
            nacho_accrual_sompi: 25,
            net_payout_sompi: 900,
            applied_topline_bps: 75,
            applied_rebate_bps: 10_000,
            applied_tier: DbWalletTier::Elite,
        }],
    )
    .await
    .unwrap();
    let cycle = payout::create_cycle(
        &pool,
        PayoutKind::Kas,
        DaaScore::new(0),
        DaaScore::new(1_000),
    )
    .await
    .unwrap();
    let p = payout::insert_payout(&pool, cycle.id, w.id, 200)
        .await
        .unwrap();
    payout::mark_payout_submitted(&pool, p.id, BlockHash::from_bytes([9u8; 32]))
        .await
        .unwrap();
    payout::mark_payout_confirmed(&pool, p.id).await.unwrap();

    let readiness = ReadinessHandle::new();
    let state = AppState::new(pool, readiness.clone(), ApiConfig::default());
    (api::app(state), readiness, container)
}

async fn get(app: &Router, uri: &str) -> (StatusCode, Value) {
    let resp = app
        .clone()
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, value)
}

#[tokio::test]
async fn health_is_always_ok() {
    let (app, _r, _c) = seeded().await;
    let (status, body) = get(&app, "/health").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn ready_reflects_readiness_flags() {
    let (app, readiness, _c) = seeded().await;

    let (status, body) = get(&app, "/ready").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["ready"], false);

    readiness.set_db_reachable(true);
    readiness.set_kaspad_synced(true);
    let (status, body) = get(&app, "/ready").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["ready"], true);
    assert_eq!(body["db_reachable"], true);
    assert_eq!(body["kaspad_synced"], true);
}

#[tokio::test]
async fn started_reflects_latch() {
    let (app, readiness, _c) = seeded().await;
    let (status, _) = get(&app, "/started").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    readiness.mark_started();
    let (status, body) = get(&app, "/started").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["started"], true);
}

#[tokio::test]
async fn pool_stats_shape() {
    let (app, _r, _c) = seeded().await;
    let (status, body) = get(&app, "/api/v1/pool/stats").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["miners_active"], 1);
    assert_eq!(body["workers_active"], 1);
    assert_eq!(body["accepted_shares"], 2);
    // Confirmed KAS payout of 200 sompi shows in the totals as a string.
    assert_eq!(body["payouts"]["kas_confirmed"]["sompi"], "200");
    assert_eq!(body["payouts"]["confirmed_payouts"], 1);
}

#[tokio::test]
async fn balance_known_unknown_and_malformed() {
    let (app, _r, _c) = seeded().await;

    let (status, body) = get(&app, &format!("/api/v1/balance/{KNOWN_ADDR}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["kas"]["allocated"]["sompi"], "900");
    assert_eq!(body["kas"]["paid"]["sompi"], "200");
    assert_eq!(body["kas"]["payable"]["sompi"], "700");

    let (status, body) = get(&app, &format!("/api/v1/balance/{UNKNOWN_ADDR}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "not_found");

    let (status, body) = get(&app, "/api/v1/balance/not-a-valid-address").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "bad_request");
}

#[tokio::test]
async fn full_rebate_reports_elite() {
    let (app, _r, _c) = seeded().await;
    let (status, body) = get(&app, &format!("/api/v1/full_rebate/{KNOWN_ADDR}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["tier"], "elite");
    assert_eq!(body["full_rebate"], true);
}

#[tokio::test]
async fn miner_payouts_carry_kind() {
    let (app, _r, _c) = seeded().await;
    let (status, body) = get(&app, &format!("/api/v1/miners/{KNOWN_ADDR}/payouts")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["payouts"][0]["kind"], "kas");
    assert_eq!(body["payouts"][0]["amount"]["sompi"], "200");
}

#[tokio::test]
async fn blocks_endpoint_ok_when_empty() {
    let (app, _r, _c) = seeded().await;
    let (status, body) = get(&app, "/api/v1/pool/blocks").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["blocks"].as_array().unwrap().is_empty());
    assert_eq!(body["next_before"], Value::Null);
}

#[tokio::test]
async fn leaderboard_ranks_seeded_miner() {
    let (app, _r, _c) = seeded().await;
    let (status, body) = get(&app, "/api/v1/pool/leaderboard?window=86400").await;
    assert_eq!(status, StatusCode::OK);
    let entries = body["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["rank"], 1);
    assert_eq!(entries[0]["address"], KNOWN_ADDR);
    assert_eq!(entries[0]["accepted_shares"], 2);
    // The lone miner owns the whole window's weight.
    assert!((entries[0]["pool_share"].as_f64().unwrap() - 1.0).abs() < 1e-9);
}

#[tokio::test]
async fn active_miners_history_ok() {
    let (app, _r, _c) = seeded().await;
    let (status, body) = get(&app, "/api/v1/pool/miners/history").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["bucket"], "1h");
    assert!(body["points"].is_array());
}

#[tokio::test]
async fn firmware_breakdown_ok_when_empty() {
    // No connection_session rows are seeded, so the breakdown is empty
    // (the bridge populates this table at disconnect in production).
    let (app, _r, _c) = seeded().await;
    let (status, body) = get(&app, "/api/v1/pool/firmware").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["entries"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn pool_geo_ok_when_empty() {
    // No connection_session rows carry a country (geo resolution is
    // unconfigured in tests), so the breakdown is empty.
    let (app, _r, _c) = seeded().await;
    let (status, body) = get(&app, "/api/v1/pool/geo").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["entries"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn pool_active_sessions_ok() {
    // No open connection_session rows in the seed, so the live snapshot
    // reports zero — but the shape and status must be correct.
    let (app, _r, _c) = seeded().await;
    let (status, body) = get(&app, "/api/v1/pool/active-sessions").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["active_sessions"].as_i64(), Some(0));
    assert_eq!(body["active_workers"].as_i64(), Some(0));
}

#[tokio::test]
async fn pool_rejects_ok_when_empty() {
    // No share_reject rows are seeded, so the breakdown is empty and the
    // total is zero (the accountant persists rejects in production).
    let (app, _r, _c) = seeded().await;
    let (status, body) = get(&app, "/api/v1/pool/rejects").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 0);
    assert!(body["by_reason"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn hashrate_history_rejects_bad_bucket() {
    let (app, _r, _c) = seeded().await;
    let (status, body) = get(&app, "/api/v1/pool/hashrate/history?bucket=7m").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "bad_request");
}
