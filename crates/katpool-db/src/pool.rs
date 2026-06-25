//! Connection pool builder.
//!
//! The new pool process owns one [`sqlx::PgPool`] for the lifetime of the
//! process. Every service crate (`accountant`, `payout-kas`,
//! `payout-krc20`, `api`) takes a shared clone — sqlx's `PgPool` is
//! `Clone` and reference-counted internally.
//!
//! Sizing defaults are tuned for the production budget in
//! `docs/capacity-plan.md`: at peak ~50–200 shares/sec hitting the DB,
//! a pool of 16–32 connections is generous (median per-query latency
//! is sub-millisecond on the recommended postgres-17 + SSD).

use std::time::Duration;

use sqlx::ConnectOptions;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};

use crate::error::DbError;

/// Operator-tunable pool configuration.
///
/// Production defaults live in [`PoolConfig::production`]; the
/// `from_env` constructor overlays env vars on top, in the same
/// fail-fast style used by the bridge's `AntiAbuseConfig`.
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// `postgres://user:pass@host:port/db` URL.
    ///
    /// **Never** committed. In CI this comes from the test runner;
    /// in production from systemd `Environment=` or sops-decrypted env.
    pub url: String,
    /// Min pool size.
    ///
    /// Holds these connections idle so cold paths don't pay handshake latency.
    pub min_connections: u32,
    /// Hard cap on connection count.
    ///
    /// Postgres-side `max_connections` must be set higher (we recommend
    /// at least 2× the application cap to leave headroom for `pg_dump`
    /// / psql / `pgBackRest`).
    pub max_connections: u32,
    /// Max time a caller will wait for a connection from the pool
    /// before [`DbError::AcquireTimeout`] surfaces.
    pub acquire_timeout: Duration,
    /// Connections idle past this duration are reaped.
    pub idle_timeout: Duration,
    /// Connections older than this are recycled (avoids any chance of
    /// long-lived state leaks from postgres).
    pub max_lifetime: Duration,
    /// `statement_timeout` set on every checked-out connection.
    /// Bounds runaway queries from taking down the pool.
    pub statement_timeout: Duration,
    /// Application name surfaced in `pg_stat_activity`. Lets operators
    /// distinguish katpool connections from pgBackRest / psql / etc.
    pub application_name: String,
}

impl PoolConfig {
    /// Production-grade defaults.
    #[must_use]
    pub fn production(url: String) -> Self {
        Self {
            url,
            min_connections: 4,
            max_connections: 32,
            acquire_timeout: Duration::from_secs(5),
            idle_timeout: Duration::from_secs(600),
            max_lifetime: Duration::from_secs(3600),
            statement_timeout: Duration::from_secs(30),
            application_name: "katpool".to_owned(),
        }
    }

    /// Pure environment-style lookup. Mirrors the pattern in
    /// `bridge/src/anti_abuse.rs` (`from_lookup` + `from_env`).
    ///
    /// Recognised keys:
    ///
    /// | Key | Type | Required? |
    /// |---|---|---|
    /// | `KATPOOL_DATABASE_URL` | `text` | yes |
    /// | `KATPOOL_DB_MIN_CONNECTIONS` | `u32` | optional |
    /// | `KATPOOL_DB_MAX_CONNECTIONS` | `u32` | optional |
    /// | `KATPOOL_DB_ACQUIRE_TIMEOUT_SECS` | `u64` | optional |
    /// | `KATPOOL_DB_IDLE_TIMEOUT_SECS` | `u64` | optional |
    /// | `KATPOOL_DB_MAX_LIFETIME_SECS` | `u64` | optional |
    /// | `KATPOOL_DB_STATEMENT_TIMEOUT_SECS` | `u64` | optional |
    /// | `KATPOOL_DB_APPLICATION_NAME` | `text` | optional |
    pub fn from_lookup<F>(lookup: F) -> Result<Self, DbError>
    where
        F: Fn(&str) -> Option<String>,
    {
        let url = lookup("KATPOOL_DATABASE_URL").ok_or_else(|| DbError::Config {
            message: "KATPOOL_DATABASE_URL must be set".to_owned(),
        })?;
        let mut cfg = Self::production(url);

        if let Some(raw) = lookup("KATPOOL_DB_MIN_CONNECTIONS") {
            cfg.min_connections = parse_u32(&raw, "KATPOOL_DB_MIN_CONNECTIONS")?;
        }
        if let Some(raw) = lookup("KATPOOL_DB_MAX_CONNECTIONS") {
            cfg.max_connections = parse_u32(&raw, "KATPOOL_DB_MAX_CONNECTIONS")?;
        }
        if let Some(raw) = lookup("KATPOOL_DB_ACQUIRE_TIMEOUT_SECS") {
            cfg.acquire_timeout =
                Duration::from_secs(parse_u64(&raw, "KATPOOL_DB_ACQUIRE_TIMEOUT_SECS")?);
        }
        if let Some(raw) = lookup("KATPOOL_DB_IDLE_TIMEOUT_SECS") {
            cfg.idle_timeout =
                Duration::from_secs(parse_u64(&raw, "KATPOOL_DB_IDLE_TIMEOUT_SECS")?);
        }
        if let Some(raw) = lookup("KATPOOL_DB_MAX_LIFETIME_SECS") {
            cfg.max_lifetime =
                Duration::from_secs(parse_u64(&raw, "KATPOOL_DB_MAX_LIFETIME_SECS")?);
        }
        if let Some(raw) = lookup("KATPOOL_DB_STATEMENT_TIMEOUT_SECS") {
            cfg.statement_timeout =
                Duration::from_secs(parse_u64(&raw, "KATPOOL_DB_STATEMENT_TIMEOUT_SECS")?);
        }
        if let Some(raw) = lookup("KATPOOL_DB_APPLICATION_NAME") {
            cfg.application_name = raw;
        }

        cfg.validate()?;
        Ok(cfg)
    }

    /// Production wrapper.
    pub fn from_env() -> Result<Self, DbError> {
        Self::from_lookup(|k| std::env::var(k).ok())
    }

    fn validate(&self) -> Result<(), DbError> {
        if self.url.is_empty() {
            return Err(DbError::Config {
                message: "DATABASE_URL must not be empty".to_owned(),
            });
        }
        if self.max_connections == 0 {
            return Err(DbError::Config {
                message: "max_connections must be > 0".to_owned(),
            });
        }
        if self.min_connections > self.max_connections {
            return Err(DbError::Config {
                message: format!(
                    "min_connections ({}) must be <= max_connections ({})",
                    self.min_connections, self.max_connections
                ),
            });
        }
        if self.statement_timeout.is_zero() {
            return Err(DbError::Config {
                message: "statement_timeout must be > 0".to_owned(),
            });
        }
        Ok(())
    }
}

fn parse_u32(raw: &str, key: &str) -> Result<u32, DbError> {
    raw.parse::<u32>().map_err(|_| DbError::Config {
        message: format!("`{key}` is not a valid u32: `{raw}`"),
    })
}

fn parse_u64(raw: &str, key: &str) -> Result<u64, DbError> {
    raw.parse::<u64>().map_err(|_| DbError::Config {
        message: format!("`{key}` is not a valid u64: `{raw}`"),
    })
}

/// Build a `PgPool` from a [`PoolConfig`].
///
/// Sets the per-connection `statement_timeout` and `application_name`
/// via the connect options, so every checked-out connection arrives
/// with the right guard rails already applied.
pub async fn build_pool(cfg: &PoolConfig) -> Result<sqlx::PgPool, DbError> {
    let statement_timeout_ms =
        u64::try_from(cfg.statement_timeout.as_millis()).map_err(|_| DbError::Config {
            message: "statement_timeout overflows u64 milliseconds".to_owned(),
        })?;

    let connect_opts: PgConnectOptions = cfg
        .url
        .parse::<PgConnectOptions>()
        .map_err(|e| DbError::Config { message: format!("invalid DATABASE_URL: {e}") })?
        .application_name(&cfg.application_name)
        .options([("statement_timeout", statement_timeout_ms.to_string().as_str())])
        // Disable sqlx's own per-statement log; tracing-subscriber owns logging.
        .disable_statement_logging();

    let pool = PgPoolOptions::new()
        .min_connections(cfg.min_connections)
        .max_connections(cfg.max_connections)
        .acquire_timeout(cfg.acquire_timeout)
        .idle_timeout(Some(cfg.idle_timeout))
        .max_lifetime(Some(cfg.max_lifetime))
        .connect_with(connect_opts)
        .await
        .map_err(DbError::from)?;

    tracing::info!(
        application_name = cfg.application_name.as_str(),
        min_connections = cfg.min_connections,
        max_connections = cfg.max_connections,
        statement_timeout_ms,
        "katpool-db pool established"
    );

    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn url() -> String {
        "postgres://kat:kat@localhost:5432/kat".to_owned()
    }

    #[test]
    fn production_defaults_validate() {
        let cfg = PoolConfig::production(url());
        cfg.validate().expect("defaults are valid");
    }

    #[test]
    fn from_lookup_requires_database_url() {
        let result = PoolConfig::from_lookup(|_| None);
        match result {
            Err(DbError::Config { message }) => {
                assert!(message.contains("KATPOOL_DATABASE_URL"), "{message}");
            }
            other => panic!("expected Config error, got {other:?}"),
        }
    }

    #[test]
    fn from_lookup_applies_overrides() {
        let map: HashMap<&str, &str> = [
            (
                "KATPOOL_DATABASE_URL",
                "postgres://kat:kat@localhost:5432/kat",
            ),
            ("KATPOOL_DB_MIN_CONNECTIONS", "2"),
            ("KATPOOL_DB_MAX_CONNECTIONS", "16"),
            ("KATPOOL_DB_ACQUIRE_TIMEOUT_SECS", "10"),
            ("KATPOOL_DB_STATEMENT_TIMEOUT_SECS", "5"),
            ("KATPOOL_DB_APPLICATION_NAME", "katpool-test"),
        ]
        .into_iter()
        .collect();
        let cfg = PoolConfig::from_lookup(|k| map.get(k).map(|s| (*s).to_owned())).expect("valid");
        assert_eq!(cfg.min_connections, 2);
        assert_eq!(cfg.max_connections, 16);
        assert_eq!(cfg.acquire_timeout, Duration::from_secs(10));
        assert_eq!(cfg.statement_timeout, Duration::from_secs(5));
        assert_eq!(cfg.application_name, "katpool-test");
    }

    #[test]
    fn from_lookup_rejects_unparsable_value() {
        let result = PoolConfig::from_lookup(|k| match k {
            "KATPOOL_DATABASE_URL" => Some(url()),
            "KATPOOL_DB_MAX_CONNECTIONS" => Some("not-a-number".to_owned()),
            _ => None,
        });
        match result {
            Err(DbError::Config { message }) => {
                assert!(message.contains("KATPOOL_DB_MAX_CONNECTIONS"), "{message}");
                assert!(message.contains("not-a-number"), "{message}");
            }
            other => panic!("expected Config error, got {other:?}"),
        }
    }

    #[test]
    fn validate_rejects_min_greater_than_max() {
        let cfg = PoolConfig {
            min_connections: 100,
            max_connections: 4,
            ..PoolConfig::production(url())
        };
        let result = cfg.validate();
        assert!(matches!(result, Err(DbError::Config { .. })));
    }

    #[test]
    fn validate_rejects_zero_max_connections() {
        let cfg = PoolConfig {
            max_connections: 0,
            ..PoolConfig::production(url())
        };
        let result = cfg.validate();
        assert!(matches!(result, Err(DbError::Config { .. })));
    }

    #[test]
    fn validate_rejects_zero_statement_timeout() {
        let cfg = PoolConfig {
            statement_timeout: Duration::ZERO,
            ..PoolConfig::production(url())
        };
        let result = cfg.validate();
        assert!(matches!(result, Err(DbError::Config { .. })));
    }
}
