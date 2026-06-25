//! API runtime configuration.
//!
//! Operationally meaningful knobs (rate limit, cache TTLs, request timeout,
//! CORS origin) are env-tunable in the same fail-fast `from_lookup` style as
//! [`katpool_db::PoolConfig`]; everything else is a centralized, documented
//! constant rather than a knob, to keep the surface small (ADR-0021, "don't
//! over-engineer").

use std::time::Duration;

/// Maximum accepted request body, in bytes. All endpoints are `GET` and
/// carry no body; this is belt-and-suspenders against abuse.
pub const MAX_BODY_BYTES: usize = 16 * 1024;

/// Default sliding window for `?window=` endpoints (hashrate, rejects).
pub const DEFAULT_WINDOW: Duration = Duration::from_secs(600);

/// Sliding window for live headline hashrate.
///
/// Shorter than [`DEFAULT_WINDOW`] so the figure reacts within a few minutes
/// without the bucket-quantization lag of a time-series tail point.
pub const LIVE_WINDOW: Duration = Duration::from_secs(300);

/// Maximum accepted `?window=` value.
pub const MAX_WINDOW: Duration = Duration::from_secs(24 * 60 * 60);

/// Default page size for keyset-paginated list endpoints.
pub const DEFAULT_PAGE_SIZE: i64 = 25;

/// Maximum accepted `limit` for keyset-paginated list endpoints.
pub const MAX_PAGE_SIZE: i64 = 100;

/// Maximum number of buckets a single time-series request may span. Bounds
/// both the query cost and the response size.
pub const MAX_SERIES_POINTS: i64 = 1_000;

/// Maximum number of entries either cache will hold before LRU eviction.
pub const CACHE_MAX_ENTRIES: u64 = 10_000;

/// Operator-tunable API configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiConfig {
    /// Sustained per-IP request rate (tokens refilled per second).
    pub rate_per_second: u64,
    /// Per-IP burst allowance (token-bucket capacity).
    pub rate_burst: u32,
    /// Hard per-request timeout (HTTP-layer backstop above the DB
    /// `statement_timeout`).
    pub request_timeout: Duration,
    /// TTL for pool-wide aggregate/series cache entries.
    pub pool_cache_ttl: Duration,
    /// TTL for per-wallet cache entries (shorter — fresher balances).
    pub wallet_cache_ttl: Duration,
    /// Allowed CORS origin. `None` (default) installs no CORS layer, so the
    /// browser same-origin policy applies (dashboard served same-origin
    /// behind nginx).
    pub cors_allow_origin: Option<String>,
    /// Display metadata for the legacy-compatible `MiningPoolStats` feed
    /// (`GET /api/pool/miningPoolStats`). These drive the public aggregator
    /// listing and must stay accurate across the cutover.
    pub mps_pool_name: String,
    /// Pool website shown in the listing (host only, no scheme).
    pub mps_url: String,
    /// Two-letter country code for the listing flag.
    pub mps_country: String,
    /// Payout scheme label shown in the listing (the engine is PROP).
    pub mps_fee_type: String,
    /// Topline fee in basis points; the feed reports `bps / 100` as a percent.
    pub mps_fee_bps: u32,
    /// Minimum payout, in whole KAS, shown in the listing.
    pub mps_min_pay_kas: i64,
    /// Pool coinbase/treasury address echoed per block in the feed (the first
    /// `KATPOOL_POOL_ADDRESS`); empty if unset.
    pub mps_pool_address: String,
    /// Advertisement image URL echoed in the feed. NOTE: `MiningPoolStats` does
    /// not render this (verified) — retained only for legacy shape parity.
    pub mps_ad_image_link: String,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            rate_per_second: 5,
            rate_burst: 20,
            request_timeout: Duration::from_secs(10),
            pool_cache_ttl: Duration::from_secs(10),
            wallet_cache_ttl: Duration::from_secs(5),
            cors_allow_origin: None,
            mps_pool_name: "Kat Pool".to_owned(),
            mps_url: "app.katpool.com".to_owned(),
            mps_country: "US".to_owned(),
            mps_fee_type: "PROP".to_owned(),
            // MiningPoolStats `poolFee` display (0.5%). Not the allocation fee.
            mps_fee_bps: 50,
            mps_min_pay_kas: 10,
            mps_pool_address: String::new(),
            mps_ad_image_link: "https://app.katpool.xyz/images/katpoolad.gif".to_owned(),
        }
    }
}

impl ApiConfig {
    /// Pure environment-style lookup. Mirrors [`katpool_db::PoolConfig::from_lookup`].
    ///
    /// Recognised keys (all optional; defaults apply when absent):
    ///
    /// | Key | Type |
    /// |---|---|
    /// | `KATPOOL_API_RATE_PER_SECOND` | `u64` |
    /// | `KATPOOL_API_RATE_BURST` | `u32` |
    /// | `KATPOOL_API_REQUEST_TIMEOUT_SECS` | `u64` |
    /// | `KATPOOL_API_POOL_CACHE_TTL_SECS` | `u64` |
    /// | `KATPOOL_API_WALLET_CACHE_TTL_SECS` | `u64` |
    /// | `KATPOOL_API_CORS_ALLOW_ORIGIN` | `text` |
    ///
    /// # Errors
    /// Returns a human-readable message if any present value fails to parse
    /// or violates an invariant (zero rate/burst, zero timeout).
    pub fn from_lookup<F>(lookup: F) -> Result<Self, String>
    where
        F: Fn(&str) -> Option<String>,
    {
        let mut cfg = Self::default();

        if let Some(raw) = lookup("KATPOOL_API_RATE_PER_SECOND") {
            cfg.rate_per_second = parse(&raw, "KATPOOL_API_RATE_PER_SECOND")?;
        }
        if let Some(raw) = lookup("KATPOOL_API_RATE_BURST") {
            cfg.rate_burst = parse(&raw, "KATPOOL_API_RATE_BURST")?;
        }
        if let Some(raw) = lookup("KATPOOL_API_REQUEST_TIMEOUT_SECS") {
            cfg.request_timeout =
                Duration::from_secs(parse(&raw, "KATPOOL_API_REQUEST_TIMEOUT_SECS")?);
        }
        if let Some(raw) = lookup("KATPOOL_API_POOL_CACHE_TTL_SECS") {
            cfg.pool_cache_ttl =
                Duration::from_secs(parse(&raw, "KATPOOL_API_POOL_CACHE_TTL_SECS")?);
        }
        if let Some(raw) = lookup("KATPOOL_API_WALLET_CACHE_TTL_SECS") {
            cfg.wallet_cache_ttl =
                Duration::from_secs(parse(&raw, "KATPOOL_API_WALLET_CACHE_TTL_SECS")?);
        }
        if let Some(raw) = lookup("KATPOOL_API_CORS_ALLOW_ORIGIN") {
            let trimmed = raw.trim();
            if !trimmed.is_empty() {
                cfg.cors_allow_origin = Some(trimmed.to_owned());
            }
        }

        // ----- MiningPoolStats listing metadata (all optional) -------------
        let set_str = |slot: &mut String, raw: Option<String>| {
            if let Some(v) = raw {
                let t = v.trim();
                if !t.is_empty() {
                    t.clone_into(slot);
                }
            }
        };
        set_str(&mut cfg.mps_pool_name, lookup("KATPOOL_MPS_POOL_NAME"));
        set_str(&mut cfg.mps_url, lookup("KATPOOL_MPS_URL"));
        set_str(&mut cfg.mps_country, lookup("KATPOOL_MPS_COUNTRY"));
        set_str(&mut cfg.mps_fee_type, lookup("KATPOOL_MPS_FEE_TYPE"));
        set_str(
            &mut cfg.mps_ad_image_link,
            lookup("KATPOOL_MPS_AD_IMAGE_LINK"),
        );
        // Display-only fee for the MiningPoolStats aggregator listing (`poolFee`).
        // Independent of `KATPOOL_FEE_TOPLINE_BPS`, which drives real allocations.
        if let Some(raw) = lookup("KATPOOL_MPS_FEE_BPS") {
            cfg.mps_fee_bps = parse(&raw, "KATPOOL_MPS_FEE_BPS")?;
        }
        if let Some(raw) = lookup("KATPOOL_MPS_MIN_PAY_KAS") {
            cfg.mps_min_pay_kas = parse(&raw, "KATPOOL_MPS_MIN_PAY_KAS")?;
        }
        // The per-block pool_address echoes the runtime coinbase address; the
        // bridge's coinbase override uses the first of a comma-separated list.
        if let Some(raw) = lookup("KATPOOL_POOL_ADDRESS") {
            let first = raw.split(',').next().unwrap_or(&raw).trim();
            if !first.is_empty() {
                first.clone_into(&mut cfg.mps_pool_address);
            }
        }

        cfg.validate()?;
        Ok(cfg)
    }

    /// Production wrapper over the process environment.
    ///
    /// # Errors
    /// See [`ApiConfig::from_lookup`].
    pub fn from_env() -> Result<Self, String> {
        Self::from_lookup(|k| std::env::var(k).ok())
    }

    fn validate(&self) -> Result<(), String> {
        if self.rate_per_second == 0 {
            return Err("KATPOOL_API_RATE_PER_SECOND must be > 0".to_owned());
        }
        if self.rate_burst == 0 {
            return Err("KATPOOL_API_RATE_BURST must be > 0".to_owned());
        }
        if self.request_timeout.is_zero() {
            return Err("KATPOOL_API_REQUEST_TIMEOUT_SECS must be > 0".to_owned());
        }
        Ok(())
    }
}

fn parse<T>(raw: &str, key: &str) -> Result<T, String>
where
    T: std::str::FromStr,
{
    raw.parse::<T>()
        .map_err(|_| format!("`{key}` is not a valid value: `{raw}`"))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn defaults_validate() {
        ApiConfig::default().validate().expect("defaults valid");
    }

    #[test]
    fn empty_lookup_yields_defaults() {
        assert_eq!(
            ApiConfig::from_lookup(|_| None).unwrap(),
            ApiConfig::default()
        );
    }

    #[test]
    fn overrides_apply() {
        let map: HashMap<&str, &str> = [
            ("KATPOOL_API_RATE_PER_SECOND", "10"),
            ("KATPOOL_API_RATE_BURST", "40"),
            ("KATPOOL_API_REQUEST_TIMEOUT_SECS", "8"),
            ("KATPOOL_API_POOL_CACHE_TTL_SECS", "15"),
            ("KATPOOL_API_WALLET_CACHE_TTL_SECS", "3"),
            ("KATPOOL_API_CORS_ALLOW_ORIGIN", "https://dash.example.com"),
        ]
        .into_iter()
        .collect();
        let cfg = ApiConfig::from_lookup(|k| map.get(k).map(|s| (*s).to_owned())).unwrap();
        assert_eq!(cfg.rate_per_second, 10);
        assert_eq!(cfg.rate_burst, 40);
        assert_eq!(cfg.request_timeout, Duration::from_secs(8));
        assert_eq!(cfg.pool_cache_ttl, Duration::from_secs(15));
        assert_eq!(cfg.wallet_cache_ttl, Duration::from_secs(3));
        assert_eq!(
            cfg.cors_allow_origin.as_deref(),
            Some("https://dash.example.com")
        );
    }

    #[test]
    fn rejects_zero_rate() {
        let err = ApiConfig::from_lookup(|k| {
            (k == "KATPOOL_API_RATE_PER_SECOND").then(|| "0".to_owned())
        })
        .unwrap_err();
        assert!(err.contains("RATE_PER_SECOND"), "{err}");
    }

    #[test]
    fn rejects_unparsable() {
        let err =
            ApiConfig::from_lookup(|k| (k == "KATPOOL_API_RATE_BURST").then(|| "x".to_owned()))
                .unwrap_err();
        assert!(err.contains("RATE_BURST"), "{err}");
    }

    #[test]
    fn blank_cors_origin_stays_none() {
        let cfg = ApiConfig::from_lookup(|k| {
            (k == "KATPOOL_API_CORS_ALLOW_ORIGIN").then(|| "   ".to_owned())
        })
        .unwrap();
        assert_eq!(cfg.cors_allow_origin, None);
    }

    #[test]
    fn mps_fee_bps_independent_of_topline() {
        let cfg = ApiConfig::from_lookup(|k| match k {
            "KATPOOL_FEE_TOPLINE_BPS" => Some("75".to_owned()),
            "KATPOOL_MPS_FEE_BPS" => Some("50".to_owned()),
            _ => None,
        })
        .unwrap();
        assert_eq!(cfg.mps_fee_bps, 50);
    }

    #[test]
    fn mps_fee_bps_defaults_to_half_percent() {
        let cfg = ApiConfig::from_lookup(|_| None).unwrap();
        assert_eq!(cfg.mps_fee_bps, 50);
    }
}
