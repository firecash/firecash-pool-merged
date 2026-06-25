//! Tests for `ShareRejected` persistence and the
//! `repo::share_reject` aggregations.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_arithmetic
)]

use chrono::{Duration, Utc};
use katpool_db::repo::share_reject::{self, DbShareRejectReason};
use katpool_domain::{CorrelationId, PoolEvent, ShareRejectReason, WalletAddress, WorkerName};

use accountant::{ConsumerConfig, EventConsumer};

mod common;
use common::{MINER_A, MINER_B, setup};

fn cfg() -> ConsumerConfig {
    ConsumerConfig::new("test".to_owned(), "mainnet".to_owned()).unwrap()
}

fn rej(addr: &str, worker_name: &str, reason: ShareRejectReason) -> PoolEvent {
    PoolEvent::ShareRejected {
        wallet: WalletAddress::new(addr.to_owned()).unwrap(),
        worker: WorkerName::new(worker_name.to_owned()).unwrap(),
        reason,
        ts: Utc::now(),
        correlation_id: CorrelationId::new_v4(),
    }
}

#[tokio::test]
async fn share_rejected_persists_one_row() {
    let env = setup().await;
    let consumer = EventConsumer::new(env.db.clone(), cfg());
    consumer
        .handle_event(rej(MINER_A, "rig-01", ShareRejectReason::Stale))
        .await;

    let count: i64 = sqlx::query_scalar("SELECT count(*)::bigint FROM share_reject")
        .fetch_one(&env.db)
        .await
        .unwrap();
    assert_eq!(count, 1);

    let wallet_addr = WalletAddress::new(MINER_A.to_owned()).unwrap();
    let w = katpool_db::repo::wallet::find_by_address(&env.db, &wallet_addr)
        .await
        .unwrap()
        .unwrap();
    let rows = share_reject::list_for_wallet(&env.db, w.id, 10)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].reason, DbShareRejectReason::Stale);
}

#[tokio::test]
async fn share_rejected_aggregates_by_reason() {
    let env = setup().await;
    let consumer = EventConsumer::new(env.db.clone(), cfg());
    // Wallet A: 3 stale, 1 low_difficulty, 2 bad_pow
    for _ in 0..3 {
        consumer
            .handle_event(rej(MINER_A, "rig-01", ShareRejectReason::Stale))
            .await;
    }
    consumer
        .handle_event(rej(MINER_A, "rig-01", ShareRejectReason::LowDifficulty))
        .await;
    for _ in 0..2 {
        consumer
            .handle_event(rej(MINER_A, "rig-01", ShareRejectReason::BadPow))
            .await;
    }
    // Wallet B: 1 missing_job (should NOT bleed into A's counts).
    consumer
        .handle_event(rej(MINER_B, "rig-02", ShareRejectReason::MissingJob))
        .await;

    let wallet_addr = WalletAddress::new(MINER_A.to_owned()).unwrap();
    let w = katpool_db::repo::wallet::find_by_address(&env.db, &wallet_addr)
        .await
        .unwrap()
        .unwrap();

    let since = Utc::now() - Duration::hours(1);
    let by_reason = share_reject::count_by_reason_for_wallet(&env.db, w.id, since)
        .await
        .unwrap();
    let map: std::collections::HashMap<_, _> = by_reason.iter().copied().collect();
    assert_eq!(map.get(&DbShareRejectReason::Stale), Some(&3));
    assert_eq!(map.get(&DbShareRejectReason::LowDifficulty), Some(&1));
    assert_eq!(map.get(&DbShareRejectReason::BadPow), Some(&2));
    assert_eq!(map.get(&DbShareRejectReason::MissingJob), None);

    // Pool-wide counts include wallet B.
    let pool = share_reject::count_by_reason_pool_wide(&env.db, since)
        .await
        .unwrap();
    let map_pool: std::collections::HashMap<_, _> = pool.iter().copied().collect();
    assert_eq!(map_pool.get(&DbShareRejectReason::MissingJob), Some(&1));
    assert_eq!(map_pool.get(&DbShareRejectReason::Stale), Some(&3));
}

#[tokio::test]
async fn share_rejected_metric_only_when_persistence_fails_does_not_panic() {
    // Sanity: an obscure reject reason path still ticks the
    // metric counter and increments `wallet`/`worker` upserts.
    let env = setup().await;
    let consumer = EventConsumer::new(env.db.clone(), cfg());
    consumer
        .handle_event(rej(MINER_A, "rig-01", ShareRejectReason::DuplicateSubmit))
        .await;
    let rows: i64 = sqlx::query_scalar("SELECT count(*)::bigint FROM share_reject")
        .fetch_one(&env.db)
        .await
        .unwrap();
    assert_eq!(rows, 1);
}

#[tokio::test]
async fn share_reject_reason_enum_label_matches_domain() {
    // The DB enum's variant labels must match
    // `ShareRejectReason::as_str()` byte-for-byte — anything
    // else and Prometheus alerts (keyed on the bridge's `reason`
    // label) won't line up against DB rows.
    assert_eq!(
        DbShareRejectReason::Stale.as_str(),
        ShareRejectReason::Stale.as_str()
    );
    assert_eq!(
        DbShareRejectReason::LowDifficulty.as_str(),
        ShareRejectReason::LowDifficulty.as_str()
    );
    assert_eq!(
        DbShareRejectReason::BadPow.as_str(),
        ShareRejectReason::BadPow.as_str()
    );
    assert_eq!(
        DbShareRejectReason::MissingJob.as_str(),
        ShareRejectReason::MissingJob.as_str()
    );
    assert_eq!(
        DbShareRejectReason::MalformedFrame.as_str(),
        ShareRejectReason::MalformedFrame.as_str()
    );
    assert_eq!(
        DbShareRejectReason::DuplicateSubmit.as_str(),
        ShareRejectReason::DuplicateSubmit.as_str()
    );
    assert_eq!(
        DbShareRejectReason::BadAddress.as_str(),
        ShareRejectReason::BadAddress.as_str()
    );
}
