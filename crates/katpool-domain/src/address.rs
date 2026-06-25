//! [`WalletAddress`] — a validated Kaspa wallet address.
//!
//! Phase 1 validation is intentionally minimal: parse the prefix
//! (`kaspa:` or `kaspatest:`) and require a reasonable bech32 body length
//! and character set. Full bech32 verification with the kaspa-addresses
//! crate lands in Phase 4 when the payout engine actually constructs
//! transactions; here we just want to reject the gross malformations
//! that would smuggle non-address strings through stratum into the
//! accounting layer.

use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A validated Kaspa wallet address. Always serializes as its canonical
/// string form. Construction goes through [`WalletAddress::new`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WalletAddress(String);

impl WalletAddress {
    /// Minimum total length: `"kaspa:"` (6) + an absolute floor of 8 body
    /// characters. Real Kaspa addresses are far longer; this is just a
    /// "definitely garbage" rejection floor.
    pub const MIN_TOTAL_LEN: usize = 14;

    /// Maximum total length we'll consider plausible. Real bech32 Kaspa
    /// addresses are ~70 characters; we cap at 96 to leave headroom and
    /// still bound memory.
    pub const MAX_TOTAL_LEN: usize = 96;

    /// Parse and validate the given string as a Kaspa wallet address.
    ///
    /// Returns an [`AddressError`] describing the first failing check.
    pub fn new(s: impl Into<String>) -> Result<Self, AddressError> {
        let s: String = s.into();
        let body = if let Some(rest) = s.strip_prefix("kaspa:") {
            rest
        } else if let Some(rest) = s.strip_prefix("kaspatest:") {
            rest
        } else {
            return Err(AddressError::InvalidPrefix);
        };

        if s.len() < Self::MIN_TOTAL_LEN {
            return Err(AddressError::TooShort { len: s.len() });
        }
        if s.len() > Self::MAX_TOTAL_LEN {
            return Err(AddressError::TooLong { len: s.len() });
        }

        // bech32 body alphabet: a-z and 0-9 only.
        for ch in body.chars() {
            if !(ch.is_ascii_lowercase() || ch.is_ascii_digit()) {
                return Err(AddressError::InvalidCharacter { ch });
            }
        }

        Ok(Self(s))
    }

    /// Canonical string form.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for WalletAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Errors from [`WalletAddress::new`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum AddressError {
    /// String didn't start with `kaspa:` or `kaspatest:`.
    #[error("address must start with `kaspa:` or `kaspatest:`")]
    InvalidPrefix,
    /// Address too short to be a real bech32 Kaspa address.
    #[error("address length {len} is below the minimum")]
    TooShort {
        /// Observed length.
        len: usize,
    },
    /// Address longer than any real Kaspa address.
    #[error("address length {len} exceeds the maximum")]
    TooLong {
        /// Observed length.
        len: usize,
    },
    /// Address body contained a non-bech32 character.
    #[error("address body contains invalid character `{ch}`")]
    InvalidCharacter {
        /// Offending character.
        ch: char,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_mainnet_address() {
        let a = WalletAddress::new(
            "kaspa:qz4j8mu269z8llgcczmfukm9fan2fq822kzxu4cfukd5fqrhxpsv2zhs9jxnp",
        )
        .expect("valid address");
        assert_eq!(
            a.as_str(),
            "kaspa:qz4j8mu269z8llgcczmfukm9fan2fq822kzxu4cfukd5fqrhxpsv2zhs9jxnp"
        );
    }

    #[test]
    fn accepts_testnet_address() {
        let a =
            WalletAddress::new("kaspatest:qrxd24c5w6pl2qa9k7q5e0lyepuu4r5t2f6awvxllk0a83qqfys9")
                .expect("valid testnet address");
        assert!(a.as_str().starts_with("kaspatest:"));
    }

    #[test]
    fn rejects_missing_prefix() {
        assert_eq!(
            WalletAddress::new("qz4j8mu269z8llgcczmfukm9fan2fq822kzxu4cfukd"),
            Err(AddressError::InvalidPrefix)
        );
    }

    #[test]
    fn rejects_too_short() {
        assert!(matches!(
            WalletAddress::new("kaspa:q"),
            Err(AddressError::TooShort { .. })
        ));
    }

    #[test]
    fn rejects_too_long() {
        let body: String = "q".repeat(200);
        assert!(matches!(
            WalletAddress::new(format!("kaspa:{body}")),
            Err(AddressError::TooLong { .. })
        ));
    }

    #[test]
    fn rejects_uppercase_in_body() {
        assert_eq!(
            WalletAddress::new("kaspa:QZ4J8MU269Z8LLGCCZMFUKM9FAN2FQ822KZXU4CFUK"),
            Err(AddressError::InvalidCharacter { ch: 'Q' })
        );
    }

    #[test]
    fn rejects_punctuation_in_body() {
        assert!(matches!(
            WalletAddress::new("kaspa:qz4j8mu269z8llg.czmfukm9fan2fq822kzxu4cfukd5fq"),
            Err(AddressError::InvalidCharacter { ch: '.' })
        ));
    }

    #[test]
    fn rejects_non_ascii() {
        assert!(matches!(
            WalletAddress::new("kaspa:qz4j8mu269z8llgcczmfukmßfan2fq822kzxu4cfukd5fq"),
            Err(AddressError::InvalidCharacter { .. })
        ));
    }

    #[test]
    fn display_matches_input() {
        let a = WalletAddress::new("kaspa:qz4j8mu269z8llgcczmfukm9fan2fq822kzxu4cfukd5fq")
            .expect("valid");
        assert_eq!(
            format!("{a}"),
            "kaspa:qz4j8mu269z8llgcczmfukm9fan2fq822kzxu4cfukd5fq"
        );
    }

    #[test]
    fn serde_roundtrip_via_json() {
        let a = WalletAddress::new("kaspa:qz4j8mu269z8llgcczmfukm9fan2fq822kzxu4cfukd5fq")
            .expect("valid");
        let json = serde_json::to_string(&a).expect("serialize");
        let back: WalletAddress = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(a, back);
    }
}
