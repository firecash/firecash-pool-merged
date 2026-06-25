//! Per-IP anti-abuse guard for the stratum listener.
//!
//! Provides two protection mechanisms applied at TCP-accept and
//! frame-parse time:
//!
//! 1. **Connection cap** — bound on concurrent connections per source
//!    IP. Stops "open 10k sockets from one IP" floods.
//! 2. **Frame-rate token bucket** — bound on parsed stratum frames per
//!    second per source IP. Stops a single connection from CPU-flooding
//!    the parser/handler with junk frames.
//!
//! Both checks are O(1) on the hot path and time-injected for testability
//! (every method that consults the wall clock takes an `Instant`
//! argument; the caller passes `Instant::now()`).
//!
//! ## Memory model
//!
//! State is held in a single `Mutex<HashMap<IpAddr, IpEntry>>`. An IP's
//! entry is created on first accept and removed when its connection
//! count returns to zero, so steady-state memory is bounded by the
//! number of currently-connected unique IPs. A configurable
//! [`AntiAbuseConfig::max_tracked_ips`] cap rejects new accepts when the
//! map is saturated, preventing memory exhaustion under attack.
//!
//! ## Lifecycle
//!
//! ```ignore
//! let guard = AntiAbuseGuard::new(AntiAbuseConfig::default());
//! // At accept:
//! match guard.try_accept_connection(ip, Instant::now()) {
//!     Ok(ticket) => { /* spawn task; hold `ticket` until disconnect */ }
//!     Err(_) => { /* drop the socket */ }
//! }
//! // At each parsed frame:
//! if !guard.try_consume_frame(ip, Instant::now()) {
//!     /* rate-limited — disconnect */
//! }
//! ```
//!
//! The [`ConnTicket`] is RAII: dropping it decrements the connection
//! count atomically, so callers cannot forget to release.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use thiserror::Error;

/// Configuration knobs for the anti-abuse guard.
///
/// All fields are validated at construction by [`AntiAbuseConfig::new`].
/// Zero/negative/non-finite values are rejected.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AntiAbuseConfig {
    /// Maximum concurrent TCP connections from a single source IP.
    /// Large mining farms behind a single NAT can have many ASICs, so
    /// this is set generously by default.
    pub max_conn_per_ip: u32,

    /// Maximum unique source IPs the guard will track simultaneously.
    /// Caps `HashMap` memory under attack. Connection attempts from a
    /// new IP when the map is full are rejected.
    pub max_tracked_ips: usize,

    /// Sustained allowed stratum frames per second per source IP, after
    /// the initial burst is consumed.
    pub frame_rate_per_sec: f64,

    /// Maximum tokens (frames) that can accumulate in the bucket, i.e.
    /// the largest legal short-term burst.
    pub frame_burst: f64,
}

impl AntiAbuseConfig {
    /// Production-grade defaults. Reasonably permissive — legitimate
    /// stratum traffic from a healthy ASIC is ~1–5 frames/sec; these
    /// limits leave large headroom while still rejecting obvious
    /// flooding patterns.
    #[must_use]
    pub const fn production() -> Self {
        Self { max_conn_per_ip: 256, max_tracked_ips: 65_536, frame_rate_per_sec: 100.0, frame_burst: 200.0 }
    }

    /// Effectively-disabled limits — useful for development/integration
    /// tests that exercise the bridge in legacy standalone mode.
    #[must_use]
    pub const fn unlimited() -> Self {
        Self { max_conn_per_ip: u32::MAX, max_tracked_ips: usize::MAX, frame_rate_per_sec: f64::MAX, frame_burst: f64::MAX }
    }

    /// Validated constructor. Returns [`AntiAbuseConfigError`] when any
    /// field is non-positive or non-finite.
    pub fn new(
        max_conn_per_ip: u32,
        max_tracked_ips: usize,
        frame_rate_per_sec: f64,
        frame_burst: f64,
    ) -> Result<Self, AntiAbuseConfigError> {
        if max_conn_per_ip == 0 {
            return Err(AntiAbuseConfigError::ZeroConnCap);
        }
        if max_tracked_ips == 0 {
            return Err(AntiAbuseConfigError::ZeroIpCap);
        }
        if !frame_rate_per_sec.is_finite() || frame_rate_per_sec <= 0.0 {
            return Err(AntiAbuseConfigError::InvalidFrameRate { value: frame_rate_per_sec });
        }
        if !frame_burst.is_finite() || frame_burst <= 0.0 {
            return Err(AntiAbuseConfigError::InvalidFrameBurst { value: frame_burst });
        }
        Ok(Self { max_conn_per_ip, max_tracked_ips, frame_rate_per_sec, frame_burst })
    }

    /// Pure environment-style lookup. Reads values via the supplied
    /// `lookup` closure (which production callers wire to
    /// [`std::env::var`]). Missing values fall back to
    /// [`AntiAbuseConfig::production`]; malformed values produce a
    /// [`AntiAbuseConfigError::InvalidEnvValue`].
    ///
    /// Recognised keys:
    ///
    /// | Key | Type | Default |
    /// |---|---|---|
    /// | `KATPOOL_ANTI_ABUSE_MAX_CONN_PER_IP` | `u32` | `256` |
    /// | `KATPOOL_ANTI_ABUSE_MAX_TRACKED_IPS` | `usize` | `65_536` |
    /// | `KATPOOL_ANTI_ABUSE_FRAME_RATE_PER_SEC` | `f64` | `100.0` |
    /// | `KATPOOL_ANTI_ABUSE_FRAME_BURST` | `f64` | `200.0` |
    pub fn from_lookup<F>(lookup: F) -> Result<Self, AntiAbuseConfigError>
    where
        F: Fn(&str) -> Option<String>,
    {
        let defaults = Self::production();
        let max_conn_per_ip = parse_env_u32(&lookup, "KATPOOL_ANTI_ABUSE_MAX_CONN_PER_IP", defaults.max_conn_per_ip)?;
        let max_tracked_ips = parse_env_usize(&lookup, "KATPOOL_ANTI_ABUSE_MAX_TRACKED_IPS", defaults.max_tracked_ips)?;
        let frame_rate_per_sec = parse_env_f64(&lookup, "KATPOOL_ANTI_ABUSE_FRAME_RATE_PER_SEC", defaults.frame_rate_per_sec)?;
        let frame_burst = parse_env_f64(&lookup, "KATPOOL_ANTI_ABUSE_FRAME_BURST", defaults.frame_burst)?;
        Self::new(max_conn_per_ip, max_tracked_ips, frame_rate_per_sec, frame_burst)
    }

    /// Convenience wrapper that reads from the real process
    /// environment. Calls [`AntiAbuseConfig::from_lookup`] with
    /// `std::env::var`-backed lookup.
    pub fn from_env() -> Result<Self, AntiAbuseConfigError> {
        Self::from_lookup(|key| std::env::var(key).ok())
    }
}

fn parse_env_u32<F>(lookup: &F, key: &str, default: u32) -> Result<u32, AntiAbuseConfigError>
where
    F: Fn(&str) -> Option<String>,
{
    match lookup(key) {
        Some(raw) => raw.parse::<u32>().map_err(|_| AntiAbuseConfigError::InvalidEnvValue { key: key.to_owned(), value: raw }),
        None => Ok(default),
    }
}

fn parse_env_usize<F>(lookup: &F, key: &str, default: usize) -> Result<usize, AntiAbuseConfigError>
where
    F: Fn(&str) -> Option<String>,
{
    match lookup(key) {
        Some(raw) => raw.parse::<usize>().map_err(|_| AntiAbuseConfigError::InvalidEnvValue { key: key.to_owned(), value: raw }),
        None => Ok(default),
    }
}

fn parse_env_f64<F>(lookup: &F, key: &str, default: f64) -> Result<f64, AntiAbuseConfigError>
where
    F: Fn(&str) -> Option<String>,
{
    match lookup(key) {
        Some(raw) => raw.parse::<f64>().map_err(|_| AntiAbuseConfigError::InvalidEnvValue { key: key.to_owned(), value: raw }),
        None => Ok(default),
    }
}

impl Default for AntiAbuseConfig {
    fn default() -> Self {
        Self::production()
    }
}

/// Errors from [`AntiAbuseConfig::new`].
#[derive(Debug, Error, PartialEq)]
pub enum AntiAbuseConfigError {
    /// `max_conn_per_ip` was zero.
    #[error("max_conn_per_ip must be > 0")]
    ZeroConnCap,
    /// `max_tracked_ips` was zero.
    #[error("max_tracked_ips must be > 0")]
    ZeroIpCap,
    /// `frame_rate_per_sec` was zero, negative, NaN, or infinite.
    #[error("frame_rate_per_sec must be finite and > 0 (got {value})")]
    InvalidFrameRate {
        /// Offending value.
        value: f64,
    },
    /// `frame_burst` was zero, negative, NaN, or infinite.
    #[error("frame_burst must be finite and > 0 (got {value})")]
    InvalidFrameBurst {
        /// Offending value.
        value: f64,
    },
    /// Environment variable was present but could not be parsed as the
    /// expected numeric type.
    #[error("environment variable `{key}` has invalid value `{value}`")]
    InvalidEnvValue {
        /// Name of the offending variable.
        key: String,
        /// Raw string value as read from the environment.
        value: String,
    },
}

#[derive(Debug)]
struct IpEntry {
    conn_count: u32,
    /// Tokens in the bucket. Refilled lazily on every check.
    tokens: f64,
    /// Last time `tokens` was refilled.
    last_refill: Instant,
}

impl IpEntry {
    fn new(burst: f64, now: Instant) -> Self {
        Self { conn_count: 0, tokens: burst, last_refill: now }
    }

    /// Refills the bucket based on elapsed time, then attempts to
    /// consume one token. Returns `true` on success.
    fn try_consume(&mut self, now: Instant, rate: f64, burst: f64) -> bool {
        let elapsed = now.saturating_duration_since(self.last_refill).as_secs_f64();
        let refill = elapsed * rate;
        self.tokens = (self.tokens + refill).min(burst);
        self.last_refill = now;
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

/// The shared, thread-safe guard. Construct once at bridge start-up and
/// hand out clones (it is internally `Arc`-shared via the `Mutex`).
#[derive(Debug)]
pub struct AntiAbuseGuard {
    config: AntiAbuseConfig,
    state: Mutex<HashMap<IpAddr, IpEntry>>,
}

impl AntiAbuseGuard {
    /// Build a new guard with the given configuration.
    #[must_use]
    pub fn new(config: AntiAbuseConfig) -> Self {
        Self { config, state: Mutex::new(HashMap::new()) }
    }

    /// Wrap in an `Arc` for sharing across the listener and any other
    /// component that needs to query/record.
    #[must_use]
    pub fn into_arc(self) -> Arc<Self> {
        Arc::new(self)
    }

    /// Read-only access to the active configuration (useful for tests
    /// and metrics labels).
    #[must_use]
    pub const fn config(&self) -> AntiAbuseConfig {
        self.config
    }

    /// Snapshot the current connection count for `ip`. Returns zero if
    /// the IP is not tracked. Test/observability use only.
    #[must_use]
    pub fn conn_count(&self, ip: IpAddr) -> u32 {
        self.state.lock().get(&ip).map_or(0, |e| e.conn_count)
    }

    /// Snapshot the number of tracked IPs (for observability).
    #[must_use]
    pub fn tracked_ip_count(&self) -> usize {
        self.state.lock().len()
    }

    /// Try to record a new TCP-accept from `ip`. On success the
    /// connection count is incremented and a [`ConnTicket`] is returned;
    /// the ticket's `Drop` impl decrements the count, so callers cannot
    /// leak. On failure the count is unchanged.
    pub fn try_accept_connection(self: &Arc<Self>, ip: IpAddr, now: Instant) -> Result<ConnTicket, AntiAbuseRejection> {
        let mut state = self.state.lock();
        let known = state.contains_key(&ip);
        if !known && state.len() >= self.config.max_tracked_ips {
            return Err(AntiAbuseRejection::IpCapReached);
        }
        let entry = state.entry(ip).or_insert_with(|| IpEntry::new(self.config.frame_burst, now));
        if entry.conn_count >= self.config.max_conn_per_ip {
            return Err(AntiAbuseRejection::ConnCapPerIp { active: entry.conn_count, cap: self.config.max_conn_per_ip });
        }
        entry.conn_count += 1;
        Ok(ConnTicket { guard: Arc::clone(self), ip })
    }

    /// Try to consume one frame's worth of token budget for `ip`. Must
    /// be called *after* a successful `try_accept_connection` — frames
    /// from an untracked IP are conservatively rejected.
    #[must_use]
    pub fn try_consume_frame(&self, ip: IpAddr, now: Instant) -> bool {
        let mut state = self.state.lock();
        match state.get_mut(&ip) {
            Some(entry) => entry.try_consume(now, self.config.frame_rate_per_sec, self.config.frame_burst),
            None => false,
        }
    }

    /// Internal release path used by the [`ConnTicket`] drop. Decrements
    /// the connection count and removes the entry when it reaches zero
    /// (so per-IP memory is reclaimed on disconnect).
    fn release(&self, ip: IpAddr) {
        let mut state = self.state.lock();
        if let Some(entry) = state.get_mut(&ip) {
            entry.conn_count = entry.conn_count.saturating_sub(1);
            if entry.conn_count == 0 {
                state.remove(&ip);
            }
        }
    }
}

/// RAII handle returned by [`AntiAbuseGuard::try_accept_connection`].
/// Holding a ticket reserves one slot in the per-IP connection counter;
/// dropping it releases the slot.
///
/// Tickets are non-cloneable on purpose — every accept gets exactly one
/// ticket, every disconnect releases exactly one ticket.
#[derive(Debug)]
pub struct ConnTicket {
    guard: Arc<AntiAbuseGuard>,
    ip: IpAddr,
}

impl ConnTicket {
    /// The IP this ticket pertains to.
    #[must_use]
    pub const fn ip(&self) -> IpAddr {
        self.ip
    }
}

impl Drop for ConnTicket {
    fn drop(&mut self) {
        self.guard.release(self.ip);
    }
}

/// Reason a connection accept was rejected.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum AntiAbuseRejection {
    /// Per-IP connection cap reached.
    #[error("per-ip connection cap reached: {active}/{cap}")]
    ConnCapPerIp {
        /// Current active connections for the offending IP.
        active: u32,
        /// Configured cap.
        cap: u32,
    },
    /// Total tracked-IP cap reached (memory protection).
    #[error("tracked-ip cap reached; rejecting new ip")]
    IpCapReached,
}

impl AntiAbuseRejection {
    /// Stable label string for metrics.
    #[must_use]
    pub const fn metric_label(&self) -> &'static str {
        match self {
            Self::ConnCapPerIp { .. } => "conn_cap_per_ip",
            Self::IpCapReached => "ip_cap_reached",
        }
    }
}

/// Approximate seconds-from-now to refill an empty bucket to a single
/// token at the configured rate. Useful for jittered reconnect hints in
/// logs and ban-list TTLs.
#[must_use]
pub fn refill_eta(config: AntiAbuseConfig) -> Duration {
    let secs = (1.0_f64 / config.frame_rate_per_sec).max(0.001);
    Duration::from_secs_f64(secs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;
    use std::time::Duration;

    fn ip(a: u8, b: u8, c: u8, d: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(a, b, c, d))
    }

    fn t0() -> Instant {
        Instant::now()
    }

    #[test]
    fn config_rejects_zero_conn_cap() {
        assert_eq!(AntiAbuseConfig::new(0, 10, 10.0, 20.0), Err(AntiAbuseConfigError::ZeroConnCap));
    }

    #[test]
    fn config_rejects_zero_ip_cap() {
        assert_eq!(AntiAbuseConfig::new(8, 0, 10.0, 20.0), Err(AntiAbuseConfigError::ZeroIpCap));
    }

    #[test]
    fn config_rejects_non_finite_rate() {
        assert!(matches!(AntiAbuseConfig::new(8, 10, f64::NAN, 20.0), Err(AntiAbuseConfigError::InvalidFrameRate { .. })));
        assert!(matches!(AntiAbuseConfig::new(8, 10, f64::INFINITY, 20.0), Err(AntiAbuseConfigError::InvalidFrameRate { .. })));
        assert!(matches!(AntiAbuseConfig::new(8, 10, -1.0, 20.0), Err(AntiAbuseConfigError::InvalidFrameRate { .. })));
    }

    #[test]
    fn config_rejects_non_finite_burst() {
        assert!(matches!(AntiAbuseConfig::new(8, 10, 10.0, 0.0), Err(AntiAbuseConfigError::InvalidFrameBurst { .. })));
    }

    #[test]
    fn conn_cap_per_ip_blocks_after_threshold() {
        let g = Arc::new(AntiAbuseGuard::new(AntiAbuseConfig::new(2, 100, 10.0, 5.0).expect("valid")));
        let now = t0();
        let _a = g.try_accept_connection(ip(10, 0, 0, 1), now).expect("accept 1");
        let _b = g.try_accept_connection(ip(10, 0, 0, 1), now).expect("accept 2");
        let err = g.try_accept_connection(ip(10, 0, 0, 1), now).expect_err("should be rejected");
        assert!(matches!(err, AntiAbuseRejection::ConnCapPerIp { active: 2, cap: 2 }));
        assert_eq!(err.metric_label(), "conn_cap_per_ip");
    }

    #[test]
    fn dropping_ticket_frees_slot() {
        let g = Arc::new(AntiAbuseGuard::new(AntiAbuseConfig::new(1, 100, 10.0, 5.0).expect("valid")));
        let now = t0();
        let ticket = g.try_accept_connection(ip(10, 0, 0, 2), now).expect("accept 1");
        assert!(g.try_accept_connection(ip(10, 0, 0, 2), now).is_err());
        drop(ticket);
        assert_eq!(g.conn_count(ip(10, 0, 0, 2)), 0);
        assert_eq!(g.tracked_ip_count(), 0, "entry should be reclaimed");
        let _t2 = g.try_accept_connection(ip(10, 0, 0, 2), now).expect("re-accept after drop");
    }

    #[test]
    fn distinct_ips_have_independent_caps() {
        let g = Arc::new(AntiAbuseGuard::new(AntiAbuseConfig::new(1, 100, 10.0, 5.0).expect("valid")));
        let now = t0();
        let _a = g.try_accept_connection(ip(10, 0, 0, 1), now).expect("ok");
        let _b = g.try_accept_connection(ip(10, 0, 0, 2), now).expect("ok");
        let _c = g.try_accept_connection(ip(10, 0, 0, 3), now).expect("ok");
        assert_eq!(g.tracked_ip_count(), 3);
    }

    #[test]
    fn tracked_ip_cap_rejects_new_ips_but_still_admits_known_ips() {
        let g = Arc::new(AntiAbuseGuard::new(AntiAbuseConfig::new(8, 2, 10.0, 5.0).expect("valid")));
        let now = t0();
        let _a = g.try_accept_connection(ip(10, 0, 0, 1), now).expect("ok");
        let _b = g.try_accept_connection(ip(10, 0, 0, 2), now).expect("ok");
        // Third *new* IP exceeds the ip-cap and must be rejected.
        let err = g.try_accept_connection(ip(10, 0, 0, 3), now).expect_err("new ip rejected at cap");
        assert_eq!(err, AntiAbuseRejection::IpCapReached);
        // An already-tracked IP is still admitted (this is by design — we
        // only reject *new* IPs when the tracker is full).
        let _c = g.try_accept_connection(ip(10, 0, 0, 1), now).expect("known ip ok");
    }

    #[test]
    fn token_bucket_allows_burst_then_throttles() {
        let g = Arc::new(AntiAbuseGuard::new(AntiAbuseConfig::new(8, 100, 1.0, 3.0).expect("valid")));
        let now = t0();
        let _t = g.try_accept_connection(ip(10, 0, 0, 1), now).expect("accept");
        // Burst of 3 should pass.
        assert!(g.try_consume_frame(ip(10, 0, 0, 1), now));
        assert!(g.try_consume_frame(ip(10, 0, 0, 1), now));
        assert!(g.try_consume_frame(ip(10, 0, 0, 1), now));
        // 4th immediate frame must be rejected (bucket empty).
        assert!(!g.try_consume_frame(ip(10, 0, 0, 1), now));
        // After ~1.5s at rate=1/s the bucket has 1.5 tokens — one more
        // frame should pass; the next should not.
        let later = now + Duration::from_millis(1_500);
        assert!(g.try_consume_frame(ip(10, 0, 0, 1), later));
        assert!(!g.try_consume_frame(ip(10, 0, 0, 1), later));
    }

    #[test]
    fn token_bucket_refill_caps_at_burst() {
        let g = Arc::new(AntiAbuseGuard::new(AntiAbuseConfig::new(8, 100, 10.0, 5.0).expect("valid")));
        let now = t0();
        let _t = g.try_accept_connection(ip(10, 0, 0, 1), now).expect("accept");
        // Drain the bucket
        for _ in 0..5 {
            assert!(g.try_consume_frame(ip(10, 0, 0, 1), now));
        }
        // Wait long enough to refill far past the burst cap; the cap
        // must still hold.
        let later = now + Duration::from_secs(60);
        for _ in 0..5 {
            assert!(g.try_consume_frame(ip(10, 0, 0, 1), later));
        }
        // 6th frame must fail because burst caps at 5 even after a long wait.
        assert!(!g.try_consume_frame(ip(10, 0, 0, 1), later));
    }

    #[test]
    fn try_consume_frame_for_untracked_ip_returns_false() {
        let g = Arc::new(AntiAbuseGuard::new(AntiAbuseConfig::new(8, 100, 10.0, 5.0).expect("valid")));
        let now = t0();
        assert!(!g.try_consume_frame(ip(10, 0, 0, 9), now));
    }

    #[test]
    fn unlimited_config_admits_huge_traffic() {
        let g = Arc::new(AntiAbuseGuard::new(AntiAbuseConfig::unlimited()));
        let now = t0();
        for n in 0..200u8 {
            let _t = g.try_accept_connection(ip(10, 0, 0, n.max(1)), now).expect("ok");
        }
    }

    #[test]
    fn refill_eta_is_finite_and_positive() {
        let eta = refill_eta(AntiAbuseConfig::production());
        assert!(eta.as_secs_f64() > 0.0);
        assert!(eta.as_secs_f64().is_finite());
    }

    fn empty_lookup(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn from_lookup_empty_yields_production_defaults() {
        let cfg = AntiAbuseConfig::from_lookup(empty_lookup).expect("defaults are valid");
        assert_eq!(cfg, AntiAbuseConfig::production());
    }

    #[test]
    fn from_lookup_applies_every_known_key() {
        let map = std::collections::HashMap::from([
            ("KATPOOL_ANTI_ABUSE_MAX_CONN_PER_IP", "8"),
            ("KATPOOL_ANTI_ABUSE_MAX_TRACKED_IPS", "32"),
            ("KATPOOL_ANTI_ABUSE_FRAME_RATE_PER_SEC", "12.5"),
            ("KATPOOL_ANTI_ABUSE_FRAME_BURST", "30"),
        ]);
        let cfg = AntiAbuseConfig::from_lookup(|k| map.get(k).map(|s| (*s).to_owned())).expect("valid");
        assert_eq!(cfg.max_conn_per_ip, 8);
        assert_eq!(cfg.max_tracked_ips, 32);
        assert!((cfg.frame_rate_per_sec - 12.5).abs() < f64::EPSILON);
        assert!((cfg.frame_burst - 30.0).abs() < f64::EPSILON);
    }

    #[test]
    fn from_lookup_rejects_unparsable_value() {
        let result = AntiAbuseConfig::from_lookup(|k| {
            if k == "KATPOOL_ANTI_ABUSE_MAX_CONN_PER_IP" { Some("not-a-number".to_owned()) } else { None }
        });
        match result {
            Err(AntiAbuseConfigError::InvalidEnvValue { key, value }) => {
                assert_eq!(key, "KATPOOL_ANTI_ABUSE_MAX_CONN_PER_IP");
                assert_eq!(value, "not-a-number");
            }
            other => panic!("expected InvalidEnvValue, got {other:?}"),
        }
    }

    #[test]
    fn from_lookup_runs_validation_on_parsed_values() {
        let result =
            AntiAbuseConfig::from_lookup(|k| if k == "KATPOOL_ANTI_ABUSE_MAX_CONN_PER_IP" { Some("0".to_owned()) } else { None });
        assert_eq!(result, Err(AntiAbuseConfigError::ZeroConnCap));
    }

    #[test]
    fn from_lookup_rejects_non_finite_rate() {
        let result =
            AntiAbuseConfig::from_lookup(|k| if k == "KATPOOL_ANTI_ABUSE_FRAME_RATE_PER_SEC" { Some("inf".to_owned()) } else { None });
        assert!(matches!(result, Err(AntiAbuseConfigError::InvalidFrameRate { .. })));
    }
}
