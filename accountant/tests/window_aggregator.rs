//! Tests for `accountant::WindowAggregator`.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_arithmetic,
    clippy::similar_names
)]

use chrono::Utc;
use katpool_db::repo::{share, wallet, worker};
use katpool_domain::{DaaScore, ShareDifficulty, WalletAddress, WorkerName};

use accountant::WindowAggregator;

mod common;
use common::{MINER_A, MINER_B, setup};

const NETWORK: &str = "mainnet";

async fn ensure_wallet_with_worker(
    db: &sqlx::PgPool,
    wallet_addr: &str,
    worker_name: &str,
) -> (katpool_db::repo::WalletId, katpool_db::repo::WorkerId) {
    let addr = WalletAddress::new(wallet_addr.to_owned()).unwrap();
    let name = WorkerName::new(worker_name.to_owned()).unwrap();
    let mut tx = db.begin().await.unwrap();
    let w = wallet::ensure(&mut *tx, &addr, NETWORK).await.unwrap();
    let wk = worker::ensure(&mut *tx, w.id, &name).await.unwrap();
    tx.commit().await.unwrap();
    (w.id, wk.id)
}

async fn seed_share(
    db: &sqlx::PgPool,
    wallet_id: katpool_db::repo::WalletId,
    worker_id: katpool_db::repo::WorkerId,
    difficulty: f64,
    daa: u64,
) {
    share::insert_credited(
        db,
        wallet_id,
        worker_id,
        None,
        ShareDifficulty::new(difficulty).unwrap(),
        DaaScore::new(daa),
        katpool_domain::CorrelationId::new_v4(),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn aggregator_rolls_up_shares_per_wallet() {
    let env = setup().await;
    let (wa, wka) = ensure_wallet_with_worker(&env.db, MINER_A, "rig-01").await;
    let (wb, wkb) = ensure_wallet_with_worker(&env.db, MINER_B, "rig-02").await;

    // Wallet A: two shares in [1000, 2000): difficulty 100 + 200.
    seed_share(&env.db, wa, wka, 100.0, 1100).await;
    seed_share(&env.db, wa, wka, 200.0, 1500).await;
    // Wallet B: one share in [1000, 2000): difficulty 500.
    seed_share(&env.db, wb, wkb, 500.0, 1200).await;
    // Wallet A: one share OUTSIDE [1000, 2000): difficulty 999.
    seed_share(&env.db, wa, wka, 999.0, 2500).await;

    let agg = WindowAggregator::new(env.db.clone());
    let outcome = agg
        .close_window(DaaScore::new(1000), DaaScore::new(2000), Utc::now())
        .await
        .unwrap();

    assert_eq!(outcome.wallets, 2, "two wallets contributed");
    assert_eq!(outcome.shares, 3, "three shares in-range; one out-of-range");
    assert!(
        (outcome.total_weight - 800.0).abs() < 1e-9,
        "total weight = 100 + 200 + 500 = 800; got {}",
        outcome.total_weight
    );

    let row_a =
        katpool_db::repo::share_window::find(&env.db, wa, DaaScore::new(1000), DaaScore::new(2000))
            .await
            .unwrap()
            .expect("wallet A window row");
    assert!((row_a.total_weight - 300.0).abs() < 1e-9);
    assert_eq!(row_a.share_count, 2);

    let row_b =
        katpool_db::repo::share_window::find(&env.db, wb, DaaScore::new(1000), DaaScore::new(2000))
            .await
            .unwrap()
            .expect("wallet B window row");
    assert!((row_b.total_weight - 500.0).abs() < 1e-9);
    assert_eq!(row_b.share_count, 1);
}

#[tokio::test]
async fn aggregator_empty_window_is_no_op() {
    let env = setup().await;
    let agg = WindowAggregator::new(env.db.clone());
    let outcome = agg
        .close_window(DaaScore::new(0), DaaScore::new(100), Utc::now())
        .await
        .unwrap();
    assert_eq!(outcome.wallets, 0);
    assert_eq!(outcome.shares, 0);
    assert!(outcome.total_weight.abs() < 1e-12);
}

#[tokio::test]
async fn aggregator_is_idempotent_and_refreshes_on_rerun() {
    let env = setup().await;
    let (wa, wka) = ensure_wallet_with_worker(&env.db, MINER_A, "rig-01").await;
    seed_share(&env.db, wa, wka, 100.0, 1100).await;

    let agg = WindowAggregator::new(env.db.clone());
    let first = agg
        .close_window(DaaScore::new(1000), DaaScore::new(2000), Utc::now())
        .await
        .unwrap();
    assert_eq!(first.shares, 1);

    // A second share lands inside the (already-closed) window.
    // This is the late-arrival scenario the aggregator's
    // ON CONFLICT branch is designed for.
    seed_share(&env.db, wa, wka, 250.0, 1700).await;

    let second = agg
        .close_window(DaaScore::new(1000), DaaScore::new(2000), Utc::now())
        .await
        .unwrap();
    assert_eq!(second.wallets, 1, "still one wallet");
    assert_eq!(second.shares, 2, "now both shares counted");
    assert!((second.total_weight - 350.0).abs() < 1e-9);

    // Verify there's exactly ONE row, not two.
    let row_count: i64 = sqlx::query_scalar(
        "SELECT count(*)::bigint FROM share_window
          WHERE wallet_id = $1 AND daa_start = 1000 AND daa_end = 2000",
    )
    .bind(wa.0)
    .fetch_one(&env.db)
    .await
    .unwrap();
    assert_eq!(row_count, 1, "ON CONFLICT must NOT duplicate rows");
}

#[tokio::test]
async fn aggregator_rejects_inverted_range() {
    let env = setup().await;
    let agg = WindowAggregator::new(env.db.clone());
    let err = agg
        .close_window(DaaScore::new(2000), DaaScore::new(1000), Utc::now())
        .await
        .unwrap_err();
    assert!(format!("{err}").contains("must be > daa_start"));

    // Equal endpoints also rejected.
    let err2 = agg
        .close_window(DaaScore::new(1000), DaaScore::new(1000), Utc::now())
        .await
        .unwrap_err();
    assert!(format!("{err2}").contains("must be > daa_start"));
}

#[tokio::test]
async fn aggregator_preserves_first_credited_at_on_rerun() {
    let env = setup().await;
    let (wa, wka) = ensure_wallet_with_worker(&env.db, MINER_A, "rig-01").await;
    seed_share(&env.db, wa, wka, 100.0, 1100).await;

    let agg = WindowAggregator::new(env.db.clone());
    let _ = agg
        .close_window(DaaScore::new(1000), DaaScore::new(2000), Utc::now())
        .await
        .unwrap();
    let row_initial =
        katpool_db::repo::share_window::find(&env.db, wa, DaaScore::new(1000), DaaScore::new(2000))
            .await
            .unwrap()
            .unwrap();

    // Sleep so the second close gets a clearly-later `now`.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let later = Utc::now();
    let _ = agg
        .close_window(DaaScore::new(1000), DaaScore::new(2000), later)
        .await
        .unwrap();

    let row_after =
        katpool_db::repo::share_window::find(&env.db, wa, DaaScore::new(1000), DaaScore::new(2000))
            .await
            .unwrap()
            .unwrap();
    assert_eq!(
        row_after.started_at, row_initial.started_at,
        "started_at must survive the ON CONFLICT refresh"
    );
    assert!(
        row_after.ended_at >= row_initial.ended_at,
        "ended_at should advance (or stay the same) on rerun"
    );
}
