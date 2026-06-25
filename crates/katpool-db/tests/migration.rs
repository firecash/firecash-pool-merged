//! End-to-end migration test.
//!
//! Spins up a Postgres container via `testcontainers`, applies the
//! katpool-db migrations into it, and asserts the resulting schema
//! matches what ADR-0011 specifies — table set, enum types, indexes,
//! a representative sample of CHECK constraints (`reject bad data`).
//!
//! Running locally requires Docker. CI uses the `ubuntu-latest` runner
//! which has Docker pre-installed.
//!
//! Each `#[tokio::test]` spins up its own ephemeral container, so the
//! tests are hermetic and parallel-safe.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_arithmetic
)]

use std::time::Duration;

use katpool_db::{PoolConfig, build_pool, migrate};
use sqlx::Row;
use testcontainers::ContainerAsync;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;
use uuid::Uuid;

/// Spins up an ephemeral postgres, runs every migration, and returns
/// the live pool. The `_container` guard must stay alive for the
/// duration of the caller; dropping it tears the container down.
async fn fresh_pool() -> (sqlx::PgPool, ContainerAsync<Postgres>) {
    let container = Postgres::default()
        .start()
        .await
        .expect("start postgres container");
    let port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("postgres port");
    let url = format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres");

    let cfg = PoolConfig {
        url,
        min_connections: 1,
        max_connections: 4,
        acquire_timeout: Duration::from_secs(10),
        idle_timeout: Duration::from_secs(60),
        max_lifetime: Duration::from_secs(300),
        statement_timeout: Duration::from_secs(30),
        application_name: "katpool-db-migration-test".to_owned(),
    };

    let pool = build_pool(&cfg).await.expect("build pool");
    migrate::run(&pool).await.expect("apply migrations");
    (pool, container)
}

#[tokio::test]
async fn migrations_apply_cleanly() {
    let (pool, _ctr) = fresh_pool().await;

    // sqlx writes a row per applied migration to `_sqlx_migrations`.
    // We must have at least one applied.
    let applied: i64 = sqlx::query_scalar("SELECT count(*)::bigint FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .expect("count migrations");
    assert!(applied >= 1, "no migrations applied: {applied}");
}

#[tokio::test]
async fn every_documented_table_exists() {
    let (pool, _ctr) = fresh_pool().await;

    let expected = [
        "wallet",
        "worker",
        "connection_session",
        "share",
        "share_window",
        "block",
        "share_allocation",
        "payout_cycle",
        "payout",
        "nacho_rebate_accrual",
        "krc20_pending_transfer",
        "treasury_snapshot",
        "audit_log",
        "pool_meta",
    ];

    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT tablename::text FROM pg_tables WHERE schemaname = 'public' ORDER BY tablename",
    )
    .fetch_all(&pool)
    .await
    .expect("list tables");
    let actual: Vec<String> = rows.into_iter().map(|(n,)| n).collect();

    for table in expected {
        assert!(
            actual.iter().any(|t| t == table),
            "missing table `{table}` (actual: {actual:?})"
        );
    }
}

#[tokio::test]
async fn every_enum_type_exists() {
    let (pool, _ctr) = fresh_pool().await;

    let expected = [
        "block_status",
        "payout_kind",
        "payout_cycle_status",
        "payout_status",
        "krc20_transfer_status",
    ];

    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT typname::text FROM pg_type WHERE typtype = 'e' AND typname = ANY($1) ORDER BY typname",
    )
    .bind(&expected[..])
    .fetch_all(&pool)
    .await
    .expect("list enums");
    let actual: Vec<String> = rows.into_iter().map(|(n,)| n).collect();
    assert_eq!(
        actual.len(),
        expected.len(),
        "missing enum types; actual: {actual:?}"
    );
}

#[tokio::test]
async fn wallet_check_constraint_rejects_bad_prefix() {
    let (pool, _ctr) = fresh_pool().await;

    let err = sqlx::query("INSERT INTO wallet (address, network) VALUES ($1, 'mainnet')")
        .bind("not-a-kaspa-address")
        .execute(&pool)
        .await
        .expect_err("expected CHECK violation");

    let db_err = err.as_database_error().expect("db error");
    // 23514 = check_violation in SQLSTATE.
    assert_eq!(
        db_err.code().as_deref(),
        Some("23514"),
        "wrong sqlstate: {db_err:?}"
    );
}

#[tokio::test]
async fn wallet_check_constraint_accepts_valid_mainnet_address() {
    let (pool, _ctr) = fresh_pool().await;

    sqlx::query("INSERT INTO wallet (address, network) VALUES ($1, 'mainnet')")
        .bind("kaspa:qz4j8mu269z8llgcczmfukm9fan2fq822kzxu4cfukd5fqrhxpsv2zhs9jxnp")
        .execute(&pool)
        .await
        .expect("valid mainnet address must insert");
}

#[tokio::test]
async fn wallet_check_constraint_accepts_valid_testnet_address() {
    let (pool, _ctr) = fresh_pool().await;

    sqlx::query("INSERT INTO wallet (address, network) VALUES ($1, 'testnet-10')")
        .bind("kaspatest:qpaaslz6kn4untywu50v59zkxztwlkwsulna78d7g4rt7elgly2az5jv3fxwz")
        .execute(&pool)
        .await
        .expect("valid testnet address must insert");
}

#[tokio::test]
async fn worker_fk_cascades_on_wallet_delete() {
    let (pool, _ctr) = fresh_pool().await;

    let wallet_id: i64 = sqlx::query_scalar(
        "INSERT INTO wallet (address, network) VALUES ($1, 'mainnet') RETURNING id",
    )
    .bind("kaspa:qz4j8mu269z8llgcczmfukm9fan2fq822kzxu4cfukd5fqrhxpsv2zhs9jxnp")
    .fetch_one(&pool)
    .await
    .expect("insert wallet");

    sqlx::query("INSERT INTO worker (wallet_id, name) VALUES ($1, $2)")
        .bind(wallet_id)
        .bind("rig-01")
        .execute(&pool)
        .await
        .expect("insert worker");

    sqlx::query("DELETE FROM wallet WHERE id = $1")
        .bind(wallet_id)
        .execute(&pool)
        .await
        .expect("delete wallet cascades");

    let workers: i64 = sqlx::query_scalar("SELECT count(*)::bigint FROM worker")
        .fetch_one(&pool)
        .await
        .expect("count workers");
    assert_eq!(workers, 0, "worker rows should cascade on wallet delete");
}

#[tokio::test]
async fn share_allocation_balance_check_rejects_mismatch() {
    let (pool, _ctr) = fresh_pool().await;

    // Set up minimal valid context: wallet → worker → block.
    let wallet_id: i64 = sqlx::query_scalar(
        "INSERT INTO wallet (address, network) VALUES ($1, 'mainnet') RETURNING id",
    )
    .bind("kaspa:qz4j8mu269z8llgcczmfukm9fan2fq822kzxu4cfukd5fqrhxpsv2zhs9jxnp")
    .fetch_one(&pool)
    .await
    .expect("insert wallet");

    let worker_id: i64 = sqlx::query_scalar(
        "INSERT INTO worker (wallet_id, name) VALUES ($1, 'rig-01') RETURNING id",
    )
    .bind(wallet_id)
    .fetch_one(&pool)
    .await
    .expect("insert worker");

    // Allocation now anchors on a matured coinbase_reward, not a block.
    let reward_id: i64 = sqlx::query_scalar(
        "INSERT INTO coinbase_reward
            (outpoint_transaction_id, outpoint_index, amount_sompi, block_daa_score)
         VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(vec![1u8; 32])
    .bind(0_i32)
    .bind(100_i64)
    .bind(1_i64)
    .fetch_one(&pool)
    .await
    .expect("insert coinbase_reward");
    let _ = worker_id;

    // Bad allocation: gross != fee + accrual + net.
    let err = sqlx::query(
        "INSERT INTO share_allocation (coinbase_reward_id, wallet_id, weight, window_total,
            gross_share_sompi, pool_fee_sompi, nacho_accrual_sompi, net_payout_sompi,
            applied_topline_bps, applied_rebate_bps, applied_tier)
         VALUES ($1, $2, 100.0, 100.0, 100, 1, 1, 1, 75, 3300, 'standard')",
    )
    .bind(reward_id)
    .bind(wallet_id)
    .execute(&pool)
    .await
    .expect_err("expected balance CHECK violation");

    assert_eq!(
        err.as_database_error()
            .and_then(sqlx::error::DatabaseError::code)
            .as_deref(),
        Some("23514"),
        "wrong sqlstate: {err:?}"
    );
}

#[tokio::test]
async fn block_lifecycle_order_check_rejects_out_of_order_timestamps() {
    let (pool, _ctr) = fresh_pool().await;

    let wallet_id: i64 = sqlx::query_scalar(
        "INSERT INTO wallet (address, network) VALUES ($1, 'mainnet') RETURNING id",
    )
    .bind("kaspa:qz4j8mu269z8llgcczmfukm9fan2fq822kzxu4cfukd5fqrhxpsv2zhs9jxnp")
    .fetch_one(&pool)
    .await
    .expect("insert wallet");

    let worker_id: i64 = sqlx::query_scalar(
        "INSERT INTO worker (wallet_id, name) VALUES ($1, 'rig-01') RETURNING id",
    )
    .bind(wallet_id)
    .fetch_one(&pool)
    .await
    .expect("insert worker");

    // matured_at without confirmed_at must be rejected.
    let err = sqlx::query(
        "INSERT INTO block (hash, finder_wallet_id, finder_worker_id, daa_score, nonce,
            correlation_id, matured_at)
         VALUES ($1, $2, $3, 1, 0, $4, now())",
    )
    .bind(vec![2u8; 32])
    .bind(wallet_id)
    .bind(worker_id)
    .bind(Uuid::new_v4())
    .execute(&pool)
    .await
    .expect_err("expected lifecycle CHECK violation");

    assert_eq!(
        err.as_database_error()
            .and_then(sqlx::error::DatabaseError::code)
            .as_deref(),
        Some("23514"),
        "wrong sqlstate: {err:?}"
    );
}

#[tokio::test]
async fn payout_idempotency_per_cycle_and_wallet() {
    let (pool, _ctr) = fresh_pool().await;

    let wallet_id: i64 = sqlx::query_scalar(
        "INSERT INTO wallet (address, network) VALUES ($1, 'mainnet') RETURNING id",
    )
    .bind("kaspa:qz4j8mu269z8llgcczmfukm9fan2fq822kzxu4cfukd5fqrhxpsv2zhs9jxnp")
    .fetch_one(&pool)
    .await
    .expect("insert wallet");

    let cycle_id: i64 = sqlx::query_scalar(
        "INSERT INTO payout_cycle (kind, daa_start, daa_end, idempotency_key)
         VALUES ('kas', 100, 200, 'kas-100-200') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .expect("insert cycle");

    sqlx::query("INSERT INTO payout (cycle_id, wallet_id, amount_sompi) VALUES ($1, $2, 1000)")
        .bind(cycle_id)
        .bind(wallet_id)
        .execute(&pool)
        .await
        .expect("first payout");

    let err =
        sqlx::query("INSERT INTO payout (cycle_id, wallet_id, amount_sompi) VALUES ($1, $2, 2000)")
            .bind(cycle_id)
            .bind(wallet_id)
            .execute(&pool)
            .await
            .expect_err("expected unique-violation");

    // 23505 = unique_violation.
    assert_eq!(
        err.as_database_error()
            .and_then(sqlx::error::DatabaseError::code)
            .as_deref(),
        Some("23505"),
        "wrong sqlstate: {err:?}"
    );
}

#[tokio::test]
async fn pool_meta_has_bootstrap_row() {
    let (pool, _ctr) = fresh_pool().await;

    let row = sqlx::query("SELECT value FROM pool_meta WHERE key = 'schema_bootstrap_migration'")
        .fetch_one(&pool)
        .await
        .expect("fetch bootstrap row");
    let value: String = row.try_get("value").expect("read value");
    assert!(value.contains("bootstrap"), "value: {value}");
}

#[tokio::test]
async fn migrations_are_idempotent() {
    let (pool, _ctr) = fresh_pool().await;
    // Second invocation must be a no-op (sqlx's migrator checks the
    // _sqlx_migrations table before re-applying).
    migrate::run(&pool).await.expect("second run is a no-op");
    let applied: i64 = sqlx::query_scalar("SELECT count(*)::bigint FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .expect("count migrations");
    assert!(applied >= 1);
}
