//! [`CorrelationId`] — a UUID v4 used as a tracing/log correlation key.
//!
//! Propagated through `tracing::Span`s and embedded in every [`PoolEvent`]
//! we emit so an operator can follow a single share/block through the
//! bridge, the accountant, the payout engine, and the API logs by a
//! single grep.
//!
//! [`PoolEvent`]: crate::events::PoolEvent

use std::fmt;

use serde::{Deserialize, Serialize};

/// A UUID v4 used as a tracing correlation identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CorrelationId(uuid::Uuid);

impl CorrelationId {
    /// Generate a fresh random correlation id.
    #[must_use]
    pub fn new_v4() -> Self {
        Self(uuid::Uuid::new_v4())
    }

    /// Construct from an existing UUID (e.g. one propagated from upstream).
    #[must_use]
    pub const fn from_uuid(id: uuid::Uuid) -> Self {
        Self(id)
    }

    /// Underlying [`uuid::Uuid`].
    #[must_use]
    pub const fn as_uuid(&self) -> &uuid::Uuid {
        &self.0
    }
}

impl fmt::Display for CorrelationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // uuid's Display impl already produces the canonical 8-4-4-4-12 hex.
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn freshly_generated_ids_differ() {
        let a = CorrelationId::new_v4();
        let b = CorrelationId::new_v4();
        assert_ne!(a, b);
    }

    #[test]
    fn display_is_canonical_uuid() {
        let id = CorrelationId::from_uuid(uuid::uuid!("550e8400-e29b-41d4-a716-446655440000"));
        assert_eq!(format!("{id}"), "550e8400-e29b-41d4-a716-446655440000");
    }

    #[test]
    fn serde_roundtrip_via_json() {
        let id = CorrelationId::new_v4();
        let j = serde_json::to_string(&id).expect("serialize");
        let back: CorrelationId = serde_json::from_str(&j).expect("deserialize");
        assert_eq!(id, back);
    }
}
