//! Shared testcontainer harness for accountant integration tests.

#![allow(
    dead_code,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_arithmetic
)]

use chrono::Utc;
use katpool_db::{PoolConfig, build_pool, migrate};
use katpool_domain::{
    BlockHash, CorrelationId, DaaScore, PoolEvent, ShareDifficulty, WalletAddress, WorkerName,
};
use testcontainers::ContainerAsync;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;

pub const MINER_A: &str = "kaspa:qypczcz0lhyf3tfsuqj86e7qc8us7r8a53nhlr4u6x4kq38td0hsjycf7sya7zq";
pub const MINER_B: &str = "kaspa:qzncghl8re9h35hp6n5wyxtslhevj6462qkrkqzlfkrs2mpkfkc5xe9s3tga7";

pub const HASH_A: &str = "cc2b1da2c931f4164c03b2066cfb3178303567a161e8a393def62c91e824138a";
pub const HASH_B: &str = "9685f4347b9aa2e100bf489f7979a30746d90823d5bfb62309513b1e23ab2274";

pub struct Env {
    pub db: sqlx::PgPool,
    _ctr: ContainerAsync<Postgres>,
}

pub async fn setup() -> Env {
    let container = Postgres::default().start().await.expect("start postgres");
    let port = container.get_host_port_ipv4(5432).await.expect("port");
    let url = format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres");
    let db = build_pool(&PoolConfig {
        url,
        min_connections: 1,
        max_connections: 4,
        application_name: "accountant-tests".to_owned(),
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

pub fn wallet(addr: &str) -> WalletAddress {
    WalletAddress::new(addr.to_owned()).expect("valid wallet")
}

pub fn worker(name: &str) -> WorkerName {
    WorkerName::new(name.to_owned()).expect("valid worker")
}

pub fn hash(hex: &str) -> BlockHash {
    BlockHash::from_hex(hex).expect("valid hash")
}

pub fn share_credited(
    wallet_addr: &str,
    worker_name: &str,
    difficulty: f64,
    daa: u64,
) -> PoolEvent {
    PoolEvent::ShareCredited {
        wallet: wallet(wallet_addr),
        worker: worker(worker_name),
        difficulty: ShareDifficulty::new(difficulty).expect("valid"),
        daa_score: DaaScore::new(daa),
        ts: Utc::now(),
        correlation_id: CorrelationId::new_v4(),
    }
}

pub fn block_found(
    wallet_addr: &str,
    worker_name: &str,
    hash_hex: &str,
    daa: u64,
    correlation_id: CorrelationId,
) -> PoolEvent {
    PoolEvent::BlockFound {
        wallet: wallet(wallet_addr),
        worker: worker(worker_name),
        hash: hash(hash_hex),
        daa_score: DaaScore::new(daa),
        ts: Utc::now(),
        correlation_id,
    }
}

pub fn session_opened(
    conn_id: u64,
    wallet_addr: Option<&str>,
    worker_name: Option<&str>,
    remote_ip: &str,
) -> PoolEvent {
    PoolEvent::SessionOpened {
        conn_id,
        wallet: wallet_addr.map(wallet),
        worker: worker_name.map(worker),
        remote_ip: remote_ip.to_owned(),
        remote_app: None,
        connected_at: Utc::now(),
        correlation_id: CorrelationId::new_v4(),
    }
}

pub fn session_closed(
    conn_id: u64,
    wallet_addr: Option<&str>,
    worker_name: Option<&str>,
    remote_ip: &str,
) -> PoolEvent {
    PoolEvent::SessionClosed {
        conn_id,
        wallet: wallet_addr.map(wallet),
        worker: worker_name.map(worker),
        remote_ip: remote_ip.to_owned(),
        remote_app: None,
        connected_at: Utc::now(),
        ts: Utc::now(),
        correlation_id: CorrelationId::new_v4(),
    }
}

pub fn block_accepted(hash_hex: &str, correlation_id: CorrelationId) -> PoolEvent {
    PoolEvent::BlockAccepted {
        hash: hash(hash_hex),
        ts: Utc::now(),
        correlation_id,
    }
}
