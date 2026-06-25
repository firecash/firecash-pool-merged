//! [`BlockHash`] — a 32-byte Kaspa block hash.
//!
//! Wraps a `[u8; 32]` with explicit hex parsing and lowercase-canonical
//! display. Rejects non-hex input. Eliminates the "we passed a wallet
//! address where we expected a block hash" class of mix-up by being a
//! distinct type.

use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// 32-byte Kaspa block hash.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BlockHash(#[serde(with = "hex_serde")] [u8; 32]);

impl BlockHash {
    /// Construct from a 32-byte array.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Parse from a 64-character lowercase hex string.
    pub fn from_hex(s: &str) -> Result<Self, BlockHashError> {
        if s.len() != 64 {
            return Err(BlockHashError::WrongLength { len: s.len() });
        }
        let mut bytes = [0u8; 32];
        hex::decode_to_slice(s, &mut bytes).map_err(|_| BlockHashError::InvalidHex)?;
        Ok(Self(bytes))
    }

    /// Borrow the inner 32-byte array.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for BlockHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in &self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

// Avoid leaking byte content via auto-derived Debug, but produce a useful
// rendering that callers can rely on for logs/traces.
impl fmt::Debug for BlockHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BlockHash({self})")
    }
}

/// Errors from [`BlockHash::from_hex`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum BlockHashError {
    /// Input was not 64 hex characters.
    #[error("expected 64 hex characters, got {len}")]
    WrongLength {
        /// Observed length.
        len: usize,
    },
    /// Input contained non-hex characters.
    #[error("invalid hex characters in block hash")]
    InvalidHex,
}

mod hex_serde {
    use serde::{Deserialize, Deserializer, Serializer, de::Error as _};

    pub fn serialize<S: Serializer>(bytes: &[u8; 32], s: S) -> Result<S::Ok, S::Error> {
        let mut buf = [0u8; 64];
        hex::encode_to_slice(bytes, &mut buf).map_err(serde::ser::Error::custom)?;
        // SAFETY: hex::encode_to_slice writes ASCII bytes only.
        let as_str = core::str::from_utf8(&buf).map_err(serde::ser::Error::custom)?;
        s.serialize_str(as_str)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; 32], D::Error> {
        let s = String::deserialize(d)?;
        if s.len() != 64 {
            return Err(D::Error::custom(format!(
                "expected 64 hex characters, got {}",
                s.len()
            )));
        }
        let mut out = [0u8; 32];
        hex::decode_to_slice(&s, &mut out)
            .map_err(|_| D::Error::custom("invalid hex characters in block hash"))?;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "06acc7179752e80fa4ef421f3dd7ff5b5bda006e3fc76c14f33f324079a3a9e2";

    #[test]
    fn round_trip_hex() {
        let h = BlockHash::from_hex(SAMPLE).expect("valid");
        assert_eq!(h.to_string(), SAMPLE);
    }

    #[test]
    fn round_trip_bytes() {
        let h = BlockHash::from_hex(SAMPLE).expect("valid");
        let h2 = BlockHash::from_bytes(*h.as_bytes());
        assert_eq!(h, h2);
    }

    #[test]
    fn rejects_wrong_length() {
        assert!(matches!(
            BlockHash::from_hex("deadbeef"),
            Err(BlockHashError::WrongLength { len: 8 })
        ));
    }

    #[test]
    fn rejects_non_hex() {
        let mut s = SAMPLE.to_string();
        s.replace_range(0..1, "Z");
        assert_eq!(BlockHash::from_hex(&s), Err(BlockHashError::InvalidHex));
    }

    #[test]
    fn debug_does_not_leak_internals() {
        let h = BlockHash::from_hex(SAMPLE).expect("valid");
        let dbg = format!("{h:?}");
        assert!(dbg.starts_with("BlockHash("));
        assert!(dbg.contains(SAMPLE));
        // Confirm no raw byte values appear in Debug — the [u8; 32] array
        // representation would look like "[6, 172, 199, ...]".
        assert!(!dbg.contains(", 172,"));
    }

    #[test]
    fn serde_roundtrip_via_json() {
        let h = BlockHash::from_hex(SAMPLE).expect("valid");
        let j = serde_json::to_string(&h).expect("serialize");
        assert_eq!(j, format!("\"{SAMPLE}\""));
        let back: BlockHash = serde_json::from_str(&j).expect("deserialize");
        assert_eq!(h, back);
    }
}
