//! Replay harness primitives for the accountant event consumer.
//!
//! Used by integration tests (`tests/replay_determinism.rs`) and CI scale
//! fixtures. The
//! contract under test is: **given the same `PoolEvent` stream,
//! two independent replays into empty Postgres databases produce
//! byte-equal consumer-written rows** (PKs and wallclock columns
//! excluded — see [`DbSnapshot`]).

use std::io::{BufRead, BufReader};
use std::path::Path;

use anyhow::Context;
use katpool_domain::PoolEvent;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{ConsumerConfig, EventConsumer};

/// Content snapshot of every table the consumer writes.
///
/// Serial PKs and `now()`-defaulted wallclock columns are excluded
/// because they differ across independent database instances even
/// when business data is identical.
#[derive(Debug, Clone, PartialEq)]
pub struct DbSnapshot {
    /// `(address, network)` ordered by address.
    pub wallets: Vec<(String, String)>,
    /// Worker names ordered lexicographically.
    pub workers: Vec<(String,)>,
    /// `(difficulty, daa_score, correlation_id)` ordered by correlation id.
    pub shares: Vec<(f64, i64, Uuid)>,
    /// `(reason, correlation_id)` ordered by correlation id.
    pub rejects: Vec<(String, Uuid)>,
    /// `(hash, daa_score, nonce, status, correlation_id)` ordered by hash.
    pub blocks: Vec<(Vec<u8>, i64, i64, String, Uuid)>,
}

/// Load a newline-delimited JSON event log (`PoolEvent` per line).
///
/// Blank lines and lines starting with `#` are skipped. Each line
/// must deserialize to exactly one [`PoolEvent`].
pub fn load_ndjson_reader<R: BufRead>(reader: R) -> anyhow::Result<Vec<PoolEvent>> {
    let mut events = Vec::new();
    for (line_no, line) in reader.lines().enumerate() {
        let line = line.with_context(|| format!("reading event log line {}", line_no + 1))?;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let event: PoolEvent = serde_json::from_str(trimmed).with_context(|| {
            format!(
                "deserializing PoolEvent on line {}: `{trimmed}`",
                line_no + 1
            )
        })?;
        events.push(event);
    }
    Ok(events)
}

/// Load events from a filesystem path.
pub fn load_ndjson_path(path: &Path) -> anyhow::Result<Vec<PoolEvent>> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("opening event log `{}`", path.display()))?;
    load_ndjson_reader(BufReader::new(file))
}

/// Replay every event through the consumer in file order.
pub async fn replay_all(consumer: &EventConsumer, events: &[PoolEvent]) {
    for event in events {
        consumer.handle_event(event.clone()).await;
    }
}

/// Snapshot consumer-written tables in a canonical order.
pub async fn snapshot(db: &PgPool) -> anyhow::Result<DbSnapshot> {
    let wallets: Vec<(String, String)> =
        sqlx::query_as("SELECT address, network FROM wallet ORDER BY address")
            .fetch_all(db)
            .await
            .context("snapshot wallet")?;
    let workers: Vec<(String,)> = sqlx::query_as("SELECT name FROM worker ORDER BY name")
        .fetch_all(db)
        .await
        .context("snapshot worker")?;
    let shares: Vec<(f64, i64, Uuid)> = sqlx::query_as(
        "SELECT difficulty, daa_score, correlation_id
           FROM share
          ORDER BY correlation_id",
    )
    .fetch_all(db)
    .await
    .context("snapshot share")?;
    let rejects: Vec<(String, Uuid)> = sqlx::query_as(
        "SELECT reason::text, correlation_id
           FROM share_reject
          ORDER BY correlation_id",
    )
    .fetch_all(db)
    .await
    .context("snapshot share_reject")?;
    let blocks: Vec<(Vec<u8>, i64, i64, String, Uuid)> = sqlx::query_as(
        "SELECT hash, daa_score, nonce, status::text, correlation_id
           FROM block
          ORDER BY hash",
    )
    .fetch_all(db)
    .await
    .context("snapshot block")?;

    Ok(DbSnapshot {
        wallets,
        workers,
        shares,
        rejects,
        blocks,
    })
}

/// Assert two snapshots are equal, with a diff-friendly message.
pub fn assert_snapshots_equal(a: &DbSnapshot, b: &DbSnapshot) -> anyhow::Result<()> {
    if a == b {
        return Ok(());
    }
    anyhow::bail!(
        "replay determinism violation: snapshots differ\n  wallets: {} vs {}\n  workers: {} vs {}\n  shares: {} vs {}\n  rejects: {} vs {}\n  blocks: {} vs {}",
        a.wallets.len(),
        b.wallets.len(),
        a.workers.len(),
        b.workers.len(),
        a.shares.len(),
        b.shares.len(),
        a.rejects.len(),
        b.rejects.len(),
        a.blocks.len(),
        b.blocks.len(),
    );
}

/// Replay `events` into two independent empty databases and assert
/// byte-equal snapshots.
pub async fn verify_dual_replay(
    events: &[PoolEvent],
    instance_a: &str,
    instance_b: &str,
    network: &str,
    db_a: &PgPool,
    db_b: &PgPool,
) -> anyhow::Result<()> {
    let cfg_a = ConsumerConfig::new(instance_a.to_owned(), network.to_owned())
        .context("consumer config A")?;
    let cfg_b = ConsumerConfig::new(instance_b.to_owned(), network.to_owned())
        .context("consumer config B")?;
    let consumer_a = EventConsumer::new(db_a.clone(), cfg_a);
    let consumer_b = EventConsumer::new(db_b.clone(), cfg_b);
    replay_all(&consumer_a, events).await;
    replay_all(&consumer_b, events).await;
    let snap_a = snapshot(db_a).await?;
    let snap_b = snapshot(db_b).await?;
    assert_snapshots_equal(&snap_a, &snap_b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use katpool_domain::{
        BlockHash, CorrelationId, DaaScore, ShareDifficulty, WalletAddress, WorkerName,
    };

    #[test]
    fn ndjson_roundtrip_and_skip_comments() {
        let wallet = WalletAddress::new(
            "kaspa:qypczcz0lhyf3tfsuqj86e7qc8us7r8a53nhlr4u6x4kq38td0hsjycf7sya7zq",
        )
        .unwrap();
        let worker = WorkerName::new("rig-01").unwrap();
        let ts = Utc.with_ymd_and_hms(2026, 5, 26, 0, 0, 0).unwrap();
        let event = PoolEvent::ShareCredited {
            wallet,
            worker,
            difficulty: ShareDifficulty::new(1024.0).unwrap(),
            daa_score: DaaScore::new(1),
            ts,
            correlation_id: CorrelationId::new_v4(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let input = format!("# header\n\n{json}\n");
        let loaded = load_ndjson_reader(input.as_bytes()).unwrap();
        assert_eq!(loaded.len(), 1);
        assert!(matches!(loaded[0], PoolEvent::ShareCredited { .. }));
    }

    #[test]
    fn block_found_serializes_for_ndjson_fixture() {
        let wallet = WalletAddress::new(
            "kaspa:qypczcz0lhyf3tfsuqj86e7qc8us7r8a53nhlr4u6x4kq38td0hsjycf7sya7zq",
        )
        .unwrap();
        let worker = WorkerName::new("rig-01").unwrap();
        let hash =
            BlockHash::from_hex("cc2b1da2c931f4164c03b2066cfb3178303567a161e8a393def62c91e824138a")
                .unwrap();
        let event = PoolEvent::BlockFound {
            wallet,
            worker,
            hash,
            daa_score: DaaScore::new(1_000_002),
            ts: Utc.with_ymd_and_hms(2026, 5, 26, 0, 0, 0).unwrap(),
            correlation_id: CorrelationId::new_v4(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("BlockFound"));
    }
}
