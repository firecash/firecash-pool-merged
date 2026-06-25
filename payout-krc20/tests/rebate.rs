//! Deterministic tests for the KAS-sompi → NACHO conversion and the
//! exact fixed-point floor-price parser (ADR-0016).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::integer_division
)]

use payout_krc20::{
    DEFAULT_MIN_NACHO_BASE_UNITS, FloorPrice, RebateError, is_payable, nacho_base_units,
};
use proptest::prelude::*;

#[test]
fn parses_canonical_floor_price() {
    let p = FloorPrice::from_decimal_str("0.000365").expect("parse");
    assert_eq!(p.mantissa(), 365);
    assert_eq!(p.scale(), 6);
}

#[test]
fn parses_integer_and_single_fraction() {
    let whole = FloorPrice::from_decimal_str("12").expect("parse");
    assert_eq!((whole.mantissa(), whole.scale()), (12, 0));
    let half = FloorPrice::from_decimal_str("1.5").expect("parse");
    assert_eq!((half.mantissa(), half.scale()), (15, 1));
}

#[test]
fn rejects_malformed_prices() {
    for bad in [
        "", ".", "-0.1", "1e-3", "0", "0.0", "1.2.3", "0.00o5", " 0.1", "0x10",
    ] {
        assert!(
            matches!(
                FloorPrice::from_decimal_str(bad),
                Err(RebateError::PriceParse { .. })
            ),
            "expected `{bad}` to be rejected"
        );
    }
}

#[test]
fn rejects_scale_beyond_max() {
    let nineteen_frac = format!("0.{}", "1".repeat(19));
    assert!(matches!(
        FloorPrice::from_decimal_str(&nineteen_frac),
        Err(RebateError::PriceParse { .. })
    ));
}

#[test]
fn converts_known_vector_exactly() {
    // 1 KAS pending (1e8 sompi) at 0.000365 KAS/NACHO:
    //   floor(1e8 * 1e6 / 365) = floor(1e14 / 365) = 273_972_602_739
    let price = FloorPrice::from_decimal_str("0.000365").expect("parse");
    let nacho = nacho_base_units(100_000_000, &price).expect("convert");
    assert_eq!(nacho, 273_972_602_739);
}

#[test]
fn integer_price_is_plain_division() {
    // At price 2 KAS/NACHO, 10 sompi → floor(10 / 2) = 5 base units.
    let price = FloorPrice::from_mantissa_scale(2, 0).expect("price");
    assert_eq!(nacho_base_units(10, &price).expect("convert"), 5);
}

#[test]
fn zero_pending_converts_to_zero() {
    let price = FloorPrice::from_decimal_str("0.000365").expect("parse");
    assert_eq!(nacho_base_units(0, &price).expect("convert"), 0);
}

#[test]
fn negative_pending_is_rejected() {
    let price = FloorPrice::from_decimal_str("0.000365").expect("parse");
    assert_eq!(
        nacho_base_units(-1, &price),
        Err(RebateError::NegativePending(-1))
    );
}

#[test]
fn max_pending_does_not_overflow() {
    let price = FloorPrice::from_mantissa_scale(1, 18).expect("price");
    // i64::MAX * 10^18 fits in u128 (≈ 9.2e36 < 3.4e38).
    let got = nacho_base_units(i64::MAX, &price).expect("no overflow");
    assert_eq!(
        got,
        u128::from(i64::MAX.unsigned_abs()) * 1_000_000_000_000_000_000
    );
}

#[test]
fn dust_gate_uses_inclusive_threshold() {
    assert!(!is_payable(
        DEFAULT_MIN_NACHO_BASE_UNITS - 1,
        DEFAULT_MIN_NACHO_BASE_UNITS
    ));
    assert!(is_payable(
        DEFAULT_MIN_NACHO_BASE_UNITS,
        DEFAULT_MIN_NACHO_BASE_UNITS
    ));
}

proptest! {
    /// The conversion is an exact floor: result*mantissa ≤ pending*10^scale
    /// < (result+1)*mantissa, and never overflows for the bounded inputs.
    #[test]
    fn conversion_is_exact_floor(
        pending in 0i64..=i64::MAX,
        mantissa in 1u128..=1_000_000_000_000u128,
        scale in 0u32..=12u32,
    ) {
        let price = FloorPrice::from_mantissa_scale(mantissa, scale).unwrap();
        let got = nacho_base_units(pending, &price).unwrap();
        let numerator = u128::from(pending.unsigned_abs()) * 10u128.pow(scale);
        prop_assert_eq!(got, numerator / mantissa);
        prop_assert!(got * mantissa <= numerator);
        prop_assert!(numerator < (got + 1) * mantissa);
    }

    /// Round-trip: a parsed price reproduces its mantissa/scale.
    #[test]
    fn parse_then_compare(mantissa in 1u128..=999_999_999u128, scale in 0u32..=9u32) {
        let s = if scale == 0 {
            mantissa.to_string()
        } else {
            // Build "<int>.<frac>" with `scale` fractional digits.
            let padded = format!("{mantissa:0>width$}", width = (scale as usize) + 1);
            let split = padded.len() - scale as usize;
            format!("{}.{}", &padded[..split], &padded[split..])
        };
        let price = FloorPrice::from_decimal_str(&s).unwrap();
        // Reconstruct the rational value and compare cross-multiplied.
        prop_assert_eq!(
            price.mantissa() * 10u128.pow(scale),
            mantissa * 10u128.pow(price.scale())
        );
    }
}
