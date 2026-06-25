//! [`WorkerName`] — a validated stratum-protocol worker label.
//!
//! A worker name is the free-form per-rig identifier following the `.` in a
//! stratum login of the form `kaspa:<address>.<worker>`. Different miners
//! send wildly different formats here, but the bridge sees it as a short
//! ASCII string. We bound the length and reject non-ASCII characters to
//! avoid log/metric/dashboard injection.

use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A validated worker name. ASCII only, length-bounded.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorkerName(String);

impl WorkerName {
    /// Maximum allowed length. Longer would trigger label-cardinality issues
    /// in Prometheus and runaway dashboard widths.
    pub const MAX_LEN: usize = 64;

    /// Construct a validated worker name.
    pub fn new(s: impl Into<String>) -> Result<Self, WorkerNameError> {
        let s: String = s.into();
        if s.is_empty() {
            return Err(WorkerNameError::Empty);
        }
        if s.len() > Self::MAX_LEN {
            return Err(WorkerNameError::TooLong { len: s.len() });
        }
        for ch in s.chars() {
            if !is_safe_worker_char(ch) {
                return Err(WorkerNameError::InvalidCharacter { ch });
            }
        }
        Ok(Self(s))
    }

    /// Borrow the underlying string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Allowed character set for a worker name: ASCII alphanumeric plus a
/// handful of common separators we've observed in production stratum
/// logins. Control characters, non-ASCII, and quotation/punctuation that
/// could break logfmt parsing are rejected.
const fn is_safe_worker_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '#' | '@' | ':' | '+' | '/' | '\\')
}

impl fmt::Display for WorkerName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Errors from [`WorkerName::new`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum WorkerNameError {
    /// Worker name was empty.
    #[error("worker name cannot be empty")]
    Empty,
    /// Worker name longer than [`WorkerName::MAX_LEN`].
    #[error(
        "worker name length {len} exceeds the maximum of {}",
        WorkerName::MAX_LEN
    )]
    TooLong {
        /// Observed length.
        len: usize,
    },
    /// Worker name contained a forbidden character.
    #[error("worker name contains invalid character `{ch}`")]
    InvalidCharacter {
        /// Offending character.
        ch: char,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_simple_alphanumeric() {
        let w = WorkerName::new("KS5Pro01").expect("valid");
        assert_eq!(w.as_str(), "KS5Pro01");
    }

    #[test]
    fn accepts_separators_we_see_in_prod() {
        for s in ["worker-1", "worker_1", "rig.1", "rig#1", "rig@1", "rig:1"] {
            assert!(WorkerName::new(s).is_ok(), "rejected legitimate input: {s}");
        }
    }

    #[test]
    fn rejects_empty() {
        assert_eq!(WorkerName::new(""), Err(WorkerNameError::Empty));
    }

    #[test]
    fn rejects_too_long() {
        let s: String = "a".repeat(WorkerName::MAX_LEN + 1);
        assert!(matches!(
            WorkerName::new(s),
            Err(WorkerNameError::TooLong { .. })
        ));
    }

    #[test]
    fn rejects_control_chars() {
        assert!(matches!(
            WorkerName::new("worker\nflood"),
            Err(WorkerNameError::InvalidCharacter { ch: '\n' })
        ));
    }

    #[test]
    fn rejects_non_ascii() {
        assert!(matches!(
            WorkerName::new("worker🔥"),
            Err(WorkerNameError::InvalidCharacter { .. })
        ));
    }

    #[test]
    fn rejects_quotes_and_punctuation_that_would_break_logs() {
        for ch in ['"', '\'', '`', ';', ' ', '\t'] {
            let s = format!("rig{ch}");
            assert!(
                matches!(
                    WorkerName::new(&s),
                    Err(WorkerNameError::InvalidCharacter { .. })
                ),
                "expected reject for char {ch:?}"
            );
        }
    }

    #[test]
    fn serde_roundtrip_via_json() {
        let w = WorkerName::new("KS5Pro01").expect("valid");
        let j = serde_json::to_string(&w).expect("serialize");
        let back: WorkerName = serde_json::from_str(&j).expect("deserialize");
        assert_eq!(w, back);
    }
}
