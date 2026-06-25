//! Floor-price quote source for the KAS→NACHO payout conversion.
//!
//! Per [ADR-0016] the NACHO floor price is the market ratio **KAS per NACHO**,
//! derived from `CoinGecko`'s USD spot prices for both assets
//! (`KAS_per_NACHO = nacho_usd / kaspa_usd`) over plain HTTPS, behind a
//! [`FloorPriceSource`] trait so the engine and tests can substitute a fake, and
//! wrapped in a [`CircuitBreaker`] that fails the cycle **closed** (skip, never
//! guess) when the upstream is degraded.
//!
//! The division is exact: the two USD quotes are read as their verbatim JSON
//! text (so an 18-significant-figure float is never truncated through `f64`,
//! ADR-0013) and divided with [`BigDecimal`], then floored to a fixed scale into
//! the integer [`FloorPrice`] the conversion math (`rebate`) consumes.
//!
//! [ADR-0016]: ../../../docs/decisions/0016-krc20-payout-conversion-and-floor-price.md

use std::collections::HashMap;
use std::str::FromStr;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use bigdecimal::{BigDecimal, RoundingMode};
use num_traits::{Signed, ToPrimitive, Zero};
use reqwest::{Client, StatusCode};
use serde_json::value::RawValue;

use crate::rebate::{FloorPrice, MAX_FLOOR_PRICE_SCALE, RebateError};

/// Default quote API base URL (no trailing slash). `CoinGecko`'s public
/// `simple/price` endpoint answers a keyless HTTPS GET (ADR-0016).
pub const DEFAULT_QUOTE_BASE: &str = "https://api.coingecko.com";

/// `CoinGecko` coin id for the NACHO token (the numerator of the ratio).
pub const DEFAULT_NACHO_COIN_ID: &str = "nacho-the-kat";

/// `CoinGecko` coin id for Kaspa (the KAS leg of the ratio).
pub const DEFAULT_KASPA_COIN_ID: &str = "kaspa";

/// Quote currency both legs are denominated in before the ratio cancels it.
pub const QUOTE_VS_CURRENCY: &str = "usd";

/// Default token ticker inscribed on the KRC-20 transfer.
pub const DEFAULT_QUOTE_TICKER: &str = "NACHO";

/// Default per-request HTTP timeout.
pub const DEFAULT_HTTP_TIMEOUT: Duration = Duration::from_secs(5);

/// Errors from fetching or interpreting a floor-price quote.
#[derive(Debug, thiserror::Error)]
pub enum QuoteError {
    /// Transport / connection / timeout failure talking to the API.
    #[error("quote upstream: {0}")]
    Upstream(String),

    /// The API returned a non-200 status.
    #[error("quote endpoint returned {0}")]
    Status(StatusCode),

    /// The response body was missing, empty, or not the expected shape.
    #[error("quote malformed: {0}")]
    Malformed(String),

    /// The quoted price could not be parsed into a [`FloorPrice`].
    #[error(transparent)]
    Price(#[from] RebateError),

    /// The circuit breaker is open; the request was not attempted.
    #[error("quote circuit open")]
    CircuitOpen,
}

/// A source of NACHO floor-price quotes.
#[async_trait]
pub trait FloorPriceSource: Send + Sync {
    /// Fetches the current floor price for `ticker`.
    async fn floor_price(&self, ticker: &str) -> Result<FloorPrice, QuoteError>;
}

/// `CoinGecko` `simple/price` shape: `{ "<coin-id>": { "<vs>": <number> } }`.
/// The USD quote is captured as a [`RawValue`] — its verbatim JSON text — so an
/// 18-significant-figure float is parsed exactly rather than through `f64`
/// (ADR-0013). Example body:
/// `{"kaspa":{"usd":0.031451},"nacho-the-kat":{"usd":1.03e-05}}`.
type SimplePriceResponse = HashMap<String, HashMap<String, Box<RawValue>>>;

/// Reads the verbatim USD quote text for `coin_id` out of a parsed body.
fn usd_quote<'a>(parsed: &'a SimplePriceResponse, coin_id: &str) -> Result<&'a str, QuoteError> {
    parsed
        .get(coin_id)
        .and_then(|legs| legs.get(QUOTE_VS_CURRENCY))
        .map(|raw| raw.get())
        .ok_or_else(|| {
            QuoteError::Malformed(format!("missing {coin_id}/{QUOTE_VS_CURRENCY} in quote"))
        })
}

/// Derives `KAS per NACHO = nacho_usd / kaspa_usd` as an exact [`FloorPrice`].
///
/// Both inputs are verbatim decimal/scientific JSON number text. The quotient is
/// computed with [`BigDecimal`] (no `f64`) and **floored** — never rounded up,
/// so a payout can never be over-funded — to [`MAX_FLOOR_PRICE_SCALE`]
/// fractional digits, matching the conversion's fixed-point contract.
///
/// # Errors
///
/// [`QuoteError::Malformed`] if either quote is not a finite number, is
/// negative, the KAS quote is zero (division undefined), or the floored ratio
/// underflows to zero / overflows `u128`.
pub fn derive_floor_price(nacho_usd: &str, kaspa_usd: &str) -> Result<FloorPrice, QuoteError> {
    let parse = |label: &str, s: &str| -> Result<BigDecimal, QuoteError> {
        BigDecimal::from_str(s.trim())
            .map_err(|e| QuoteError::Malformed(format!("{label} quote `{s}`: {e}")))
    };
    let nacho = parse("nacho", nacho_usd)?;
    let kaspa = parse("kaspa", kaspa_usd)?;
    if nacho.is_negative() || kaspa.is_negative() {
        return Err(QuoteError::Malformed("negative quote".to_owned()));
    }
    if nacho.is_zero() {
        return Err(QuoteError::Malformed("nacho quote is zero".to_owned()));
    }
    if kaspa.is_zero() {
        return Err(QuoteError::Malformed(
            "kaspa quote is zero (division undefined)".to_owned(),
        ));
    }
    // Floor to the fixed scale the integer conversion expects. RoundingMode::Down
    // truncates toward zero; both legs are positive, so this is a true floor and
    // never over-funds a payout (ADR-0016).
    let ratio =
        (nacho / kaspa).with_scale_round(i64::from(MAX_FLOOR_PRICE_SCALE), RoundingMode::Down);
    let (mantissa_bi, exponent) = ratio.as_bigint_and_exponent();
    // `with_scale_round` fixes the scale, so the exponent equals our scale.
    let scale = u32::try_from(exponent)
        .map_err(|_| QuoteError::Malformed(format!("unexpected quote scale {exponent}")))?;
    let mantissa = mantissa_bi
        .to_u128()
        .ok_or_else(|| QuoteError::Malformed("derived price overflows u128".to_owned()))?;
    Ok(FloorPrice::from_mantissa_scale(mantissa, scale)?)
}

/// Parses a `CoinGecko` `simple/price` body and derives the NACHO floor price.
///
/// # Errors
///
/// [`QuoteError::Malformed`] for unparsable JSON, a missing coin/quote leg, or
/// a non-derivable ratio (see [`derive_floor_price`]).
pub fn parse_simple_price_response(
    body: &[u8],
    nacho_id: &str,
    kaspa_id: &str,
) -> Result<FloorPrice, QuoteError> {
    let parsed: SimplePriceResponse =
        serde_json::from_slice(body).map_err(|e| QuoteError::Malformed(format!("json: {e}")))?;
    let nacho_usd = usd_quote(&parsed, nacho_id)?;
    let kaspa_usd = usd_quote(&parsed, kaspa_id)?;
    derive_floor_price(nacho_usd, kaspa_usd)
}

/// HTTP-backed `CoinGecko` floor-price source. Quotes NACHO and KAS in USD in a
/// single `simple/price` request and derives KAS-per-NACHO (ADR-0016).
#[derive(Debug, Clone)]
pub struct CoinGeckoFloorPrice {
    base: String,
    nacho_id: String,
    kaspa_id: String,
    http: Client,
}

impl CoinGeckoFloorPrice {
    /// Builds a client against `base` (no trailing slash) with the given
    /// request timeout, using the default NACHO/KAS coin ids.
    ///
    /// # Errors
    ///
    /// Returns [`QuoteError::Upstream`] if the `reqwest` client cannot be
    /// built.
    pub fn new(base: impl Into<String>, http_timeout: Duration) -> Result<Self, QuoteError> {
        let http = Client::builder()
            .timeout(http_timeout)
            .connect_timeout(Duration::from_secs(2))
            .user_agent(concat!("katpool-payout-krc20/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| QuoteError::Upstream(format!("reqwest build: {e}")))?;
        Ok(Self {
            base: base.into(),
            nacho_id: DEFAULT_NACHO_COIN_ID.to_owned(),
            kaspa_id: DEFAULT_KASPA_COIN_ID.to_owned(),
            http,
        })
    }
}

impl Default for CoinGeckoFloorPrice {
    /// Production default: `api.coingecko.com` with the default timeout.
    ///
    /// # Panics
    ///
    /// Only if `reqwest` cannot build a client with static, valid settings,
    /// which does not happen in practice.
    fn default() -> Self {
        #[allow(clippy::expect_used)]
        Self::new(DEFAULT_QUOTE_BASE, DEFAULT_HTTP_TIMEOUT).expect("static reqwest client builds")
    }
}

#[async_trait]
impl FloorPriceSource for CoinGeckoFloorPrice {
    /// `ticker` is advisory: `CoinGecko` is queried by coin id, so the configured
    /// NACHO/KAS ids drive the request (the inscription ticker is applied
    /// elsewhere on the transfer).
    async fn floor_price(&self, _ticker: &str) -> Result<FloorPrice, QuoteError> {
        let url = format!(
            "{}/api/v3/simple/price?ids={},{}&vs_currencies={}&precision=18",
            self.base, self.nacho_id, self.kaspa_id, QUOTE_VS_CURRENCY
        );
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| QuoteError::Upstream(format!("{url}: {e}")))?;
        if resp.status() != StatusCode::OK {
            return Err(QuoteError::Status(resp.status()));
        }
        let body = resp
            .bytes()
            .await
            .map_err(|e| QuoteError::Upstream(format!("body: {e}")))?;
        parse_simple_price_response(&body, &self.nacho_id, &self.kaspa_id)
    }
}

// ---------- circuit breaker ------------------------------------------

/// Circuit-breaker state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Requests pass through; failures are counted.
    Closed,
    /// Requests short-circuit until the cooldown elapses.
    Open,
    /// A single trial request is allowed to probe recovery.
    HalfOpen,
}

/// A pure, time-injected circuit-breaker state machine.
///
/// `Closed → Open` after `failure_threshold` consecutive failures;
/// `Open → HalfOpen` once `cooldown` has elapsed; `HalfOpen → Closed` on a
/// success, or back to `Open` on a failure. Time is supplied by the caller
/// ([`Instant`]) so transitions are deterministic in tests.
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    state: CircuitState,
    consecutive_failures: u32,
    failure_threshold: u32,
    cooldown: Duration,
    opened_at: Option<Instant>,
}

impl CircuitBreaker {
    /// Builds a closed breaker that opens after `failure_threshold`
    /// consecutive failures and probes again after `cooldown`.
    #[must_use]
    pub const fn new(failure_threshold: u32, cooldown: Duration) -> Self {
        Self {
            state: CircuitState::Closed,
            consecutive_failures: 0,
            failure_threshold,
            cooldown,
            opened_at: None,
        }
    }

    /// Current state, after applying any time-based `Open → HalfOpen`
    /// transition relative to `now`.
    #[must_use]
    pub fn state(&self, now: Instant) -> CircuitState {
        match (self.state, self.opened_at) {
            (CircuitState::Open, Some(opened)) if now.duration_since(opened) >= self.cooldown => {
                CircuitState::HalfOpen
            }
            (s, _) => s,
        }
    }

    /// Whether a request should be attempted now (i.e. the circuit is not
    /// open). Call before issuing a request.
    #[must_use]
    pub fn allows_request(&self, now: Instant) -> bool {
        self.state(now) != CircuitState::Open
    }

    /// Records a successful request: resets failures and closes the circuit.
    pub const fn on_success(&mut self) {
        self.state = CircuitState::Closed;
        self.consecutive_failures = 0;
        self.opened_at = None;
    }

    /// Records a failed request at `now`. Trips the circuit open once the
    /// consecutive-failure threshold is reached (or immediately re-opens
    /// from half-open).
    pub fn on_failure(&mut self, now: Instant) {
        if self.state(now) == CircuitState::HalfOpen {
            self.state = CircuitState::Open;
            self.opened_at = Some(now);
            return;
        }
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);
        if self.consecutive_failures >= self.failure_threshold {
            self.state = CircuitState::Open;
            self.opened_at = Some(now);
        }
    }
}

/// A [`FloorPriceSource`] wrapped in a [`CircuitBreaker`].
///
/// While the circuit is open, [`floor_price`](FloorPriceSource::floor_price)
/// returns [`QuoteError::CircuitOpen`] without touching the upstream. The
/// breaker is behind a `Mutex` so the guarded source stays `Send + Sync`.
pub struct BreakeredSource<S: FloorPriceSource> {
    inner: S,
    breaker: tokio::sync::Mutex<CircuitBreaker>,
}

impl<S: FloorPriceSource> BreakeredSource<S> {
    /// Wraps `inner` with a fresh breaker.
    #[must_use]
    pub fn new(inner: S, breaker: CircuitBreaker) -> Self {
        Self {
            inner,
            breaker: tokio::sync::Mutex::new(breaker),
        }
    }
}

#[async_trait]
impl<S: FloorPriceSource> FloorPriceSource for BreakeredSource<S> {
    async fn floor_price(&self, ticker: &str) -> Result<FloorPrice, QuoteError> {
        let now = Instant::now();
        {
            let breaker = self.breaker.lock().await;
            if !breaker.allows_request(now) {
                return Err(QuoteError::CircuitOpen);
            }
        }
        let result = self.inner.floor_price(ticker).await;
        let mut breaker = self.breaker.lock().await;
        match &result {
            Ok(_) => breaker.on_success(),
            Err(_) => breaker.on_failure(Instant::now()),
        }
        result
    }
}
