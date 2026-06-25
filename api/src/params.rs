//! Query/path parameter parsing and bounding.
//!
//! Every externally-supplied value is validated and clamped here before it
//! reaches a repo function: window/limit are bounded, time-series spans are
//! capped to [`MAX_SERIES_POINTS`] buckets, and addresses are parsed through
//! the domain newtype. Invalid input yields a `400` with a safe message.

use std::time::Duration;

use chrono::{DateTime, Utc};
use katpool_domain::WalletAddress;
use serde::Deserialize;

use crate::config::{
    DEFAULT_PAGE_SIZE, DEFAULT_WINDOW, MAX_PAGE_SIZE, MAX_SERIES_POINTS, MAX_WINDOW,
};
use crate::error::ApiError;

/// Time-series bucket width. Closed enum (not free-form seconds) so the
/// grid is fixed and the cache key space is small (ADR-0021).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bucket {
    /// One minute.
    OneMinute,
    /// Five minutes.
    FiveMinutes,
    /// One hour.
    OneHour,
    /// One day.
    OneDay,
}

impl Bucket {
    /// Bucket width in seconds.
    #[must_use]
    pub const fn seconds(self) -> i64 {
        match self {
            Self::OneMinute => 60,
            Self::FiveMinutes => 300,
            Self::OneHour => 3_600,
            Self::OneDay => 86_400,
        }
    }

    /// Parse the wire token (`1m`/`5m`/`1h`/`1d`).
    fn parse(raw: &str) -> Result<Self, ApiError> {
        match raw {
            "1m" => Ok(Self::OneMinute),
            "5m" => Ok(Self::FiveMinutes),
            "1h" => Ok(Self::OneHour),
            "1d" => Ok(Self::OneDay),
            other => Err(ApiError::bad_request(format!(
                "invalid bucket `{other}`: expected one of 1m, 5m, 1h, 1d"
            ))),
        }
    }
}

/// `?window=<secs>` for sliding-window endpoints (hashrate, rejects).
#[derive(Debug, Default, Deserialize)]
pub struct WindowParams {
    /// Window length in seconds.
    pub window: Option<u64>,
}

/// Resolve a window: default [`DEFAULT_WINDOW`], capped at [`MAX_WINDOW`],
/// rejecting zero.
pub fn window(params: &WindowParams) -> Result<Duration, ApiError> {
    match params.window {
        None => Ok(DEFAULT_WINDOW),
        Some(0) => Err(ApiError::bad_request("window must be > 0")),
        Some(secs) => {
            let requested = Duration::from_secs(secs);
            Ok(requested.min(MAX_WINDOW))
        }
    }
}

/// `?window=&limit=` for the leaderboard (a windowed, top-N read).
#[derive(Debug, Default, Deserialize)]
pub struct LeaderboardParams {
    /// Window length in seconds (same bounds as [`WindowParams`]).
    pub window: Option<u64>,
    /// Number of ranked entries; clamped to `[1, MAX_PAGE_SIZE]`.
    pub limit: Option<i64>,
}

/// Resolve the leaderboard window (default/capped like [`window`]) and a
/// clamped entry limit.
pub fn leaderboard(params: &LeaderboardParams) -> Result<(Duration, i64), ApiError> {
    let win = window(&WindowParams {
        window: params.window,
    })?;
    let limit = params
        .limit
        .unwrap_or(DEFAULT_PAGE_SIZE)
        .clamp(1, MAX_PAGE_SIZE);
    Ok((win, limit))
}

/// Keyset pagination query: `?limit=&before=`.
#[derive(Debug, Default, Deserialize)]
pub struct PageParams {
    /// Page size; clamped to `[1, MAX_PAGE_SIZE]`.
    pub limit: Option<i64>,
    /// Exclusive upper-bound id cursor (the previous page's smallest id).
    pub before: Option<i64>,
}

/// A validated page request: a clamped limit and an optional positive cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Page {
    /// Clamped page size.
    pub limit: i64,
    /// Optional exclusive id cursor.
    pub before_id: Option<i64>,
}

/// Resolve pagination: default [`DEFAULT_PAGE_SIZE`], clamp to
/// `[1, MAX_PAGE_SIZE]`, and reject a non-positive `before` cursor.
pub fn page(params: &PageParams) -> Result<Page, ApiError> {
    let limit = params
        .limit
        .unwrap_or(DEFAULT_PAGE_SIZE)
        .clamp(1, MAX_PAGE_SIZE);
    if params.before.is_some_and(|before| before <= 0) {
        return Err(ApiError::bad_request("before cursor must be a positive id"));
    }
    Ok(Page {
        limit,
        before_id: params.before,
    })
}

/// Time-range + bucket query: `?from=&to=&bucket=` (RFC3339 timestamps).
#[derive(Debug, Default, Deserialize)]
pub struct RangeParams {
    /// Inclusive lower bound (RFC3339). Default: `to - 24h`.
    pub from: Option<String>,
    /// Exclusive upper bound (RFC3339). Default: now.
    pub to: Option<String>,
    /// Bucket token (`1m`/`5m`/`1h`/`1d`). Default: `1h`.
    pub bucket: Option<String>,
}

/// A validated, bounded time-series request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    /// Inclusive start (UTC).
    pub from: DateTime<Utc>,
    /// Exclusive end (UTC).
    pub until: DateTime<Utc>,
    /// Bucket width.
    pub bucket: Bucket,
}

/// Default range span when `from` is omitted.
const DEFAULT_RANGE_SECS: i64 = 24 * 60 * 60;

/// Resolve and bound a time-series request.
///
/// Defaults: `to = now`, `from = to - 24h`, `bucket = 1h`. Rejects an
/// inverted/empty range, a non-RFC3339 timestamp, and any request whose
/// bucket count would exceed [`MAX_SERIES_POINTS`].
#[allow(clippy::integer_division)] // bucket-count bound; exact floor is intended
pub fn range(params: &RangeParams) -> Result<Range, ApiError> {
    let until = match &params.to {
        Some(raw) => parse_rfc3339(raw, "to")?,
        None => Utc::now(),
    };
    let from = match &params.from {
        Some(raw) => parse_rfc3339(raw, "from")?,
        None => until - chrono::Duration::seconds(DEFAULT_RANGE_SECS),
    };
    let bucket = match &params.bucket {
        Some(raw) => Bucket::parse(raw)?,
        None => Bucket::OneHour,
    };

    if until <= from {
        return Err(ApiError::bad_request("`to` must be after `from`"));
    }
    let span_secs = (until - from).num_seconds();
    let points = span_secs / bucket.seconds();
    if points > MAX_SERIES_POINTS {
        return Err(ApiError::bad_request(format!(
            "range too large: {points} buckets exceeds the {MAX_SERIES_POINTS} maximum; widen the bucket or narrow the range"
        )));
    }
    Ok(Range {
        from,
        until,
        bucket,
    })
}

fn parse_rfc3339(raw: &str, field: &str) -> Result<DateTime<Utc>, ApiError> {
    DateTime::parse_from_rfc3339(raw)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|_| ApiError::bad_request(format!("`{field}` must be an RFC3339 timestamp")))
}

/// Parse a path `:address` segment through the domain newtype. Returns a
/// safe `400` (without echoing the raw input) on any validation failure.
pub fn parse_address(raw: &str) -> Result<WalletAddress, ApiError> {
    WalletAddress::new(raw).map_err(|_| ApiError::bad_request("invalid wallet address"))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn window_defaults_and_caps() {
        assert_eq!(
            window(&WindowParams { window: None }).unwrap(),
            DEFAULT_WINDOW
        );
        assert_eq!(
            window(&WindowParams {
                window: Some(10_000_000)
            })
            .unwrap(),
            MAX_WINDOW
        );
        assert!(window(&WindowParams { window: Some(0) }).is_err());
    }

    #[test]
    fn page_clamps_limit_and_rejects_bad_cursor() {
        assert_eq!(
            page(&PageParams::default()).unwrap().limit,
            DEFAULT_PAGE_SIZE
        );
        assert_eq!(
            page(&PageParams {
                limit: Some(10_000),
                before: None
            })
            .unwrap()
            .limit,
            MAX_PAGE_SIZE
        );
        assert_eq!(
            page(&PageParams {
                limit: Some(0),
                before: None
            })
            .unwrap()
            .limit,
            1
        );
        assert!(
            page(&PageParams {
                limit: None,
                before: Some(0)
            })
            .is_err()
        );
    }

    #[test]
    fn bucket_tokens_parse() {
        assert_eq!(Bucket::parse("1m").unwrap().seconds(), 60);
        assert_eq!(Bucket::parse("5m").unwrap().seconds(), 300);
        assert_eq!(Bucket::parse("1h").unwrap().seconds(), 3_600);
        assert_eq!(Bucket::parse("1d").unwrap().seconds(), 86_400);
        assert!(Bucket::parse("7m").is_err());
    }

    #[test]
    fn range_defaults_to_last_24h_hourly() {
        let r = range(&RangeParams::default()).unwrap();
        assert_eq!(r.bucket, Bucket::OneHour);
        assert_eq!((r.until - r.from).num_seconds(), DEFAULT_RANGE_SECS);
    }

    #[test]
    fn range_rejects_too_many_buckets() {
        let err = range(&RangeParams {
            from: Some("2024-01-01T00:00:00Z".to_owned()),
            to: Some("2025-01-01T00:00:00Z".to_owned()),
            bucket: Some("1m".to_owned()),
        })
        .unwrap_err();
        assert!(matches!(err, ApiError::BadRequest(_)));
    }

    #[test]
    fn range_rejects_inverted() {
        assert!(
            range(&RangeParams {
                from: Some("2025-01-02T00:00:00Z".to_owned()),
                to: Some("2025-01-01T00:00:00Z".to_owned()),
                bucket: None,
            })
            .is_err()
        );
    }

    #[test]
    fn range_rejects_bad_timestamp() {
        assert!(
            range(&RangeParams {
                from: Some("not-a-time".to_owned()),
                to: None,
                bucket: None,
            })
            .is_err()
        );
    }

    #[test]
    fn address_validation_rejects_garbage() {
        assert!(parse_address("not-an-address").is_err());
    }
}
