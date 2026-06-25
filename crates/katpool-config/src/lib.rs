//! Strongly-typed configuration **file** loader for katpool.
//!
//! This crate parses an optional TOML or YAML config file into a typed,
//! validated [`FileConfig`]. It is the *file* layer of the runtime's
//! configuration; the `katpool` binary keeps reading environment variables
//! exactly as before and applies a strict precedence:
//!
//! ```text
//! environment variable  >  config file  >  built-in default
//! ```
//!
//! i.e. an env var always wins over the file, and the file only supplies a
//! value where the env var is unset. When no config-file path is provided
//! (the `KATPOOL_CONFIG` env var is empty/unset) the runtime behaves exactly
//! as the pure-environment configuration it has always used.
//!
//! Loading is fail-fast and never falls back to silent defaults: a missing
//! file, a malformed document, an unknown key (`deny_unknown_fields`), or a
//! value that fails validation aborts the boot with an actionable error.
//!
//! Only the *core* runtime surface is modelled here (node/db/network,
//! stratum, maturity, and the operational toggles). Payout, KRC-20,
//! consolidation, and treasury-key settings remain environment-only by
//! design — they carry secrets or money-movement policy and are kept out of
//! a checked-in file.

#![cfg_attr(not(test), warn(missing_docs))]
// Tests construct values and assert with unwrap/expect; keep that ergonomic
// without relaxing the lint for the library code itself.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

use std::path::Path;

use serde::Deserialize;
use validator::Validate;

/// Crate version constant.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Errors raised while loading or validating a config file.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// The file could not be read, parsed, or deserialized into
    /// [`FileConfig`] (includes unknown-key errors from
    /// `deny_unknown_fields`).
    #[error("reading config file: {0}")]
    Source(#[from] config::ConfigError),
    /// The file parsed but failed schema validation (e.g. an out-of-range
    /// value).
    #[error("invalid config file: {0}")]
    Invalid(#[from] validator::ValidationErrors),
}

/// The file-supplied configuration layer.
///
/// Every field is optional: an absent key means "no file opinion — fall back
/// to the environment variable, then the built-in default". The field names
/// are the YAML/TOML keys; they mirror the corresponding `KATPOOL_*`
/// environment variables so an operator can move a value between the two
/// without surprises.
///
/// `deny_unknown_fields` makes a typo'd or stale key a hard boot error rather
/// than a silently ignored setting.
#[derive(Debug, Default, Clone, Deserialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct FileConfig {
    /// kaspad gRPC URL (`KASPAD_GRPC_URL`).
    pub kaspad_url: Option<String>,
    /// `PostgreSQL` connection URL (`KATPOOL_DATABASE_URL`).
    pub database_url: Option<String>,
    /// Comma-separated pool payout address(es) (`KATPOOL_POOL_ADDRESS`).
    pub pool_address: Option<String>,
    /// Network identifier override (`KATPOOL_NETWORK`); normally derived from
    /// the pool address prefix.
    pub network: Option<String>,
    /// Stable instance identifier for logs/metrics (`KATPOOL_INSTANCE_ID`).
    pub instance_id: Option<String>,

    /// Single-port stratum bind (`KATPOOL_STRATUM_PORT`).
    pub stratum_port: Option<String>,
    /// Multi-port stratum seeds `port:seed,...` (`KATPOOL_STRATUM_PORTS`).
    pub stratum_ports: Option<String>,
    /// Per-miner difficulty floor (`KATPOOL_MIN_SHARE_DIFF`). ASIC-class
    /// default is 4096; must be at least 1.
    #[validate(range(min = 1))]
    pub min_share_diff: Option<u32>,
    /// Enable the vardiff retarget loop (`KATPOOL_VAR_DIFF`).
    pub var_diff: Option<bool>,
    /// Target accepted shares-per-minute for vardiff (`KATPOOL_SHARES_PER_MIN`).
    #[validate(range(min = 1))]
    pub shares_per_min: Option<u32>,
    /// Require a PROXY-protocol v2 header on each connection
    /// (`KATPOOL_STRATUM_PROXY_PROTOCOL`).
    pub proxy_protocol: Option<bool>,
    /// Top-line pool fee in basis points (`KATPOOL_FEE_TOPLINE_BPS`); 0..=10000.
    #[validate(range(max = 10_000))]
    pub fee_topline_bps: Option<u16>,
    /// Event broadcast channel capacity (`KATPOOL_BROADCAST_CAPACITY`).
    #[validate(range(min = 1))]
    pub broadcast_capacity: Option<usize>,

    /// Prometheus exporter bind (`KATPOOL_PROM_PORT`); empty disables.
    pub prom_port: Option<String>,
    /// Public read-only API bind (`KATPOOL_API_PORT`); empty disables.
    pub api_port: Option<String>,
    /// Dedicated liveness/readiness probe bind (`KATPOOL_HEALTH_CHECK_PORT`).
    pub health_check_port: Option<String>,

    /// Maturity poller interval, seconds (`KATPOOL_MATURITY_POLL_SECS`).
    #[validate(range(min = 1))]
    pub maturity_poll_secs: Option<u64>,
    /// Coinbase maturity depth in DAA score (`KATPOOL_COINBASE_MATURITY`).
    pub coinbase_maturity: Option<u64>,
    /// PROP window span in DAA score (`KATPOOL_WINDOW_DAA_SPAN`).
    pub window_daa_span: Option<u64>,
    /// Maturity sweep batch size (`KATPOOL_MATURITY_BATCH_SIZE`).
    pub maturity_batch_size: Option<i64>,

    /// Wallet tier classifier: `static` or `kasplex` (`KATPOOL_TIER_CLASSIFIER`).
    pub tier_classifier: Option<String>,
    /// Graceful-shutdown drain budget, seconds (`KATPOOL_SHUTDOWN_DRAIN_SECS`).
    pub shutdown_drain_secs: Option<u64>,
}

impl FileConfig {
    /// Load and validate a config file from `path`.
    ///
    /// The format is inferred from the extension (`.yaml`/`.yml`/`.toml`).
    ///
    /// # Errors
    /// Returns [`ConfigError::Source`] if the file is missing, unreadable,
    /// malformed, or contains an unknown key, and [`ConfigError::Invalid`] if
    /// a value fails range validation.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let settings = config::Config::builder()
            .add_source(config::File::from(path).required(true))
            .build()?;
        let parsed: Self = settings.try_deserialize()?;
        parsed.validate()?;
        Ok(parsed)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::{ConfigError, FileConfig};

    fn write(name: &str, body: &str) -> tempfile::TempPath {
        let mut f = tempfile::Builder::new()
            .suffix(name)
            .tempfile()
            .expect("tempfile");
        f.write_all(body.as_bytes()).expect("write");
        f.into_temp_path()
    }

    #[test]
    fn loads_yaml_subset() {
        let path = write(
            ".yaml",
            "network: testnet-10\nstratum_port: \"5701\"\nmin_share_diff: 4096\nvar_diff: true\n",
        );
        let cfg = FileConfig::load(&path).expect("load");
        assert_eq!(cfg.network.as_deref(), Some("testnet-10"));
        assert_eq!(cfg.stratum_port.as_deref(), Some("5701"));
        assert_eq!(cfg.min_share_diff, Some(4096));
        assert_eq!(cfg.var_diff, Some(true));
        // Unspecified keys stay None so the runtime falls back to env/default.
        assert!(cfg.kaspad_url.is_none());
        assert!(cfg.shares_per_min.is_none());
    }

    #[test]
    fn loads_toml_subset() {
        let path = write(
            ".toml",
            "instance_id = \"katpool-tn10\"\nshares_per_min = 20\n",
        );
        let cfg = FileConfig::load(&path).expect("load");
        assert_eq!(cfg.instance_id.as_deref(), Some("katpool-tn10"));
        assert_eq!(cfg.shares_per_min, Some(20));
    }

    #[test]
    fn unknown_key_is_a_hard_error() {
        let path = write(".yaml", "stratum_prt: \"5701\"\n");
        let err = FileConfig::load(&path).expect_err("typo must fail");
        assert!(matches!(err, ConfigError::Source(_)), "got {err:?}");
    }

    #[test]
    fn out_of_range_value_fails_validation() {
        let path = write(".yaml", "min_share_diff: 0\n");
        let err = FileConfig::load(&path).expect_err("zero must fail");
        assert!(matches!(err, ConfigError::Invalid(_)), "got {err:?}");
    }

    #[test]
    fn fee_above_one_hundred_percent_fails() {
        let path = write(".yaml", "fee_topline_bps: 10001\n");
        let err = FileConfig::load(&path).expect_err("over 100% must fail");
        assert!(matches!(err, ConfigError::Invalid(_)), "got {err:?}");
    }

    #[test]
    fn comments_only_file_yields_all_defaults() {
        // An operator copying the example verbatim (every key commented out)
        // must load as an empty file layer, not a parse error.
        let path = write(".yaml", "# only comments\n# nothing active\n");
        let cfg = FileConfig::load(&path).expect("comments-only must load");
        assert!(cfg.kaspad_url.is_none());
        assert!(cfg.stratum_port.is_none());
        assert!(cfg.min_share_diff.is_none());
    }

    #[test]
    fn missing_file_is_an_error() {
        let err = FileConfig::load(std::path::Path::new("/nonexistent/katpool.yaml"))
            .expect_err("missing file must fail");
        assert!(matches!(err, ConfigError::Source(_)), "got {err:?}");
    }
}
