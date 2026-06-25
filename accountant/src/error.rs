//! Typed errors surfaced by the accountant.
//!
//! Split into setup errors ([`AccountantError`]) which are fatal
//! at startup and per-event errors ([`EventError`]) which the
//! consumer recovers from by logging + a metric tick + continuing
//! to drain the channel.

use thiserror::Error;

/// Setup-time errors.
///
/// Bad config, can't open the DB pool, etc.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum AccountantError {
    /// Fee config validation failed (out-of-range basis points,
    /// negative threshold, etc.).
    #[error("invalid fee config: {0}")]
    Config(String),

    /// Database error encountered during setup (typically a
    /// connection failure during pool warm-up).
    #[error("database error during setup")]
    Db(#[from] katpool_db::DbError),
}

/// Per-event errors.
///
/// The consumer logs these against the triggering event's
/// `correlation_id` and increments a metric; it does **not** abort
/// the consumer task. The same kind never retries the same event
/// automatically — the broadcast channel model is at-most-once.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EventError {
    /// `wallet::ensure` failed.
    #[error("wallet upsert failed: {0}")]
    WalletEnsure(katpool_db::DbError),

    /// `worker::ensure` failed.
    #[error("worker upsert failed: {0}")]
    WorkerEnsure(katpool_db::DbError),

    /// `share::insert_credited` failed.
    #[error("share insert failed: {0}")]
    ShareInsert(katpool_db::DbError),

    /// `share_reject::insert` failed.
    #[error("share_reject insert failed: {0}")]
    ShareRejectInsert(katpool_db::DbError),

    /// A `ShareRejected` event arrived with a domain reason that
    /// the DB enum doesn't yet know. Means the bridge added a
    /// `non_exhaustive` variant without a paired schema migration.
    /// Recoverable: log + metric, skip persistence.
    #[error("unknown share-reject reason `{reason}`; add a migration")]
    UnknownRejectReason {
        /// `as_str()` label of the unmapped reason.
        reason: &'static str,
    },

    /// `block::ensure` failed.
    #[error("block ensure failed: {0}")]
    BlockEnsure(katpool_db::DbError),

    /// `block::mark_submitted` failed.
    #[error("block mark_submitted failed: {0}")]
    BlockMarkSubmitted(katpool_db::DbError),

    /// A `BlockAccepted` event arrived for a hash with no prior
    /// `BlockFound`. The accountant cannot construct a `block`
    /// row from `BlockAccepted` alone (no finder wallet/worker,
    /// no `daa_score`, no nonce), so it logs + counts and moves on.
    #[error("block accepted without prior found: hash={hash}")]
    OrphanBlockAccepted {
        /// Block hash from the accepted event, hex-encoded.
        hash: String,
    },

    /// A `SessionClosed` event carried a `remote_ip` that didn't parse
    /// as an IP address. Recoverable: log + metric, skip persistence.
    #[error("session close had unparsable remote_ip `{ip}`")]
    SessionBadIp {
        /// The raw value the bridge sent.
        ip: String,
    },

    /// `connection_session::record_closed` (or its worker upsert) failed.
    #[error("session record failed: {0}")]
    SessionRecord(katpool_db::DbError),
}
