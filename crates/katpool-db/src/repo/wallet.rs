//! Wallet aggregate.
//!
//! Wallets are the root identity in the schema — every share,
//! worker, block, and payout chains back to a `wallet_id`. The
//! [`ensure`] function is the workhorse: idempotent upsert by
//! address, refresh of `last_seen_at` on hit.

use chrono::{DateTime, Utc};
use katpool_domain::WalletAddress;
use sqlx::PgExecutor;

use crate::DbError;
use crate::repo::WalletId;

/// A row from the `wallet` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Wallet {
    /// Synthetic primary key.
    pub id: WalletId,
    /// Canonical bech32 address.
    pub address: String,
    /// `mainnet`, `testnet-10`, `testnet-11`, `devnet`, or `simnet`.
    pub network: String,
    /// First time we observed this wallet on this network.
    pub first_seen_at: DateTime<Utc>,
    /// Last time the application touched this row.
    pub last_seen_at: DateTime<Utc>,
}

/// Find a wallet by address; create it if missing.
///
/// On a hit, refreshes `last_seen_at`. Idempotent and safe to call
/// once per `ShareCredited` event without race-condition concerns —
/// the `ON CONFLICT` is atomic.
///
/// The DB's `wallet_address_format` CHECK constraint will refuse a
/// network/address-prefix mismatch (e.g. `address` starting with
/// `kaspa:` but `network = 'testnet-10'`). The caller sees that as
/// [`DbError::Constraint`] with SQLSTATE `23514`.
pub async fn ensure<'e, E>(
    executor: E,
    address: &WalletAddress,
    network: &str,
) -> Result<Wallet, DbError>
where
    E: PgExecutor<'e>,
{
    sqlx::query_as::<_, Wallet>(
        "
        INSERT INTO wallet (address, network)
        VALUES ($1, $2)
        ON CONFLICT (address) DO UPDATE
            SET last_seen_at = now()
        RETURNING id, address, network, first_seen_at, last_seen_at
        ",
    )
    .bind(address.as_str())
    .bind(network)
    .fetch_one(executor)
    .await
    .map_err(DbError::from)
}

/// Fetch a wallet by primary key. Returns [`DbError::NotFound`] if no
/// such row exists.
pub async fn get_by_id<'e, E: PgExecutor<'e>>(
    executor: E,
    id: WalletId,
) -> Result<Wallet, DbError> {
    sqlx::query_as::<_, Wallet>(
        "SELECT id, address, network, first_seen_at, last_seen_at FROM wallet WHERE id = $1",
    )
    .bind(id.0)
    .fetch_one(executor)
    .await
    .map_err(DbError::from)
}

/// Look up a wallet by address. Returns `Ok(None)` rather than an
/// error if not found — useful for the API layer's "does this wallet
/// have any history" queries.
pub async fn find_by_address<'e, E: PgExecutor<'e>>(
    executor: E,
    address: &WalletAddress,
) -> Result<Option<Wallet>, DbError> {
    sqlx::query_as::<_, Wallet>(
        "SELECT id, address, network, first_seen_at, last_seen_at FROM wallet WHERE address = $1",
    )
    .bind(address.as_str())
    .fetch_optional(executor)
    .await
    .map_err(DbError::from)
}
