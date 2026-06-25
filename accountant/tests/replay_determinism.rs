//! Replay-determinism test for the consumer (M1 + M4).

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_arithmetic
)]

use chrono::{TimeZone, Utc};
use katpool_db::{PoolConfig, build_pool, migrate};
use katpool_domain::{
    BlockHash, CorrelationId, DaaScore, PoolEvent, ShareDifficulty, ShareRejectReason,
    WalletAddress, WorkerName,
};
use testcontainers::ContainerAsync;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;
use uuid::Uuid;

use accountant::{ConsumerConfig, EventConsumer, assert_snapshots_equal, replay_all, snapshot};

const MINER_A: &str = "kaspa:qypczcz0lhyf3tfsuqj86e7qc8us7r8a53nhlr4u6x4kq38td0hsjycf7sya7zq";
const MINER_B: &str = "kaspa:qzncghl8re9h35hp6n5wyxtslhevj6462qkrkqzlfkrs2mpkfkc5xe9s3tga7";
const HASH_A: &str = "cc2b1da2c931f4164c03b2066cfb3178303567a161e8a393def62c91e824138a";
const HASH_B: &str = "9685f4347b9aa2e100bf489f7979a30746d90823d5bfb62309513b1e23ab2274";

struct Env {
    db: sqlx::PgPool,
    _ctr: ContainerAsync<Postgres>,
}

async fn fresh_db() -> Env {
    let container = Postgres::default().start().await.expect("postgres");
    let port = container.get_host_port_ipv4(5432).await.expect("port");
    let url = format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres");
    let db = build_pool(&PoolConfig {
        url,
        min_connections: 1,
        max_connections: 4,
        application_name: "replay".to_owned(),
        ..PoolConfig::production("placeholder".to_owned())
    })
    .await
    .expect("pool");
    migrate::run(&db).await.expect("migrate");
    Env {
        db,
        _ctr: container,
    }
}

const fn corr(i: u8) -> CorrelationId {
    let mut bytes = [0u8; 16];
    bytes[15] = i;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    CorrelationId::from_uuid(Uuid::from_bytes(bytes))
}

fn deterministic_stream() -> Vec<PoolEvent> {
    let ts = Utc.with_ymd_and_hms(2026, 5, 26, 0, 0, 0).unwrap();
    let wallet_a = WalletAddress::new(MINER_A.to_owned()).unwrap();
    let wallet_b = WalletAddress::new(MINER_B.to_owned()).unwrap();
    let worker_a = WorkerName::new("rig-01".to_owned()).unwrap();
    let worker_b = WorkerName::new("rig-02".to_owned()).unwrap();
    let hash_a = BlockHash::from_hex(HASH_A).unwrap();
    let hash_b = BlockHash::from_hex(HASH_B).unwrap();

    vec![
        PoolEvent::ShareCredited {
            wallet: wallet_a.clone(),
            worker: worker_a.clone(),
            difficulty: ShareDifficulty::new(1024.0).unwrap(),
            daa_score: DaaScore::new(1_000_000),
            ts,
            correlation_id: corr(1),
        },
        PoolEvent::ShareCredited {
            wallet: wallet_b.clone(),
            worker: worker_b.clone(),
            difficulty: ShareDifficulty::new(2048.0).unwrap(),
            daa_score: DaaScore::new(1_000_001),
            ts,
            correlation_id: corr(2),
        },
        PoolEvent::ShareRejected {
            wallet: wallet_a.clone(),
            worker: worker_a.clone(),
            reason: ShareRejectReason::Stale,
            ts,
            correlation_id: corr(3),
        },
        PoolEvent::BlockFound {
            wallet: wallet_a.clone(),
            worker: worker_a.clone(),
            hash: hash_a,
            daa_score: DaaScore::new(1_000_002),
            ts,
            correlation_id: corr(4),
        },
        PoolEvent::BlockAccepted {
            hash: hash_a,
            ts,
            correlation_id: corr(4),
        },
        PoolEvent::ShareRejected {
            wallet: wallet_b,
            worker: worker_b,
            reason: ShareRejectReason::LowDifficulty,
            ts,
            correlation_id: corr(5),
        },
        PoolEvent::BlockFound {
            wallet: wallet_a,
            worker: worker_a,
            hash: hash_b,
            daa_score: DaaScore::new(1_000_010),
            ts,
            correlation_id: corr(6),
        },
    ]
}

#[tokio::test]
async fn same_event_stream_produces_identical_db_state() {
    let stream = deterministic_stream();
    let env_a = fresh_db().await;
    let env_b = fresh_db().await;

    let consumer_a = EventConsumer::new(
        env_a.db.clone(),
        ConsumerConfig::new("a".to_owned(), "mainnet".to_owned()).unwrap(),
    );
    let consumer_b = EventConsumer::new(
        env_b.db.clone(),
        ConsumerConfig::new("b".to_owned(), "mainnet".to_owned()).unwrap(),
    );

    replay_all(&consumer_a, &stream).await;
    replay_all(&consumer_b, &stream).await;

    let snap_a = snapshot(&env_a.db).await.unwrap();
    let snap_b = snapshot(&env_b.db).await.unwrap();
    assert_snapshots_equal(&snap_a, &snap_b).unwrap();

    assert!(!snap_a.wallets.is_empty());
    assert!(!snap_a.shares.is_empty());
    assert!(!snap_a.blocks.is_empty());
    assert!(!snap_a.rejects.is_empty());
}

#[tokio::test]
async fn replaying_same_stream_into_same_db_is_idempotent_for_blocks() {
    let stream = deterministic_stream();
    let env = fresh_db().await;
    let consumer = EventConsumer::new(
        env.db.clone(),
        ConsumerConfig::new("x".to_owned(), "mainnet".to_owned()).unwrap(),
    );
    replay_all(&consumer, &stream).await;
    let snap1 = snapshot(&env.db).await.unwrap();
    replay_all(&consumer, &stream).await;
    let snap2 = snapshot(&env.db).await.unwrap();
    assert_eq!(
        snap1.blocks, snap2.blocks,
        "block rows must be idempotent on replay"
    );
    assert_eq!(snap1.wallets, snap2.wallets);
    assert_eq!(snap1.workers, snap2.workers);
}
