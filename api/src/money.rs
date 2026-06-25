//! Integer-amount → wire encoding.
//!
//! All on-chain amounts (KAS sompi, NACHO base units) are emitted as
//! **decimal strings**, never JSON numbers: a JavaScript dashboard — the
//! primary consumer — loses precision on integers above 2^53, and KAS sompi
//! totals brush that ceiling while NACHO base units exceed it (ADR-0021 §F).
//!
//! Each amount is rendered twice: the raw integer (`*_sompi` / `*_base_units`)
//! and a human-readable fixed-point decimal (`*_kas` / `*_nacho`), both as
//! strings, so a client can display either without re-deriving precision.

use serde::Serialize;

/// KAS / NACHO decimal precision: 1 KAS = 10^8 sompi (also the base-unit
/// scale of NACHO, a KRC-20 token with 8 decimals).
const KAS_DECIMALS: u32 = 8;

/// A signed on-chain amount rendered for the wire: the raw integer plus a
/// fixed-point decimal, both decimal strings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KasAmount {
    /// Raw amount in sompi (1 KAS = 100,000,000 sompi), as a decimal string.
    pub sompi: String,
    /// Human-readable KAS, fixed-point with trailing zeros trimmed.
    pub kas: String,
}

impl KasAmount {
    /// Build a KAS amount from a sompi integer.
    #[must_use]
    pub fn from_sompi(sompi: i64) -> Self {
        Self {
            sompi: sompi.to_string(),
            kas: scaled_decimal_string(sompi, KAS_DECIMALS),
        }
    }
}

/// Render a scaled integer as a fixed-point decimal string with trailing
/// zeros trimmed (e.g. `173951600000` at 8 decimals → `"1739.516"`).
///
/// Pure and allocation-bounded; no floating point, so the rendering is exact.
#[allow(clippy::integer_division)] // exact base-10 split of a fixed-point value
#[must_use]
pub fn scaled_decimal_string(units: i64, decimals: u32) -> String {
    let divisor = 10_u64.pow(decimals);
    let negative = units < 0;
    let magnitude = units.unsigned_abs();
    let whole = magnitude / divisor;
    let frac = magnitude % divisor;
    let sign = if negative { "-" } else { "" };
    if frac == 0 {
        return format!("{sign}{whole}");
    }
    let frac_full = format!("{frac:0width$}", width = decimals as usize);
    let frac_trimmed = frac_full.trim_end_matches('0');
    format!("{sign}{whole}.{frac_trimmed}")
}

/// KAS value of a sompi amount, as a fixed-point decimal string.
#[must_use]
pub fn kas_string(sompi: i64) -> String {
    scaled_decimal_string(sompi, KAS_DECIMALS)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    /// Sompi per whole KAS.
    const SOMPI_PER_KAS: i64 = 100_000_000;

    #[test]
    fn whole_kas_has_no_fraction() {
        assert_eq!(kas_string(SOMPI_PER_KAS), "1");
        assert_eq!(kas_string(0), "0");
    }

    #[test]
    fn fractional_kas_trims_trailing_zeros() {
        assert_eq!(kas_string(173_951_600_000), "1739.516");
        assert_eq!(kas_string(150_000_000), "1.5");
    }

    #[test]
    fn one_sompi_is_smallest_unit() {
        assert_eq!(kas_string(1), "0.00000001");
    }

    #[test]
    fn negative_amounts_render_with_sign() {
        assert_eq!(kas_string(-150_000_000), "-1.5");
    }

    #[test]
    fn from_sompi_emits_both_fields() {
        let a = KasAmount::from_sompi(173_951_600_000);
        assert_eq!(a.sompi, "173951600000");
        assert_eq!(a.kas, "1739.516");
    }
}
