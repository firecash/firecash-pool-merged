//! KAS-sompi → NACHO base-unit conversion at payout time.
//!
//! Per [ADR-0016], a wallet's pending NACHO rebate is stored in KAS-sompi
//! (`nacho_rebate_accrual.accrued_sompi − paid_sompi`) with the tier
//! multiplier already applied at accrual time. At payout we convert that
//! pending balance to NACHO base units at the prevailing floor price — with
//! **no** further multiplier.
//!
//! Because both KAS-sompi and NACHO base units carry 8 decimal places, the
//! scale factors cancel:
//!
//! ```text
//! nacho_base_units = floor(pending_sompi / floor_price_kas_per_nacho)
//! ```
//!
//! The floor price is held as an exact fixed-point rational
//! `mantissa / 10^scale` (no `f64` in value math, per ADR-0013), so the
//! conversion is integer-exact:
//!
//! ```text
//! nacho_base_units = floor(pending_sompi × 10^scale / mantissa)
//! ```
//!
//! [ADR-0016]: ../../../docs/decisions/0016-krc20-payout-conversion-and-floor-price.md

/// Default minimum pending rebate for a NACHO cycle (10 KAS-worth).
///
/// In KAS-sompi; the coarse pre-filter for wallet selection ahead of the
/// per-amount dust gate. Operator-tunable. `1 KAS = 100_000_000 sompi`.
pub const DEFAULT_MIN_PENDING_SOMPI: i64 = 1_000_000_000;

/// Default minimum NACHO base-units actually worth a reveal transaction
/// (dust gate). `1 NACHO = 10^8 base units`. Amounts below this stay
/// accrued for a later cycle rather than burning a reveal fee.
pub const DEFAULT_MIN_NACHO_BASE_UNITS: u128 = 100_000_000;

/// Largest decimal scale (fractional digits) accepted for a floor price.
/// Bounds `10^scale` so the conversion can never overflow `u128`.
pub const MAX_FLOOR_PRICE_SCALE: u32 = 18;

/// Errors from floor-price parsing and rebate conversion.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RebateError {
    /// The floor-price string was not a finite positive decimal.
    #[error("floor price `{input}`: {reason}")]
    PriceParse {
        /// The offending input.
        input: String,
        /// Why it was rejected.
        reason: &'static str,
    },

    /// The pending balance was negative (the `paid ≤ accrued` CHECK should
    /// make this unreachable, but the conversion refuses it defensively).
    #[error("pending sompi must be non-negative, got {0}")]
    NegativePending(i64),

    /// The integer conversion overflowed `u128`.
    #[error("conversion overflow")]
    Overflow,
}

/// An exact fixed-point floor price: `value = mantissa / 10^scale`, in KAS
/// per one NACHO token. `mantissa` is guaranteed non-zero and `scale` is at
/// most [`MAX_FLOOR_PRICE_SCALE`] by construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FloorPrice {
    mantissa: u128,
    scale: u32,
}

impl FloorPrice {
    /// Builds a price from an explicit mantissa and scale.
    ///
    /// # Errors
    ///
    /// Rejects a zero mantissa or a scale above [`MAX_FLOOR_PRICE_SCALE`].
    pub fn from_mantissa_scale(mantissa: u128, scale: u32) -> Result<Self, RebateError> {
        if mantissa == 0 {
            return Err(RebateError::PriceParse {
                input: format!("{mantissa}e-{scale}"),
                reason: "price is zero",
            });
        }
        if scale > MAX_FLOOR_PRICE_SCALE {
            return Err(RebateError::PriceParse {
                input: format!("{mantissa}e-{scale}"),
                reason: "scale exceeds maximum",
            });
        }
        Ok(Self { mantissa, scale })
    }

    /// Parses a plain decimal string (e.g. `"0.000365"`, `"1.5"`, `"12"`).
    ///
    /// Rejects empty input, signs, exponent notation, multiple decimal
    /// points, non-digit characters, an all-zero value, and scales beyond
    /// [`MAX_FLOOR_PRICE_SCALE`].
    ///
    /// # Errors
    ///
    /// Returns [`RebateError::PriceParse`] for any of the above.
    pub fn from_decimal_str(input: &str) -> Result<Self, RebateError> {
        let err = |reason: &'static str| RebateError::PriceParse {
            input: input.to_owned(),
            reason,
        };

        if input.is_empty() {
            return Err(err("empty"));
        }
        let mut parts = input.split('.');
        let int_part = parts.next().unwrap_or("");
        let frac_part = parts.next().unwrap_or("");
        if parts.next().is_some() {
            return Err(err("multiple decimal points"));
        }
        if int_part.is_empty() && frac_part.is_empty() {
            return Err(err("no digits"));
        }
        if !int_part
            .bytes()
            .chain(frac_part.bytes())
            .all(|b| b.is_ascii_digit())
        {
            return Err(err("non-digit character"));
        }

        let scale = u32::try_from(frac_part.len()).map_err(|_| err("scale exceeds maximum"))?;
        if scale > MAX_FLOOR_PRICE_SCALE {
            return Err(err("scale exceeds maximum"));
        }

        let mut digits = String::with_capacity(int_part.len() + frac_part.len());
        digits.push_str(int_part);
        digits.push_str(frac_part);
        let mantissa: u128 = digits.parse().map_err(|_| err("value out of range"))?;
        if mantissa == 0 {
            return Err(err("price is zero"));
        }
        Ok(Self { mantissa, scale })
    }

    /// The raw mantissa (numerator over `10^scale`).
    #[must_use]
    pub const fn mantissa(&self) -> u128 {
        self.mantissa
    }

    /// The decimal scale (denominator exponent).
    #[must_use]
    pub const fn scale(&self) -> u32 {
        self.scale
    }
}

/// Converts a pending KAS-sompi balance to NACHO base units, rounding down.
///
/// `nacho_base_units = floor(pending_sompi × 10^scale / mantissa)`.
///
/// # Errors
///
/// - [`RebateError::NegativePending`] if `pending_sompi < 0`.
/// - [`RebateError::Overflow`] if the intermediate product exceeds `u128`
///   (not reachable for `pending_sompi ≤ i64::MAX` and a bounded scale).
pub fn nacho_base_units(pending_sompi: i64, price: &FloorPrice) -> Result<u128, RebateError> {
    if pending_sompi < 0 {
        return Err(RebateError::NegativePending(pending_sompi));
    }
    let pending = u128::from(pending_sompi.unsigned_abs());
    let factor = 10u128
        .checked_pow(price.scale)
        .ok_or(RebateError::Overflow)?;
    let scaled = pending.checked_mul(factor).ok_or(RebateError::Overflow)?;
    // Floor division is the defined rounding for the conversion (ADR-0016):
    // never over-pay a fractional base unit. mantissa is non-zero by invariant.
    #[allow(clippy::integer_division)]
    Ok(scaled / price.mantissa)
}

/// Whether a converted NACHO amount clears the dust gate (is worth paying).
#[must_use]
pub const fn is_payable(nacho_base_units: u128, min_nacho_base_units: u128) -> bool {
    nacho_base_units >= min_nacho_base_units
}
