//! Postgres session advisory locks for single-leader background work.
//!
//! The KAS payout engine runs on every `katpool` instance, but only one may
//! drive a cycle at a time. A Postgres *session* advisory lock gives us mutual
//! exclusion without a lock table or TTL bookkeeping: at most one session holds
//! a given key, and the lock is released the instant that session ends.
//!
//! [`AdvisoryLock`] backs the lock with a connection **detached** from the pool
//! ([`sqlx::pool::PoolConnection::detach`]), so correctness does not depend on
//! the happy path: if the guard is dropped — even by a panic — the owned
//! [`PgConnection`] closes and Postgres frees the lock server-side. Returning a
//! still-locked connection to the pool (the failure mode of locking on a
//! pooled connection) cannot happen.

use katpool_db::DbError;
use sqlx::{Connection, PgConnection, PgPool};
use tracing::warn;

/// A held Postgres session advisory lock.
///
/// Acquire with [`AdvisoryLock::try_acquire`]; release with
/// [`AdvisoryLock::release`] for the clean path. Dropping without releasing is
/// safe but logs a warning — the backing connection close frees the lock.
pub struct AdvisoryLock {
    conn: Option<PgConnection>,
    key: i64,
}

impl AdvisoryLock {
    /// Try to acquire the advisory lock for `key` without blocking.
    ///
    /// Returns `Ok(None)` when another session already holds it: the caller is
    /// not the leader this round and should skip its work and retry later.
    pub async fn try_acquire(pool: &PgPool, key: i64) -> Result<Option<Self>, DbError> {
        let mut conn = pool.acquire().await?.detach();
        let acquired: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
            .bind(key)
            .fetch_one(&mut conn)
            .await?;
        if acquired {
            Ok(Some(Self {
                conn: Some(conn),
                key,
            }))
        } else {
            // We hold nothing — close the helper connection promptly rather
            // than leak it out of the pool.
            let _ = conn.close().await;
            Ok(None)
        }
    }

    /// The advisory key this guard holds.
    #[must_use]
    pub const fn key(&self) -> i64 {
        self.key
    }

    /// Release the lock explicitly and close the backing connection.
    pub async fn release(mut self) -> Result<(), DbError> {
        if let Some(mut conn) = self.conn.take() {
            sqlx::query("SELECT pg_advisory_unlock($1)")
                .bind(self.key)
                .execute(&mut conn)
                .await?;
            let _ = conn.close().await;
        }
        Ok(())
    }
}

impl Drop for AdvisoryLock {
    fn drop(&mut self) {
        if self.conn.is_some() {
            // Cannot run async unlock here; dropping the owned PgConnection
            // closes the session, which frees the advisory lock server-side.
            warn!(
                key = self.key,
                "advisory lock dropped without explicit release; connection \
                 close frees it server-side"
            );
        }
    }
}

/// Derive a stable 64-bit advisory key from a namespace string (FNV-1a).
///
/// Advisory keys are global per database, so callers use a descriptive
/// namespace (e.g. `"payout-kas:kas-leader"`) and let this map it to the
/// `bigint` key deterministically across processes and releases.
#[must_use]
pub fn advisory_key(namespace: &str) -> i64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in namespace.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    // Reinterpret the bits (not a lossy numeric cast) into the signed key.
    i64::from_ne_bytes(hash.to_ne_bytes())
}
