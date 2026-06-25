//! Repository-layer integration tests.
//!
//! Each `#[tokio::test]` spins up its own ephemeral postgres
//! container, applies the migrations, and exercises one
//! repository contract. Hermetic, parallel-safe, and idempotent.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_arithmetic
)]

use std::time::Duration;

use katpool_db::repo::block::BlockStatus;
use katpool_db::repo::{audit, block, share, wallet, worker};
use katpool_db::{PoolConfig, build_pool, migrate};
use katpool_domain::{
    BlockHash, CorrelationId, DaaScore, ShareDifficulty, WalletAddress, WorkerName,
};
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
        application_name: "katpool-db-repo-test".to_owned(),
    };

    let pool = build_pool(&cfg).await.expect("pool");
    migrate::run(&pool).await.expect("migrate");
    (pool, container)
}

fn sample_wallet_addr() -> WalletAddress {
    // 62-char body — comfortably inside the schema's `[a-z0-9]{50,80}` range.
    WalletAddress::new("kaspa:qz4j8mu269z8llgcczmfukm9fan2fq822kzxu4cfukd5fqrhxpsv2zhs9jxnp")
        .expect("valid")
}

fn sample_worker_name() -> WorkerName {
    WorkerName::new("rig-01").expect("valid")
}

// ---- wallet ---------------------------------------------------------

#[tokio::test]
async fn wallet_ensure_creates_then_refreshes() {
    let (pool, _ctr) = fresh_pool().await;
    let addr = sample_wallet_addr();

    let w1 = wallet::ensure(&pool, &addr, "mainnet")
        .await
        .expect("create");
    let id1 = w1.id;
    let first_seen = w1.first_seen_at;
    let last_seen = w1.last_seen_at;

    // Sleep a beat so the refreshed last_seen_at is observably newer.
    tokio::time::sleep(Duration::from_millis(50)).await;

    let w2 = wallet::ensure(&pool, &addr, "mainnet")
        .await
        .expect("upsert");
    assert_eq!(w2.id, id1, "ensure must return the same id");
    assert_eq!(
        w2.first_seen_at, first_seen,
        "first_seen_at must not change"
    );
    assert!(
        w2.last_seen_at > last_seen,
        "last_seen_at must advance: {} -> {}",
        last_seen,
        w2.last_seen_at
    );
}

#[tokio::test]
async fn wallet_find_by_address_returns_none_when_missing() {
    let (pool, _ctr) = fresh_pool().await;
    let addr = sample_wallet_addr();
    let result = wallet::find_by_address(&pool, &addr).await.expect("query");
    assert!(result.is_none());
}

#[tokio::test]
async fn wallet_ensure_rejects_network_address_mismatch() {
    let (pool, _ctr) = fresh_pool().await;
    let mainnet_addr = sample_wallet_addr();
    // Insert a mainnet address with network 'testnet-10' — the
    // wallet_address_format CHECK should reject.
    let err = wallet::ensure(&pool, &mainnet_addr, "testnet-10")
        .await
        .expect_err("must reject");
    assert_eq!(
        err.sqlstate(),
        Some("23514"),
        "expected check_violation, got {err:?}"
    );
}

#[tokio::test]
async fn wallet_get_by_id_returns_not_found_on_missing() {
    let (pool, _ctr) = fresh_pool().await;
    let err = wallet::get_by_id(&pool, katpool_db::repo::WalletId(99_999_999))
        .await
        .expect_err("must be NotFound");
    assert!(err.is_not_found(), "expected NotFound, got {err:?}");
}

// ---- worker ---------------------------------------------------------

#[tokio::test]
async fn worker_ensure_is_idempotent_and_lists() {
    let (pool, _ctr) = fresh_pool().await;
    let w = wallet::ensure(&pool, &sample_wallet_addr(), "mainnet")
        .await
        .expect("wallet");

    let wn1 = WorkerName::new("rig-A").expect("valid");
    let wn2 = WorkerName::new("rig-B").expect("valid");

    let a1 = worker::ensure(&pool, w.id, &wn1).await.expect("a1");
    let a2 = worker::ensure(&pool, w.id, &wn1).await.expect("a2 upsert");
    assert_eq!(a1.id, a2.id, "same (wallet, name) must produce same id");

    let _ = worker::ensure(&pool, w.id, &wn2).await.expect("b");

    let listed = worker::list_for_wallet(&pool, w.id).await.expect("list");
    assert_eq!(listed.len(), 2);
    let names: Vec<String> = listed.iter().map(|w| w.name.clone()).collect();
    assert!(names.contains(&"rig-A".to_owned()));
    assert!(names.contains(&"rig-B".to_owned()));
}

#[tokio::test]
async fn worker_cascades_when_wallet_deleted() {
    let (pool, _ctr) = fresh_pool().await;
    let w = wallet::ensure(&pool, &sample_wallet_addr(), "mainnet")
        .await
        .expect("wallet");
    let _ = worker::ensure(&pool, w.id, &sample_worker_name())
        .await
        .expect("worker");

    sqlx::query("DELETE FROM wallet WHERE id = $1")
        .bind(w.id.0)
        .execute(&pool)
        .await
        .expect("delete wallet cascades");

    let workers = worker::list_for_wallet(&pool, w.id).await.expect("list");
    assert!(workers.is_empty());
}

// ---- share ----------------------------------------------------------

#[tokio::test]
async fn share_insert_and_window_aggregates_match() {
    let (pool, _ctr) = fresh_pool().await;
    let w = wallet::ensure(&pool, &sample_wallet_addr(), "mainnet")
        .await
        .expect("wallet");
    let wk = worker::ensure(&pool, w.id, &sample_worker_name())
        .await
        .expect("worker");

    // Insert a deterministic set of shares: difficulty 1.0 .. 5.0,
    // daa_scores 100..105. Total weight = 1+2+3+4+5 = 15.
    let mut last_id = None;
    for i in 0_u32..5 {
        let diff = ShareDifficulty::new(f64::from(i + 1)).expect("valid");
        let daa = DaaScore::new(u64::from(100_u32 + i));
        let cid = CorrelationId::new_v4();
        let id = share::insert_credited(&pool, w.id, wk.id, None, diff, daa, cid)
            .await
            .expect("insert share");
        last_id = Some(id);
    }
    assert!(last_id.is_some());

    let window_weight =
        share::sum_weight_for_window(&pool, w.id, DaaScore::new(100), DaaScore::new(200))
            .await
            .expect("sum");
    assert!((window_weight - 15.0).abs() < 1e-9, "got {window_weight}");

    let window_count = share::count_for_window(&pool, w.id, DaaScore::new(100), DaaScore::new(200))
        .await
        .expect("count");
    assert_eq!(window_count, 5);

    // Narrow window: only diff=1 (daa=100) and diff=2 (daa=101) should
    // be included (half-open [100, 102)).
    let narrow_weight =
        share::sum_weight_for_window(&pool, w.id, DaaScore::new(100), DaaScore::new(102))
            .await
            .expect("narrow sum");
    assert!((narrow_weight - 3.0).abs() < 1e-9, "got {narrow_weight}");

    let total = share::total_weight_for_window(&pool, DaaScore::new(0), DaaScore::new(1_000_000))
        .await
        .expect("total");
    assert!((total - 15.0).abs() < 1e-9, "got {total}");
}

#[tokio::test]
async fn share_window_aggregate_for_unknown_wallet_returns_zero() {
    let (pool, _ctr) = fresh_pool().await;
    let weight = share::sum_weight_for_window(
        &pool,
        katpool_db::repo::WalletId(1),
        DaaScore::new(0),
        DaaScore::new(100),
    )
    .await
    .expect("sum");
    assert!((weight - 0.0).abs() < f64::EPSILON);
}

// ---- block ----------------------------------------------------------

const fn sample_block_hash(byte: u8) -> BlockHash {
    BlockHash::from_bytes([byte; 32])
}

#[tokio::test]
async fn block_insert_then_find() {
    let (pool, _ctr) = fresh_pool().await;
    let w = wallet::ensure(&pool, &sample_wallet_addr(), "mainnet")
        .await
        .expect("wallet");
    let wk = worker::ensure(&pool, w.id, &sample_worker_name())
        .await
        .expect("worker");
    let hash = sample_block_hash(0x01);

    let id = block::insert(
        &pool,
        hash,
        w.id,
        wk.id,
        DaaScore::new(500),
        42,
        CorrelationId::new_v4(),
    )
    .await
    .expect("insert");

    let fetched = block::find_by_hash(&pool, hash)
        .await
        .expect("find")
        .expect("present");
    assert_eq!(fetched.id, id);
    assert_eq!(fetched.daa_score, 500);
    assert_eq!(fetched.status, BlockStatus::Found);
    assert!(fetched.submitted_at.is_none());
    assert!(fetched.confirmed_at.is_none());
    assert!(fetched.matured_at.is_none());
}

#[tokio::test]
async fn block_lifecycle_advance_in_order() {
    let (pool, _ctr) = fresh_pool().await;
    let w = wallet::ensure(&pool, &sample_wallet_addr(), "mainnet")
        .await
        .expect("wallet");
    let wk = worker::ensure(&pool, w.id, &sample_worker_name())
        .await
        .expect("worker");
    let hash = sample_block_hash(0x02);
    let _ = block::insert(
        &pool,
        hash,
        w.id,
        wk.id,
        DaaScore::new(1),
        0,
        CorrelationId::new_v4(),
    )
    .await
    .expect("insert");

    block::mark_submitted(&pool, hash).await.expect("submitted");
    let after_sub = block::find_by_hash(&pool, hash)
        .await
        .expect("find")
        .expect("present");
    assert_eq!(after_sub.status, BlockStatus::SubmittedToNode);
    assert!(after_sub.submitted_at.is_some());

    block::mark_confirmed_blue(&pool, hash, Some(1234))
        .await
        .expect("confirmed");
    let after_conf = block::find_by_hash(&pool, hash)
        .await
        .expect("find")
        .expect("present");
    assert_eq!(after_conf.status, BlockStatus::ConfirmedBlue);
    assert_eq!(after_conf.blue_score, Some(1234));

    block::mark_matured(&pool, hash, 5_000_000_000)
        .await
        .expect("matured");
    let after_mat = block::find_by_hash(&pool, hash)
        .await
        .expect("find")
        .expect("present");
    assert_eq!(after_mat.status, BlockStatus::Matured);
    assert_eq!(after_mat.miner_reward_sompi, Some(5_000_000_000));
}

#[tokio::test]
async fn block_mark_submitted_is_idempotent() {
    let (pool, _ctr) = fresh_pool().await;
    let w = wallet::ensure(&pool, &sample_wallet_addr(), "mainnet")
        .await
        .expect("wallet");
    let wk = worker::ensure(&pool, w.id, &sample_worker_name())
        .await
        .expect("worker");
    let hash = sample_block_hash(0x03);
    let _ = block::insert(
        &pool,
        hash,
        w.id,
        wk.id,
        DaaScore::new(1),
        0,
        CorrelationId::new_v4(),
    )
    .await
    .expect("insert");

    block::mark_submitted(&pool, hash).await.expect("first");
    block::mark_submitted(&pool, hash)
        .await
        .expect("second is a no-op");

    let block = block::find_by_hash(&pool, hash)
        .await
        .expect("find")
        .expect("present");
    assert_eq!(block.status, BlockStatus::SubmittedToNode);
}

#[tokio::test]
async fn block_list_by_status_orders_oldest_first() {
    let (pool, _ctr) = fresh_pool().await;
    let w = wallet::ensure(&pool, &sample_wallet_addr(), "mainnet")
        .await
        .expect("wallet");
    let wk = worker::ensure(&pool, w.id, &sample_worker_name())
        .await
        .expect("worker");
    for i in 1_u8..=3 {
        let _ = block::insert(
            &pool,
            sample_block_hash(i),
            w.id,
            wk.id,
            DaaScore::new(u64::from(i)),
            0,
            CorrelationId::new_v4(),
        )
        .await
        .expect("insert");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    let listed = block::list_by_status(&pool, &[BlockStatus::Found], 10)
        .await
        .expect("list");
    assert_eq!(listed.len(), 3);
    // Oldest-first (FIFO drain), so the first-inserted (hash byte 1) is
    // first and the last-inserted (hash byte 3) is last.
    assert_eq!(listed[0].hash[0], 1);
    assert_eq!(listed[2].hash[0], 3);
}

#[tokio::test]
async fn block_orphan_can_be_set_from_any_non_terminal_state() {
    let (pool, _ctr) = fresh_pool().await;
    let w = wallet::ensure(&pool, &sample_wallet_addr(), "mainnet")
        .await
        .expect("wallet");
    let wk = worker::ensure(&pool, w.id, &sample_worker_name())
        .await
        .expect("worker");
    let hash = sample_block_hash(0x04);
    let _ = block::insert(
        &pool,
        hash,
        w.id,
        wk.id,
        DaaScore::new(1),
        0,
        CorrelationId::new_v4(),
    )
    .await
    .expect("insert");
    block::mark_submitted(&pool, hash).await.expect("submitted");

    block::mark_orphaned(&pool, hash).await.expect("orphan");
    let after = block::find_by_hash(&pool, hash)
        .await
        .expect("find")
        .expect("present");
    assert_eq!(after.status, BlockStatus::Orphaned);
}

// ---- audit ----------------------------------------------------------

#[tokio::test]
async fn audit_append_and_query_by_subject() {
    let (pool, _ctr) = fresh_pool().await;
    let entry = audit::NewEntry::new("accountant", "payout.broadcast")
        .subject("payout", 7)
        .correlation_id(uuid::Uuid::new_v4())
        .payload(serde_json::json!({ "tx_hash": "deadbeef" }));

    let id = audit::append(&pool, entry.clone()).await.expect("append");
    assert!(id.0 > 0);
    let id2 = audit::append(&pool, entry)
        .await
        .expect("append again is fine");
    assert_ne!(id, id2, "each append must yield a fresh id");

    let entries = audit::list_for_subject(&pool, "payout", 7, 10)
        .await
        .expect("list");
    assert_eq!(entries.len(), 2);
    for e in &entries {
        assert_eq!(e.actor, "accountant");
        assert_eq!(e.action, "payout.broadcast");
        assert_eq!(e.subject_type.as_deref(), Some("payout"));
        assert_eq!(e.subject_id, Some(7));
        assert_eq!(e.payload["tx_hash"], "deadbeef");
    }
}

#[tokio::test]
async fn audit_minimal_entry_payload_is_empty_object() {
    let (pool, _ctr) = fresh_pool().await;
    let _ = audit::append(&pool, audit::NewEntry::new("system", "boot"))
        .await
        .expect("append");
    let entries = audit::list_for_subject(&pool, "payout", 1, 10)
        .await
        .expect("list");
    assert!(
        entries.is_empty(),
        "subject was not specified, must not match"
    );
}

// ---- transaction-spanning composition ------------------------------

#[tokio::test]
async fn wallet_worker_ensure_inside_a_transaction() {
    let (pool, _ctr) = fresh_pool().await;
    let mut tx = pool.begin().await.expect("begin");

    let w = wallet::ensure(&mut *tx, &sample_wallet_addr(), "mainnet")
        .await
        .expect("wallet inside tx");
    let _ = worker::ensure(&mut *tx, w.id, &sample_worker_name())
        .await
        .expect("worker inside tx");

    tx.commit().await.expect("commit");

    let after = wallet::find_by_address(&pool, &sample_wallet_addr())
        .await
        .expect("find")
        .expect("present");
    assert_eq!(after.id, w.id);
}

#[tokio::test]
async fn transaction_rollback_undoes_inserts() {
    let (pool, _ctr) = fresh_pool().await;
    let mut tx = pool.begin().await.expect("begin");
    let _ = wallet::ensure(&mut *tx, &sample_wallet_addr(), "mainnet")
        .await
        .expect("wallet inside tx");
    tx.rollback().await.expect("rollback");

    let after = wallet::find_by_address(&pool, &sample_wallet_addr())
        .await
        .expect("find");
    assert!(after.is_none(), "rolled-back insert must not be visible");
}
