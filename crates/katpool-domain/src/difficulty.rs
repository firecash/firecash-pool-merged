//! [`ShareDifficulty`] — a validated share-difficulty value.
//!
//! Stratum protocol's difficulty is conventionally an `f64` because miner
//! libraries expect the wire-format double. The bridge stores its
//! per-worker `min_diff` and `shares_diff` as `f64` for the same reason.
//! We wrap it as a newtype that only admits finite, strictly-positive
//! values — eliminating the "NaN slips into a Prometheus histogram" class
//! of bugs.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A finite, positive difficulty value.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ShareDifficulty(f64);

impl ShareDifficulty {
    /// Construct a difficulty value from an `f64`. Rejects NaN, ±infinity,
    /// zero, and negative values.
    pub fn new(d: f64) -> Result<Self, DifficultyError> {
        if !d.is_finite() {
            return Err(DifficultyError::NotFinite);
        }
        if d <= 0.0 {
            return Err(DifficultyError::NonPositive);
        }
        Ok(Self(d))
    }

    /// Underlying `f64` value.
    #[must_use]
    pub const fn value(self) -> f64 {
        self.0
    }
}

/// Errors from [`ShareDifficulty::new`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum DifficultyError {
    /// Value was NaN or ±infinity.
    #[error("difficulty must be finite")]
    NotFinite,
    /// Value was zero or negative.
    #[error("difficulty must be strictly positive")]
    NonPositive,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_positive_finite() {
        let d = ShareDifficulty::new(2048.0).expect("valid");
        assert!((d.value() - 2048.0).abs() < f64::EPSILON);
    }

    #[test]
    fn accepts_tiny_positive() {
        assert!(ShareDifficulty::new(f64::MIN_POSITIVE).is_ok());
    }

    #[test]
    fn rejects_zero() {
        assert_eq!(ShareDifficulty::new(0.0), Err(DifficultyError::NonPositive));
    }

    #[test]
    fn rejects_negative() {
        assert_eq!(
            ShareDifficulty::new(-1.0),
            Err(DifficultyError::NonPositive)
        );
    }

    #[test]
    fn rejects_nan() {
        assert_eq!(
            ShareDifficulty::new(f64::NAN),
            Err(DifficultyError::NotFinite)
        );
    }

    #[test]
    fn rejects_infinity() {
        assert_eq!(
            ShareDifficulty::new(f64::INFINITY),
            Err(DifficultyError::NotFinite)
        );
        assert_eq!(
            ShareDifficulty::new(f64::NEG_INFINITY),
            Err(DifficultyError::NotFinite)
        );
    }

    #[test]
    fn serde_roundtrip_via_json() {
        let d = ShareDifficulty::new(4096.5).expect("valid");
        let j = serde_json::to_string(&d).expect("serialize");
        let back: ShareDifficulty = serde_json::from_str(&j).expect("deserialize");
        assert_eq!(d, back);
    }
}
