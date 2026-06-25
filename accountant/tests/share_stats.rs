//! Tests for `repo::share_stats` aggregations.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_arithmetic,
    clippy::similar_names
)]

use chrono::{Duration, Utc};
use katpool_db::repo::{share, share_stats, wallet, worker};
use katpool_domain::{
    CorrelationId, DaaScore, ShareDifficulty, ShareRejectReason, WalletAddress, WorkerName,
};

use accountant::{ConsumerConfig, EventConsumer};

mod common;
use common::{MINER_A, MINER_B, setup};

const NETWORK: &str = "mainnet";

async fn ensure_a_b(
    db: &sqlx::PgPool,
) -> (
    (katpool_db::repo::WalletId, katpool_db::repo::WorkerId),
    (katpool_db::repo::WalletId, katpool_db::repo::WorkerId),
) {
    let mut tx = db.begin().await.unwrap();
    let wa = wallet::ensure(
        &mut *tx,
        &WalletAddress::new(MINER_A.to_owned()).unwrap(),
        NETWORK,
    )
    .await
    .unwrap();
    let wka = worker::ensure(
        &mut *tx,
        wa.id,
        &WorkerName::new("rig-01".to_owned()).unwrap(),
    )
    .await
    .unwrap();
    let wb = wallet::ensure(
        &mut *tx,
        &WalletAddress::new(MINER_B.to_owned()).unwrap(),
        NETWORK,
    )
    .await
    .unwrap();
    let wkb = worker::ensure(
        &mut *tx,
        wb.id,
        &WorkerName::new("rig-02".to_owned()).unwrap(),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    ((wa.id, wka.id), (wb.id, wkb.id))
}

#[tokio::test]
async fn accepted_returns_zero_for_idle_wallet() {
    let env = setup().await;
    let ((wa, _), _) = ensure_a_b(&env.db).await;
    let s = share_stats::accepted_for_wallet(&env.db, wa, Utc::now() - Duration::hours(1))
        .await
        .unwrap();
    assert_eq!(s.share_count, 0);
    assert!(s.total_weight.abs() < 1e-12);
}

#[tokio::test]
async fn accepted_sums_difficulty_per_wallet() {
    let env = setup().await;
    let ((wa, wka), (wb, wkb)) = ensure_a_b(&env.db).await;

    for diff in [100.0_f64, 200.0, 300.0] {
        share::insert_credited(
            &env.db,
            wa,
            wka,
            None,
            ShareDifficulty::new(diff).unwrap(),
            DaaScore::new(1_000_000),
            CorrelationId::new_v4(),
        )
        .await
        .unwrap();
    }
    share::insert_credited(
        &env.db,
        wb,
        wkb,
        None,
        ShareDifficulty::new(750.0).unwrap(),
        DaaScore::new(1_000_000),
        CorrelationId::new_v4(),
    )
    .await
    .unwrap();

    let since = Utc::now() - Duration::hours(1);
    let a = share_stats::accepted_for_wallet(&env.db, wa, since)
        .await
        .unwrap();
    assert_eq!(a.share_count, 3);
    assert!((a.total_weight - 600.0).abs() < 1e-9);

    let b = share_stats::accepted_for_wallet(&env.db, wb, since)
        .await
        .unwrap();
    assert_eq!(b.share_count, 1);
    assert!((b.total_weight - 750.0).abs() < 1e-9);

    let pool = share_stats::accepted_pool_wide(&env.db, since)
        .await
        .unwrap();
    assert_eq!(pool.share_count, 4);
    assert!((pool.total_weight - 1350.0).abs() < 1e-9);
}

#[tokio::test]
async fn hashrate_estimate_uses_2_pow_32_factor() {
    let env = setup().await;
    let ((wa, wka), _) = ensure_a_b(&env.db).await;
    // 100 shares of difficulty 4_294_967_296 (= 2^32) within a
    // 100-second window: expected hashrate is exactly 2^64 / 100
    // hashes per second.
    for _ in 0..100 {
        share::insert_credited(
            &env.db,
            wa,
            wka,
            None,
            ShareDifficulty::new(4_294_967_296.0).unwrap(),
            DaaScore::new(1_000_000),
            CorrelationId::new_v4(),
        )
        .await
        .unwrap();
    }
    // Anchor the window to the first share so the (pool-age-clamped)
    // denominator is exactly the intended 100 s: shares are all credited at
    // insert time, and the hashrate estimator divides by
    // `until - max(since, first_share_in_window)`, so a window that started
    // before the first share would otherwise shrink the denominator.
    let first: chrono::DateTime<Utc> = sqlx::query_scalar("SELECT min(credited_at) FROM share")
        .fetch_one(&env.db)
        .await
        .unwrap();
    let since = first;
    let until = first + Duration::seconds(100);
    let rate = share_stats::hashrate_estimate_for_wallet(&env.db, wa, since, until)
        .await
        .unwrap();
    // 100 shares * 2^32 difficulty * 2^32 hashes/difficulty / 100s
    // = 2^64 / 100 hashes/s.
    let expected = 100.0_f64 * 4_294_967_296.0 * 4_294_967_296.0 / 100.0;
    assert!(
        (rate / expected - 1.0).abs() < 1e-9,
        "rate {rate} not close to expected {expected}"
    );
}

#[tokio::test]
async fn hashrate_estimate_rejects_inverted_window() {
    let env = setup().await;
    let ((wa, _), _) = ensure_a_b(&env.db).await;
    let now = Utc::now();
    assert!(
        share_stats::hashrate_estimate_for_wallet(&env.db, wa, now, now)
            .await
            .is_err()
    );
    assert!(
        share_stats::hashrate_estimate_for_wallet(&env.db, wa, now, now - Duration::seconds(1))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn combined_summary_one_round_trip() {
    let env = setup().await;
    let ((wa, wka), _) = ensure_a_b(&env.db).await;
    let consumer = EventConsumer::new(
        env.db.clone(),
        ConsumerConfig::new("test".to_owned(), "mainnet".to_owned()).unwrap(),
    );

    // Two accepted, three rejected for wallet A.
    for _ in 0..2 {
        share::insert_credited(
            &env.db,
            wa,
            wka,
            None,
            ShareDifficulty::new(500.0).unwrap(),
            DaaScore::new(1_000_000),
            CorrelationId::new_v4(),
        )
        .await
        .unwrap();
    }
    for _ in 0..3 {
        consumer
            .handle_event(katpool_domain::PoolEvent::ShareRejected {
                wallet: WalletAddress::new(MINER_A.to_owned()).unwrap(),
                worker: WorkerName::new("rig-01".to_owned()).unwrap(),
                reason: ShareRejectReason::Stale,
                ts: Utc::now(),
                correlation_id: CorrelationId::new_v4(),
            })
            .await;
    }

    let s =
        share_stats::accepted_and_rejected_for_wallet(&env.db, wa, Utc::now() - Duration::hours(1))
            .await
            .unwrap();
    assert_eq!(s.accepted_count, 2);
    assert!((s.accepted_weight - 1000.0).abs() < 1e-9);
    assert_eq!(s.rejected_count, 3);
}
