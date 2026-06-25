//! Per-miner stats — read-side aggregations over the `share` and
//! `share_reject` tables.
//!
//! The HTTP API in Phase 6 composes these into the JSON the
//! frontend renders. The accountant uses some of them in its
//! Prometheus exporter so a Grafana dashboard can chart per-miner
//! hashrate without scraping every miner directly.
//!
//! All functions are **read-only** — they never write to the
//! database. Time-range queries take an inclusive `since`
//! `DateTime<Utc>`.

// `float_arithmetic` is denied workspace-wide because most of our
// money math must be integer. Hashrate estimates are the
// exception: they're floating-point by definition (H/s is a rate,
// not a sompi figure), and Phase 6 surfaces them as JSON numbers
// where lossy f64 is the accepted representation.
#![allow(
    clippy::cast_possible_wrap,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::float_arithmetic
)]

use chrono::{DateTime, Utc};
use sqlx::PgExecutor;

use crate::DbError;
use crate::repo::{WalletId, WorkerId};

/// The share-difficulty → expected-hashes constant. A share of
/// difficulty `D` represents `D × 2^32` expected hashes; dividing the
/// summed difficulty by the window's wall-clock seconds yields H/s.
/// `2^32` is exact in `f64` (only 33 significant bits).
const HASHES_PER_DIFFICULTY: f64 = 4_294_967_296.0;

/// Estimated hashrate (H/s) from a summed share-difficulty `weight`
/// accumulated over `secs` wall-clock seconds, using the `2^32`-hashes-
/// per-difficulty stratum convention (see [`HASHES_PER_DIFFICULTY`]).
///
/// This is the single definition of the pool's hashrate estimate; every
/// per-wallet/-worker/-bucket/leaderboard figure routes through it so they
/// cannot drift apart. Callers guarantee `secs > 0` (the window validators
/// reject empty/inverted ranges), so no divide-by-zero guard is needed here.
#[must_use]
fn hashrate_hs(weight: f64, secs: f64) -> f64 {
    weight * HASHES_PER_DIFFICULTY / secs
}

/// Wall-clock seconds to divide a windowed share-weight by, clamped to the
/// span the pool actually existed within `[since, until)`.
///
/// A windowed hashrate is `weight × 2^32 / seconds`. Dividing by the full
/// nominal window over-states the denominator whenever the pool is younger
/// than the window (e.g. a 24h leaderboard the day after a fresh cutover):
/// there is no share data for the part of the window that predates the pool,
/// so every figure is uniformly under-reported. `pool_first` is the earliest
/// share present in the window (pool-wide); when it is later than `since` it
/// becomes the effective window start, so the rate reflects the period over
/// which shares could actually have been produced. For a pool older than the
/// window `pool_first <= since`, leaving the full window in force. Never < 1.
#[must_use]
fn effective_window_secs(
    since: DateTime<Utc>,
    until: DateTime<Utc>,
    pool_first: Option<DateTime<Utc>>,
) -> f64 {
    let start = match pool_first {
        Some(first) if first > since => first,
        _ => since,
    };
    (until - start).num_seconds().max(1) as f64
}

/// Per-miner accepted-share aggregates over a time window.
#[derive(Debug, Clone, Copy, PartialEq, sqlx::FromRow)]
pub struct AcceptedShareStats {
    /// Count of accepted shares.
    pub share_count: i64,
    /// Sum of share difficulty — the PROP weight contribution.
    pub total_weight: f64,
}

/// Accepted share aggregate for one wallet since `since`.
///
/// Returns `(0, 0.0)` for wallets with no shares in the window
/// — that's the legitimate "no activity" answer, not a
/// not-found case.
pub async fn accepted_for_wallet<'e, E>(
    executor: E,
    wallet_id: WalletId,
    since: DateTime<Utc>,
) -> Result<AcceptedShareStats, DbError>
where
    E: PgExecutor<'e>,
{
    let row: (Option<i64>, Option<f64>) = sqlx::query_as(
        "SELECT count(*)::bigint, sum(difficulty)
           FROM share
          WHERE wallet_id = $1
            AND credited_at >= $2",
    )
    .bind(wallet_id.0)
    .bind(since)
    .fetch_one(executor)
    .await?;
    Ok(AcceptedShareStats {
        share_count: row.0.unwrap_or(0),
        total_weight: row.1.unwrap_or(0.0),
    })
}

/// Pool-wide accepted share aggregate since `since`.
pub async fn accepted_pool_wide<'e, E>(
    executor: E,
    since: DateTime<Utc>,
) -> Result<AcceptedShareStats, DbError>
where
    E: PgExecutor<'e>,
{
    let row: (Option<i64>, Option<f64>) = sqlx::query_as(
        "SELECT count(*)::bigint, sum(difficulty)
           FROM share
          WHERE credited_at >= $1",
    )
    .bind(since)
    .fetch_one(executor)
    .await?;
    Ok(AcceptedShareStats {
        share_count: row.0.unwrap_or(0),
        total_weight: row.1.unwrap_or(0.0),
    })
}

/// Estimated hashrate over a sliding wall-clock window, in H/s.
///
/// Computation: `sum(difficulty * 2^32) / window_secs`. The
/// factor of 2^32 comes from the share-difficulty convention —
/// one share of difficulty D represents D × 2^32 expected hashes.
///
/// Returns 0.0 if the window has zero shares.
pub async fn hashrate_estimate_for_wallet<'e, E>(
    executor: E,
    wallet_id: WalletId,
    since: DateTime<Utc>,
    until: DateTime<Utc>,
) -> Result<f64, DbError>
where
    E: PgExecutor<'e>,
{
    if until <= since {
        return Err(DbError::Config {
            message: "hashrate_estimate_for_wallet: until must be after since".to_owned(),
        });
    }
    // Pool-wide `min(credited_at)` (not wallet-scoped) so the denominator
    // corrects only for pool age, not for when this wallet joined.
    let row: (Option<f64>, Option<DateTime<Utc>>) = sqlx::query_as(
        "SELECT sum(difficulty),
                (SELECT min(credited_at) FROM share
                  WHERE credited_at >= $2 AND credited_at < $3)
           FROM share
          WHERE wallet_id = $1
            AND credited_at >= $2
            AND credited_at <  $3",
    )
    .bind(wallet_id.0)
    .bind(since)
    .bind(until)
    .fetch_one(executor)
    .await?;
    let weight = row.0.unwrap_or(0.0);
    let secs = effective_window_secs(since, until, row.1);
    Ok(hashrate_hs(weight, secs))
}

/// Pool-wide estimated hashrate over the same window.
pub async fn hashrate_estimate_pool_wide<'e, E>(
    executor: E,
    since: DateTime<Utc>,
    until: DateTime<Utc>,
) -> Result<f64, DbError>
where
    E: PgExecutor<'e>,
{
    if until <= since {
        return Err(DbError::Config {
            message: "hashrate_estimate_pool_wide: until must be after since".to_owned(),
        });
    }
    let row: (Option<f64>, Option<DateTime<Utc>>) = sqlx::query_as(
        "SELECT sum(difficulty), min(credited_at)
           FROM share
          WHERE credited_at >= $1
            AND credited_at <  $2",
    )
    .bind(since)
    .bind(until)
    .fetch_one(executor)
    .await?;
    let weight = row.0.unwrap_or(0.0);
    let secs = effective_window_secs(since, until, row.1);
    Ok(hashrate_hs(weight, secs))
}

/// Combined accepted + rejected counts for a wallet — both since
/// `since` so the caller sees a consistent time window.
///
/// One round-trip; the SQL emits a single row with both halves.
pub async fn accepted_and_rejected_for_wallet<'e, E>(
    executor: E,
    wallet_id: WalletId,
    since: DateTime<Utc>,
) -> Result<WalletShareSummary, DbError>
where
    E: PgExecutor<'e>,
{
    let row: (Option<i64>, Option<f64>, Option<i64>) = sqlx::query_as(
        "SELECT
           (SELECT count(*)::bigint
              FROM share
             WHERE wallet_id = $1 AND credited_at >= $2),
           (SELECT sum(difficulty)
              FROM share
             WHERE wallet_id = $1 AND credited_at >= $2),
           (SELECT count(*)::bigint
              FROM share_reject
             WHERE wallet_id = $1 AND rejected_at >= $2)",
    )
    .bind(wallet_id.0)
    .bind(since)
    .fetch_one(executor)
    .await?;
    Ok(WalletShareSummary {
        accepted_count: row.0.unwrap_or(0),
        accepted_weight: row.1.unwrap_or(0.0),
        rejected_count: row.2.unwrap_or(0),
    })
}

/// One-shot summary of a wallet's share activity. Used by the
/// Phase 6 API's `/miner/{address}` JSON.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WalletShareSummary {
    /// Accepted shares since the window start.
    pub accepted_count: i64,
    /// Sum of accepted share difficulty.
    pub accepted_weight: f64,
    /// Rejected shares since the window start (across all reasons).
    pub rejected_count: i64,
}

/// Accepted share aggregate for one worker since `since`.
///
/// Returns `(0, 0.0)` for workers with no shares in the window — the
/// legitimate "no activity" answer, not a not-found case. Drives the
/// Phase 6 API's per-worker (`/miners/{address}/workers`) breakdown.
pub async fn accepted_for_worker<'e, E>(
    executor: E,
    worker_id: WorkerId,
    since: DateTime<Utc>,
) -> Result<AcceptedShareStats, DbError>
where
    E: PgExecutor<'e>,
{
    let row: (Option<i64>, Option<f64>) = sqlx::query_as(
        "SELECT count(*)::bigint, sum(difficulty)
           FROM share
          WHERE worker_id = $1
            AND credited_at >= $2",
    )
    .bind(worker_id.0)
    .bind(since)
    .fetch_one(executor)
    .await?;
    Ok(AcceptedShareStats {
        share_count: row.0.unwrap_or(0),
        total_weight: row.1.unwrap_or(0.0),
    })
}

/// Estimated hashrate for one worker over a sliding window, in H/s.
///
/// Same `sum(difficulty) × 2^32 / window_secs` convention as
/// [`hashrate_estimate_for_wallet`]. Returns 0.0 for an empty window.
pub async fn hashrate_estimate_for_worker<'e, E>(
    executor: E,
    worker_id: WorkerId,
    since: DateTime<Utc>,
    until: DateTime<Utc>,
) -> Result<f64, DbError>
where
    E: PgExecutor<'e>,
{
    if until <= since {
        return Err(DbError::Config {
            message: "hashrate_estimate_for_worker: until must be after since".to_owned(),
        });
    }
    let row: (Option<f64>, Option<DateTime<Utc>>) = sqlx::query_as(
        "SELECT sum(difficulty),
                (SELECT min(credited_at) FROM share
                  WHERE credited_at >= $2 AND credited_at < $3)
           FROM share
          WHERE worker_id = $1
            AND credited_at >= $2
            AND credited_at <  $3",
    )
    .bind(worker_id.0)
    .bind(since)
    .bind(until)
    .fetch_one(executor)
    .await?;
    let weight = row.0.unwrap_or(0.0);
    let secs = effective_window_secs(since, until, row.1);
    Ok(hashrate_hs(weight, secs))
}

/// Distinct active wallets and workers (≥ 1 accepted share) since
/// `since`. One round-trip; drives the pool-stats "miners online"
/// figure.
pub async fn active_participant_counts<'e, E>(
    executor: E,
    since: DateTime<Utc>,
) -> Result<ActiveCounts, DbError>
where
    E: PgExecutor<'e>,
{
    let row: (Option<i64>, Option<i64>) = sqlx::query_as(
        "SELECT count(DISTINCT wallet_id)::bigint,
                count(DISTINCT worker_id)::bigint
           FROM share
          WHERE credited_at >= $1",
    )
    .bind(since)
    .fetch_one(executor)
    .await?;
    Ok(ActiveCounts {
        wallets: row.0.unwrap_or(0),
        workers: row.1.unwrap_or(0),
    })
}

/// Distinct-participant counts over a window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActiveCounts {
    /// Distinct wallets with ≥ 1 accepted share.
    pub wallets: i64,
    /// Distinct workers with ≥ 1 accepted share.
    pub workers: i64,
}

/// One point of a hashrate time-series: the bucket's start instant and
/// the estimated hashrate (H/s) over that bucket.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HashratePoint {
    /// Inclusive bucket start (UTC), aligned to a `bucket_secs` grid.
    pub bucket_start: DateTime<Utc>,
    /// Estimated hashrate over the bucket in H/s.
    pub hashrate: f64,
    /// `true` when the query's `until` bound falls inside this bucket, so the
    /// rate was divided by elapsed seconds rather than the full bucket width.
    pub is_partial: bool,
}

/// Validate the shared arguments of the series queries.
fn validate_series_args(
    from: DateTime<Utc>,
    until: DateTime<Utc>,
    bucket_secs: i64,
    what: &str,
) -> Result<f64, DbError> {
    if until <= from {
        return Err(DbError::Config {
            message: format!("{what}: until must be after from"),
        });
    }
    if bucket_secs <= 0 {
        return Err(DbError::Config {
            message: format!("{what}: bucket_secs must be positive, got {bucket_secs}"),
        });
    }
    Ok(bucket_secs as f64)
}

/// Wall-clock seconds to attribute to one bucket when a series ends at `until`
/// (the query's exclusive upper bound). Completed buckets use the full width;
/// the trailing in-progress bucket uses elapsed time since its start, clamped
/// to at least one second so a just-opened bucket does not divide by zero.
#[must_use]
fn bucket_effective_secs(
    bucket_start: DateTime<Utc>,
    until: DateTime<Utc>,
    bucket_secs: i64,
) -> f64 {
    let elapsed = (until - bucket_start).num_seconds();
    if elapsed >= bucket_secs {
        bucket_secs as f64
    } else {
        elapsed.max(1) as f64
    }
}

/// Build [`HashratePoint`]s from raw `(bucket_epoch, weight)` rows.
///
/// `bucket_epoch` is the integer bucket-grid second; the weight is the
/// summed share difficulty in that bucket. Hashrate is
/// `weight × 2^32 / effective_secs`, where `effective_secs` is the full
/// bucket width for completed buckets and the elapsed span for the trailing
/// partial bucket ending at `until`.
fn points_from_rows(
    rows: Vec<(f64, Option<f64>)>,
    bucket_secs: i64,
    until: DateTime<Utc>,
) -> Result<Vec<HashratePoint>, DbError> {
    rows.into_iter()
        .map(|(epoch, weight)| {
            let secs = epoch.round();
            // `chrono` from a non-negative epoch second; share timestamps are
            // post-2024 so the cast and conversion never go negative.
            let bucket_start =
                DateTime::<Utc>::from_timestamp(secs as i64, 0).ok_or_else(|| DbError::Config {
                    message: format!("hashrate series: bucket epoch {secs} out of range"),
                })?;
            let effective = bucket_effective_secs(bucket_start, until, bucket_secs);
            let is_partial = effective < bucket_secs as f64;
            Ok(HashratePoint {
                bucket_start,
                hashrate: hashrate_hs(weight.unwrap_or(0.0), effective),
                is_partial,
            })
        })
        .collect()
}

/// Pool-wide hashrate time-series over `[from, until)`, bucketed to a
/// `bucket_secs`-second grid (aligned to the unix epoch). Empty buckets
/// are omitted; the caller zero-fills if it needs a dense series.
///
/// The caller (Phase 6 API) is responsible for bounding the span and
/// bucket count before calling; this function only rejects a
/// non-positive bucket or an empty/inverted range.
pub async fn hashrate_series_pool_wide<'e, E>(
    executor: E,
    from: DateTime<Utc>,
    until: DateTime<Utc>,
    bucket_secs: i64,
) -> Result<Vec<HashratePoint>, DbError>
where
    E: PgExecutor<'e>,
{
    validate_series_args(from, until, bucket_secs, "hashrate_series_pool_wide")?;
    let rows: Vec<(f64, Option<f64>)> = sqlx::query_as(
        "SELECT floor(extract(epoch FROM credited_at) / $3::double precision)
                    * $3::double precision AS bucket_epoch,
                sum(difficulty) AS weight
           FROM share
          WHERE credited_at >= $1
            AND credited_at <  $2
          GROUP BY bucket_epoch
          ORDER BY bucket_epoch ASC",
    )
    .bind(from)
    .bind(until)
    .bind(bucket_secs)
    .fetch_all(executor)
    .await?;
    points_from_rows(rows, bucket_secs, until)
}

/// One entry of the pool leaderboard: a wallet ranked by its summed
/// share difficulty (≈ hashrate) over the window.
#[derive(Debug, Clone, PartialEq)]
pub struct LeaderboardEntry {
    /// Miner wallet address.
    pub address: String,
    /// Network the wallet was seen on.
    pub network: String,
    /// Accepted shares in the window.
    pub accepted_shares: i64,
    /// Sum of accepted share difficulty (the PROP weight) in the window.
    pub total_weight: f64,
    /// Estimated hashrate over the window (H/s).
    pub hashrate_hs: f64,
}

/// Raw leaderboard row: address, network, accepted-share count, summed
/// difficulty, and the pool-wide earliest share in the window (identical on
/// every row; used to clamp the hashrate denominator for a young pool).
type LeaderboardRow = (String, String, i64, Option<f64>, Option<DateTime<Utc>>);

/// Top `limit` miners by summed share difficulty over `[since, until)`.
///
/// Joins `share` to `wallet` so the caller receives the address directly,
/// and computes each entry's window hashrate with the same
/// `sum(difficulty) × 2^32 / window_secs` convention as the per-wallet
/// estimate. Ordered by descending weight, ties broken by accepted-share
/// count then wallet id for a stable page. `limit` must be bounded by the
/// caller. Returns an empty vec for an idle window.
pub async fn leaderboard<'e, E>(
    executor: E,
    since: DateTime<Utc>,
    until: DateTime<Utc>,
    limit: i64,
) -> Result<Vec<LeaderboardEntry>, DbError>
where
    E: PgExecutor<'e>,
{
    if until <= since {
        return Err(DbError::Config {
            message: "leaderboard: until must be after since".to_owned(),
        });
    }
    // `pool_first` (same value every row) is the earliest share in the window
    // pool-wide; it clamps the denominator so a pool younger than the window
    // isn't divided by time it could not have mined (uniform under-reporting).
    let rows: Vec<LeaderboardRow> = sqlx::query_as(
        "SELECT w.address, w.network,
                count(*)::bigint AS accepted_shares,
                sum(s.difficulty) AS total_weight,
                (SELECT min(credited_at) FROM share
                  WHERE credited_at >= $1 AND credited_at < $2) AS pool_first
           FROM share s
           JOIN wallet w ON w.id = s.wallet_id
          WHERE s.credited_at >= $1
            AND s.credited_at <  $2
          GROUP BY w.id, w.address, w.network
          ORDER BY total_weight DESC, accepted_shares DESC, w.id ASC
          LIMIT $3",
    )
    .bind(since)
    .bind(until)
    .bind(limit)
    .fetch_all(executor)
    .await?;
    let pool_first = rows.first().and_then(|r| r.4);
    let secs = effective_window_secs(since, until, pool_first);
    Ok(rows
        .into_iter()
        .map(|(address, network, accepted_shares, weight, _pool_first)| {
            let total_weight = weight.unwrap_or(0.0);
            LeaderboardEntry {
                address,
                network,
                accepted_shares,
                total_weight,
                hashrate_hs: hashrate_hs(total_weight, secs),
            }
        })
        .collect())
}

/// One point of an active-miners time-series: the bucket start and the
/// count of distinct wallets that landed ≥ 1 accepted share in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActiveMinersPoint {
    /// Inclusive bucket start (UTC), aligned to a `bucket_secs` grid.
    pub bucket_start: DateTime<Utc>,
    /// Distinct active wallets in the bucket.
    pub miners: i64,
}

/// Distinct-active-wallet count per bucket over `[from, until)`.
///
/// Bucketed to a `bucket_secs`-second grid (aligned to the unix epoch).
/// Empty buckets are omitted; the caller zero-fills if it needs a dense
/// series. Same bounding contract as [`hashrate_series_pool_wide`]: the caller
/// caps the span/bucket count; this only rejects a non-positive bucket or
/// an empty/inverted range.
pub async fn active_wallets_series<'e, E>(
    executor: E,
    from: DateTime<Utc>,
    until: DateTime<Utc>,
    bucket_secs: i64,
) -> Result<Vec<ActiveMinersPoint>, DbError>
where
    E: PgExecutor<'e>,
{
    validate_series_args(from, until, bucket_secs, "active_wallets_series")?;
    let rows: Vec<(f64, i64)> = sqlx::query_as(
        "SELECT floor(extract(epoch FROM credited_at) / $3::double precision)
                    * $3::double precision AS bucket_epoch,
                count(DISTINCT wallet_id)::bigint AS miners
           FROM share
          WHERE credited_at >= $1
            AND credited_at <  $2
          GROUP BY bucket_epoch
          ORDER BY bucket_epoch ASC",
    )
    .bind(from)
    .bind(until)
    .bind(bucket_secs)
    .fetch_all(executor)
    .await?;
    rows.into_iter()
        .map(|(epoch, miners)| {
            let secs = epoch.round();
            let bucket_start =
                DateTime::<Utc>::from_timestamp(secs as i64, 0).ok_or_else(|| DbError::Config {
                    message: format!("active miners series: bucket epoch {secs} out of range"),
                })?;
            Ok(ActiveMinersPoint {
                bucket_start,
                miners,
            })
        })
        .collect()
}

/// Per-wallet hashrate time-series over `[from, until)`, same bucketing
/// as [`hashrate_series_pool_wide`].
pub async fn hashrate_series_for_wallet<'e, E>(
    executor: E,
    wallet_id: WalletId,
    from: DateTime<Utc>,
    until: DateTime<Utc>,
    bucket_secs: i64,
) -> Result<Vec<HashratePoint>, DbError>
where
    E: PgExecutor<'e>,
{
    validate_series_args(from, until, bucket_secs, "hashrate_series_for_wallet")?;
    let rows: Vec<(f64, Option<f64>)> = sqlx::query_as(
        "SELECT floor(extract(epoch FROM credited_at) / $4::double precision)
                    * $4::double precision AS bucket_epoch,
                sum(difficulty) AS weight
           FROM share
          WHERE wallet_id = $1
            AND credited_at >= $2
            AND credited_at <  $3
          GROUP BY bucket_epoch
          ORDER BY bucket_epoch ASC",
    )
    .bind(wallet_id.0)
    .bind(from)
    .bind(until)
    .bind(bucket_secs)
    .fetch_all(executor)
    .await?;
    points_from_rows(rows, bucket_secs, until)
}

#[cfg(test)]
mod tests {
    // Pinning exact, expected float values here is intentional — these are
    // the conversion constants, not lossy measurements.
    #![allow(clippy::float_cmp)]

    use chrono::{DateTime, Duration, Utc};

    use super::{
        HASHES_PER_DIFFICULTY, bucket_effective_secs, effective_window_secs, hashrate_hs,
        points_from_rows,
    };

    #[test]
    fn effective_secs_uses_full_window_for_a_mature_pool() {
        // Pool first share predates the window ⇒ full nominal window applies.
        let until = Utc::now();
        let since = until - Duration::hours(24);
        let pool_first = since - Duration::days(30);
        assert_eq!(
            effective_window_secs(since, until, Some(pool_first)),
            24.0 * 3600.0
        );
        // No clamp data (empty window) also leaves the full window.
        assert_eq!(effective_window_secs(since, until, None), 24.0 * 3600.0);
    }

    #[test]
    fn effective_secs_clamps_to_pool_age_for_a_young_pool() {
        // Pool only 6h old inside a 24h window ⇒ divide by 6h, not 24h, so a
        // fresh-cutover leaderboard isn't uniformly under-reported by 4x.
        let until = Utc::now();
        let since = until - Duration::hours(24);
        let pool_first = until - Duration::hours(6);
        assert_eq!(
            effective_window_secs(since, until, Some(pool_first)),
            6.0 * 3600.0
        );
    }

    #[test]
    fn effective_secs_never_below_one() {
        let t = Utc::now();
        assert_eq!(effective_window_secs(t, t, Some(t)), 1.0);
    }

    #[test]
    fn hashrate_uses_two_pow_32_per_difficulty() {
        // The convention: one difficulty-1 share == 2^32 expected hashes.
        assert_eq!(HASHES_PER_DIFFICULTY, 4_294_967_296.0);
        // 1.0 summed-difficulty over 1 s ⇒ exactly 2^32 H/s.
        assert_eq!(hashrate_hs(1.0, 1.0), HASHES_PER_DIFFICULTY);
        // Empty window ⇒ zero hashrate.
        assert_eq!(hashrate_hs(0.0, 300.0), 0.0);
    }

    #[test]
    fn hashrate_is_linear_in_weight_and_inverse_in_time() {
        let base = hashrate_hs(1.0, 1.0);
        let double = 2.0 * base;
        let half = base / 2.0;
        assert!((hashrate_hs(2.0, 1.0) - double).abs() < 1e-3);
        assert!((hashrate_hs(1.0, 2.0) - half).abs() < 1e-3);
    }

    #[test]
    fn hashrate_matches_live_tn10_sample() {
        // Ground-truth sample measured from the live tn10 DB (2026-06-05):
        // Σ(difficulty) = 394_442.79 over a 300 s window ⇒ ≈5.6 TH/s, which
        // is the single Goldshell ASIC's real stratum-side rate. Guards the
        // estimator against an order-of-magnitude regression.
        let hs = hashrate_hs(394_442.79, 300.0);
        assert!((hs - 5.6e12).abs() < 0.3e12, "expected ≈5.6 TH/s, got {hs}");
    }

    #[test]
    fn bucket_effective_secs_full_or_partial() {
        let start = DateTime::parse_from_rfc3339("2026-06-23T04:40:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let mid = DateTime::parse_from_rfc3339("2026-06-23T04:42:30Z")
            .unwrap()
            .with_timezone(&Utc);
        let end = DateTime::parse_from_rfc3339("2026-06-23T04:45:00Z")
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(bucket_effective_secs(start, end, 300), 300.0);
        assert_eq!(bucket_effective_secs(start, mid, 300), 150.0);
    }

    #[test]
    fn points_from_rows_prorates_trailing_bucket() {
        let from = DateTime::parse_from_rfc3339("2026-06-23T04:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let until = DateTime::parse_from_rfc3339("2026-06-23T04:42:30Z")
            .unwrap()
            .with_timezone(&Utc);
        let epoch = (from.timestamp() as f64 / 300.0).floor() * 300.0;
        let full_weight = 300.0;
        let partial_epoch = (until.timestamp() as f64 / 300.0).floor() * 300.0;
        let rows = vec![
            (epoch, Some(full_weight)),
            (partial_epoch, Some(full_weight / 2.0)),
        ];
        let points = points_from_rows(rows, 300, until).unwrap();
        assert_eq!(points.len(), 2);
        assert!(!points[0].is_partial);
        assert!(points[1].is_partial);
        // Half the weight over 150 s ⇒ same rate as full weight over 300 s.
        assert!(
            (points[0].hashrate - points[1].hashrate).abs() < 1.0,
            "partial bucket should match completed rate"
        );
    }
}
