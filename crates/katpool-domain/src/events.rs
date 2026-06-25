//! [`PoolEvent`] — events the stratum bridge publishes for the rest of
//! the pool to consume.
//!
//! Carried on a `tokio::sync::broadcast` channel inside the
//! single-binary katpool process. Receivers must be tolerant of slow-
//! consumer behaviour (the channel is bounded; lagging receivers see
//! `RecvError::Lagged`).
//!
//! ## Event boundaries
//!
//! The bridge owns the protocol-and-PoW concerns. Anything that
//! requires watching for coinbase maturity, computing PROP allocations,
//! or signing payout transactions happens **downstream** of the events
//! defined here.
//!
//! - [`PoolEvent::ShareCredited`] fires when a valid share is accepted
//!   from a connected miner. This is the primary signal the accountant
//!   uses to bookkeep per-miner work.
//! - [`PoolEvent::ShareRejected`] fires when the bridge refuses a share
//!   with a typed reason (stale, low-difficulty, bad `PoW`, etc.). The
//!   accountant uses these for per-miner reject counters; the anti-abuse
//!   layer uses them for ban-list decisioning.
//! - [`PoolEvent::BlockFound`] fires the instant a share's `PoW` meets the
//!   network target — **before** we submit the candidate block to
//!   `kaspad`. The accountant pre-records the candidate so that if `kaspad`
//!   takes a long time to confirm we still have an audit record.
//! - [`PoolEvent::BlockAccepted`] fires the instant `kaspad` responds with
//!   `Ok` to our `submit_block` call. This is **not** the coinbase
//!   maturity signal — the accountant emits its own follow-up event
//!   (Phase 3+) when it observes the matured coinbase via
//!   `UtxoProcessor`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    address::WalletAddress, correlation_id::CorrelationId, difficulty::ShareDifficulty,
    hash::BlockHash, score::DaaScore, worker::WorkerName,
};

/// Events emitted by the stratum bridge.
///
/// `#[non_exhaustive]` so future event variants don't break receivers.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum PoolEvent {
    /// A valid share was accepted from a miner.
    ShareCredited {
        /// The wallet credited.
        wallet: WalletAddress,
        /// The worker rig that submitted the share.
        worker: WorkerName,
        /// The pool difficulty assigned to this worker at submit time.
        difficulty: ShareDifficulty,
        /// The DAA score of the job the share was submitted against.
        daa_score: DaaScore,
        /// Wall-clock time of credit.
        ts: DateTime<Utc>,
        /// Tracing correlation id.
        correlation_id: CorrelationId,
    },
    /// A share was rejected.
    ShareRejected {
        /// The wallet whose submission was rejected.
        wallet: WalletAddress,
        /// The worker rig that submitted.
        worker: WorkerName,
        /// Why we rejected.
        reason: ShareRejectReason,
        /// Wall-clock time of rejection.
        ts: DateTime<Utc>,
        /// Tracing correlation id.
        correlation_id: CorrelationId,
    },
    /// A share's `PoW` met the network target — block candidate constructed.
    /// Fires **before** the candidate is submitted to `kaspad`.
    BlockFound {
        /// The wallet that submitted the winning share.
        wallet: WalletAddress,
        /// The worker rig.
        worker: WorkerName,
        /// The full block hash of the candidate.
        hash: BlockHash,
        /// The DAA score of the candidate.
        daa_score: DaaScore,
        /// Wall-clock time the bridge detected the candidate.
        ts: DateTime<Utc>,
        /// Tracing correlation id.
        correlation_id: CorrelationId,
    },
    /// `kaspad` responded `Ok` to our `submit_block` call. This is the
    /// bridge-immediate "accepted by the local node" event, not the
    /// coinbase-matured event — see the module-level docs.
    BlockAccepted {
        /// The block hash that was accepted.
        hash: BlockHash,
        /// Wall-clock time of acceptance.
        ts: DateTime<Utc>,
        /// Tracing correlation id; matches the corresponding
        /// `BlockFound` event so consumers can pair them.
        correlation_id: CorrelationId,
    },
    /// A stratum session authenticated (`mining.authorize`). Emitted once
    /// per connection so the accountant can `open` a live
    /// `connection_session` row immediately — the session becomes visible
    /// while still connected, with its worker bound from the start. The
    /// row is later finalized by the matching [`PoolEvent::SessionClosed`]
    /// (correlated by `conn_id`). The bridge owns no database, so this
    /// carries everything the row needs; the hot share path is untouched.
    SessionOpened {
        /// Process-unique connection id, correlating this open with its
        /// later `SessionClosed`.
        conn_id: u64,
        /// The authenticated wallet.
        wallet: Option<WalletAddress>,
        /// The worker rig, if the authorize payload carried one.
        worker: Option<WorkerName>,
        /// Remote miner IP (the real client IP after PROXY-protocol
        /// resolution), as text — parsed to `inet` by the consumer.
        remote_ip: String,
        /// Reported stratum `mining.subscribe` user-agent, if any.
        remote_app: Option<String>,
        /// When the underlying TCP session was accepted.
        connected_at: DateTime<Utc>,
        /// Tracing correlation id.
        correlation_id: CorrelationId,
    },
    /// A stratum TCP session ended. Emitted once per disconnect so the
    /// accountant can finalize the `connection_session` row: it closes the
    /// live row opened by the matching [`PoolEvent::SessionOpened`]
    /// (correlated by `conn_id`), or — for sessions that dropped before
    /// authorize (no open row) — persists a completed row for per-IP
    /// forensics and the firmware/device breakdown.
    SessionClosed {
        /// Process-unique connection id, correlating this close with its
        /// earlier `SessionOpened` (if the session authorized).
        conn_id: u64,
        /// The authenticated wallet, if the session reached authorize.
        wallet: Option<WalletAddress>,
        /// The worker rig, if the session reached authorize.
        worker: Option<WorkerName>,
        /// Remote miner IP (the real client IP after PROXY-protocol
        /// resolution), as text — parsed to `inet` by the consumer.
        remote_ip: String,
        /// Reported stratum `mining.subscribe` user-agent, if any.
        remote_app: Option<String>,
        /// When the TCP session was accepted.
        connected_at: DateTime<Utc>,
        /// When the session ended.
        ts: DateTime<Utc>,
        /// Tracing correlation id.
        correlation_id: CorrelationId,
    },
}

/// Typed reason for share rejection. Each variant maps to one concrete
/// rejection code path in `bridge/src/share_handler.rs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ShareRejectReason {
    /// Submission too old — block template already superseded.
    Stale,
    /// `PoW` didn't meet the worker's assigned pool difficulty.
    LowDifficulty,
    /// `kaspad` rejected the candidate block (typically bad `PoW`).
    BadPow,
    /// Stratum referenced a job id we don't recognise.
    MissingJob,
    /// Submission frame couldn't be parsed (wrong arity, bad types).
    MalformedFrame,
    /// Same submission already seen within the dedup window.
    DuplicateSubmit,
    /// Authenticated wallet address didn't parse as a Kaspa address.
    BadAddress,
}

impl ShareRejectReason {
    /// Stable lowercase string suitable for metrics labels and log fields.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Stale => "stale",
            Self::LowDifficulty => "low_difficulty",
            Self::BadPow => "bad_pow",
            Self::MissingJob => "missing_job",
            Self::MalformedFrame => "malformed_frame",
            Self::DuplicateSubmit => "duplicate_submit",
            Self::BadAddress => "bad_address",
        }
    }
}

impl std::fmt::Display for ShareRejectReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_wallet() -> WalletAddress {
        WalletAddress::new("kaspa:qz4j8mu269z8llgcczmfukm9fan2fq822kzxu4cfukd5fq").expect("valid")
    }

    fn sample_worker() -> WorkerName {
        WorkerName::new("rig-01").expect("valid")
    }

    fn sample_hash() -> BlockHash {
        BlockHash::from_hex("06acc7179752e80fa4ef421f3dd7ff5b5bda006e3fc76c14f33f324079a3a9e2")
            .expect("valid")
    }

    #[test]
    fn share_reject_reasons_have_stable_labels() {
        assert_eq!(ShareRejectReason::Stale.as_str(), "stale");
        assert_eq!(ShareRejectReason::LowDifficulty.as_str(), "low_difficulty");
        assert_eq!(ShareRejectReason::BadPow.as_str(), "bad_pow");
        assert_eq!(ShareRejectReason::MissingJob.as_str(), "missing_job");
        assert_eq!(
            ShareRejectReason::MalformedFrame.as_str(),
            "malformed_frame"
        );
        assert_eq!(
            ShareRejectReason::DuplicateSubmit.as_str(),
            "duplicate_submit"
        );
        assert_eq!(ShareRejectReason::BadAddress.as_str(), "bad_address");
    }

    #[test]
    fn share_credited_roundtrips_through_serde() {
        let event = PoolEvent::ShareCredited {
            wallet: sample_wallet(),
            worker: sample_worker(),
            difficulty: ShareDifficulty::new(2048.0).expect("valid"),
            daa_score: DaaScore::new(414_699_558),
            ts: Utc::now(),
            correlation_id: CorrelationId::new_v4(),
        };
        let json = serde_json::to_string(&event).expect("serialize");
        let back: PoolEvent = serde_json::from_str(&json).expect("deserialize");
        // Bit-equal serialisation roundtrip (the timestamps will compare equal
        // because chrono serializes/deserializes to nanosecond precision).
        let again = serde_json::to_string(&back).expect("re-serialize");
        assert_eq!(json, again);
    }

    #[test]
    fn share_rejected_carries_reason_in_serialization() {
        let event = PoolEvent::ShareRejected {
            wallet: sample_wallet(),
            worker: sample_worker(),
            reason: ShareRejectReason::Stale,
            ts: Utc::now(),
            correlation_id: CorrelationId::new_v4(),
        };
        let json = serde_json::to_string(&event).expect("serialize");
        assert!(json.contains("\"Stale\""), "missing reason in: {json}");
    }

    #[test]
    fn block_found_carries_hash_in_serialization() {
        let h = sample_hash();
        let event = PoolEvent::BlockFound {
            wallet: sample_wallet(),
            worker: sample_worker(),
            hash: h,
            daa_score: DaaScore::new(414_700_000),
            ts: Utc::now(),
            correlation_id: CorrelationId::new_v4(),
        };
        let json = serde_json::to_string(&event).expect("serialize");
        assert!(json.contains(&h.to_string()), "missing hash in: {json}");
    }

    #[test]
    fn session_opened_roundtrips_through_serde() {
        let event = PoolEvent::SessionOpened {
            conn_id: 42,
            wallet: Some(sample_wallet()),
            worker: Some(sample_worker()),
            remote_ip: "203.0.113.7".to_owned(),
            remote_app: Some("GodMiner/1.0".to_owned()),
            connected_at: Utc::now(),
            correlation_id: CorrelationId::new_v4(),
        };
        let json = serde_json::to_string(&event).expect("serialize");
        let back: PoolEvent = serde_json::from_str(&json).expect("deserialize");
        let again = serde_json::to_string(&back).expect("re-serialize");
        assert_eq!(json, again);
        assert!(matches!(back, PoolEvent::SessionOpened { conn_id: 42, .. }));
    }

    #[test]
    fn session_closed_roundtrips_through_serde() {
        let event = PoolEvent::SessionClosed {
            conn_id: 7,
            wallet: Some(sample_wallet()),
            worker: Some(sample_worker()),
            remote_ip: "203.0.113.7".to_owned(),
            remote_app: Some("GodMiner/1.0".to_owned()),
            connected_at: Utc::now(),
            ts: Utc::now(),
            correlation_id: CorrelationId::new_v4(),
        };
        let json = serde_json::to_string(&event).expect("serialize");
        let back: PoolEvent = serde_json::from_str(&json).expect("deserialize");
        let again = serde_json::to_string(&back).expect("re-serialize");
        assert_eq!(json, again);
        assert!(json.contains("GodMiner/1.0"), "missing app in: {json}");
    }

    #[test]
    fn session_closed_allows_anonymous_session() {
        let event = PoolEvent::SessionClosed {
            conn_id: 0,
            wallet: None,
            worker: None,
            remote_ip: "2001:db8::1".to_owned(),
            remote_app: None,
            connected_at: Utc::now(),
            ts: Utc::now(),
            correlation_id: CorrelationId::new_v4(),
        };
        let json = serde_json::to_string(&event).expect("serialize");
        let back: PoolEvent = serde_json::from_str(&json).expect("deserialize");
        assert!(matches!(
            back,
            PoolEvent::SessionClosed { wallet: None, .. }
        ));
    }

    #[test]
    fn block_accepted_carries_only_hash_and_metadata() {
        let h = sample_hash();
        let event = PoolEvent::BlockAccepted {
            hash: h,
            ts: Utc::now(),
            correlation_id: CorrelationId::new_v4(),
        };
        let json = serde_json::to_string(&event).expect("serialize");
        assert!(json.contains(&h.to_string()), "missing hash in: {json}");
        assert!(json.contains("BlockAccepted"));
    }
}
