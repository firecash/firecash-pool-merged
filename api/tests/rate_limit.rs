//! Rate-limit (429) integration test.
//!
//! The `tower_governor` `PeerIpKeyExtractor` needs real per-connection peer
//! IP info, which `oneshot` can't supply — so this test binds an ephemeral
//! port, serves via [`api::serve`], and hammers `/health` over the loopback
//! to prove the in-app limiter trips with a `429`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::time::Duration;

use api::{ApiConfig, AppState, ReadinessHandle};
use katpool_db::{PoolConfig, build_pool, migrate};
use testcontainers::ContainerAsync;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;

async fn fresh_pool() -> (sqlx::PgPool, ContainerAsync<Postgres>) {
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
        application_name: "katpool-api-ratelimit-test".to_owned(),
    };
    let pool = build_pool(&cfg).await.expect("pool");
    migrate::run(&pool).await.expect("migrate");
    (pool, container)
}

#[tokio::test]
async fn over_limit_requests_get_429() {
    let (pool, _container) = fresh_pool().await;

    // Tightest possible bucket: 1 token, 1/sec refill.
    let config = ApiConfig {
        rate_per_second: 1,
        rate_burst: 1,
        ..ApiConfig::default()
    };
    let state = AppState::new(pool, ReadinessHandle::new(), config);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = api::serve(listener, state).await;
    });

    // Give the server a moment to start accepting.
    tokio::time::sleep(Duration::from_millis(150)).await;

    let client = reqwest::Client::new();
    let url = format!("http://{addr}/health");

    let mut saw_ok = false;
    let mut saw_throttled = false;
    for _ in 0..15 {
        let status = client.get(&url).send().await.unwrap().status();
        if status.is_success() {
            saw_ok = true;
        }
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            saw_throttled = true;
        }
    }
    assert!(saw_ok, "at least one request should pass the limiter");
    assert!(saw_throttled, "a burst should trip the 429 rate limit");
}
