//! Advisory-lock integration tests against a real Postgres (testcontainers).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::time::Duration;

use katpool_db::{PoolConfig, build_pool, migrate};
use katpool_idempotency::{AdvisoryLock, advisory_key};
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
        application_name: "idempotency-lock-test".to_owned(),
    };
    let pool = build_pool(&cfg).await.expect("pool");
    migrate::run(&pool).await.expect("migrate");
    (pool, container)
}

#[tokio::test]
async fn advisory_lock_is_mutually_exclusive_and_releasable() {
    let (pool, _ctr) = fresh_pool().await;
    let key = advisory_key("test:kas-leader");

    let held = AdvisoryLock::try_acquire(&pool, key)
        .await
        .expect("acquire")
        .expect("lock is free");
    assert_eq!(held.key(), key);

    // A second acquisition of the same key (distinct session) is blocked.
    let contended = AdvisoryLock::try_acquire(&pool, key)
        .await
        .expect("try again");
    assert!(
        contended.is_none(),
        "second acquire must be blocked while held"
    );

    // A different key is independent and acquirable concurrently.
    let other = AdvisoryLock::try_acquire(&pool, advisory_key("test:other"))
        .await
        .expect("acquire other")
        .expect("other free");
    other.release().await.expect("release other");

    // After explicit release the key is free again.
    held.release().await.expect("release");
    let reacquired = AdvisoryLock::try_acquire(&pool, key)
        .await
        .expect("reacquire")
        .expect("free after release");
    reacquired.release().await.expect("release reacquired");
}
