//! M4 scale + determinism harness (CI-default).
//!
//! Generates a synthetic event stream sized for CI (~1:50 of a busy
//! 24h mainnet share rate) and proves dual-replay byte-equality via
//! [`accountant::verify_dual_replay`].

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_arithmetic,
    clippy::cast_possible_wrap,
    clippy::cast_possible_truncation
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

use accountant::verify_dual_replay;

/// ~1:50 of 24h at ~20 shares/s ≈ 34k events; CI uses 700 for speed.
const CI_EVENT_COUNT: usize = 700;

const WALLETS: [&str; 4] = [
    "kaspa:qypczcz0lhyf3tfsuqj86e7qc8us7r8a53nhlr4u6x4kq38td0hsjycf7sya7zq",
    "kaspa:qzncghl8re9h35hp6n5wyxtslhevj6462qkrkqzlfkrs2mpkfkc5xe9s3tga7",
    "kaspa:qz4j8mu269z8llgcczmfukm9fan2fq822kzxu4cfukd5fqrhxpsv2zhs9jxnp",
    "kaspa:qpxww7flfg6d3cnht3rqel9l56nyn40cwx8a2jkypx3n66rpzrr3kyc2ewm6c",
];

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
        application_name: "replay-scale".to_owned(),
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

fn correlation_id_for_sequence(seq: u64) -> CorrelationId {
    let mut bytes = [0u8; 16];
    bytes[0..8].copy_from_slice(&seq.to_be_bytes());
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    CorrelationId::from_uuid(Uuid::from_bytes(bytes))
}

fn synth_stream(n: usize) -> Vec<PoolEvent> {
    let ts = Utc.with_ymd_and_hms(2026, 5, 26, 12, 0, 0).unwrap();
    let mut events = Vec::with_capacity(n);
    for i in 0..n {
        let seq = u64::try_from(i + 1).expect("seq");
        let wallet_idx = i % WALLETS.len();
        let wallet = WalletAddress::new(WALLETS[wallet_idx].to_owned()).unwrap();
        let worker = WorkerName::new(format!("rig-{wallet_idx:02}")).unwrap();
        let cid = correlation_id_for_sequence(seq);
        match i % 17 {
            0 => events.push(PoolEvent::ShareRejected {
                wallet: wallet.clone(),
                worker: worker.clone(),
                reason: ShareRejectReason::LowDifficulty,
                ts,
                correlation_id: cid,
            }),
            1 => events.push(PoolEvent::ShareRejected {
                wallet: wallet.clone(),
                worker: worker.clone(),
                reason: ShareRejectReason::Stale,
                ts,
                correlation_id: cid,
            }),
            2 | 3 => {
                let mut bytes = [0u8; 32];
                bytes[0..8].copy_from_slice(b"replaym4");
                bytes[24..32].copy_from_slice(&(i as u64).to_be_bytes());
                let hash = BlockHash::from_bytes(bytes);
                events.push(PoolEvent::BlockFound {
                    wallet: wallet.clone(),
                    worker: worker.clone(),
                    hash,
                    daa_score: DaaScore::new(1_000_000 + u64::try_from(i).expect("daa")),
                    ts,
                    correlation_id: cid,
                });
                if i % 6 == 3 {
                    events.push(PoolEvent::BlockAccepted {
                        hash,
                        ts,
                        correlation_id: cid,
                    });
                }
            }
            _ => events.push(PoolEvent::ShareCredited {
                wallet,
                worker,
                difficulty: ShareDifficulty::new(1024.0 + f64::from((i % 8) as u32)).unwrap(),
                daa_score: DaaScore::new(1_000_000 + u64::try_from(i).expect("daa")),
                ts,
                correlation_id: cid,
            }),
        }
    }
    events
}

#[tokio::test]
async fn scaled_synthetic_stream_is_replay_deterministic() {
    let stream = synth_stream(CI_EVENT_COUNT);
    let env_a = fresh_db().await;
    let env_b = fresh_db().await;
    verify_dual_replay(
        &stream, "scale-a", "scale-b", "mainnet", &env_a.db, &env_b.db,
    )
    .await
    .expect("dual replay must be byte-equal");
}

#[tokio::test]
async fn ndjson_roundtrip_fixture_is_replay_deterministic() {
    let stream = synth_stream(64);
    let mut ndjson = String::new();
    for event in &stream {
        ndjson.push_str(&serde_json::to_string(event).unwrap());
        ndjson.push('\n');
    }
    let loaded = accountant::load_ndjson_reader(ndjson.as_bytes()).expect("load");
    assert_eq!(loaded.len(), stream.len());

    let env_a = fresh_db().await;
    let env_b = fresh_db().await;
    verify_dual_replay(
        &loaded, "ndjson-a", "ndjson-b", "mainnet", &env_a.db, &env_b.db,
    )
    .await
    .expect("ndjson dual replay");
}
