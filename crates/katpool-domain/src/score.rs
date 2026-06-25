//! [`DaaScore`] — Difficulty Adjusted Accumulation score, the `BlockDAG`'s
//! analogue of block height.
//!
//! Within the Kaspa consensus core this is a `u64`. We wrap it as a
//! newtype so functions like `f(daa_score, block_count)` can't be called
//! with the arguments transposed.

use std::fmt;

use serde::{Deserialize, Serialize};

/// A DAA score (`BlockDAG` ordering value).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DaaScore(u64);

impl DaaScore {
    /// Construct from a raw u64.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Underlying u64 value.
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

impl fmt::Display for DaaScore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u64> for DaaScore {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<DaaScore> for u64 {
    fn from(value: DaaScore) -> Self {
        value.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips() {
        let s = DaaScore::new(414_699_558);
        assert_eq!(s.value(), 414_699_558);
        assert_eq!(format!("{s}"), "414699558");
    }

    #[test]
    fn orders_naturally() {
        assert!(DaaScore::new(10) < DaaScore::new(20));
        assert!(DaaScore::new(u64::MAX) > DaaScore::new(0));
    }

    #[test]
    fn serde_roundtrip_via_json() {
        let s = DaaScore::new(123_456_789);
        let j = serde_json::to_string(&s).expect("serialize");
        assert_eq!(j, "123456789");
        let back: DaaScore = serde_json::from_str(&j).expect("deserialize");
        assert_eq!(s, back);
    }
}
