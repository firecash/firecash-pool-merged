//! Optional IP→country resolution for stratum sessions (ADR-0025).
//!
//! Wraps a `MaxMind` `GeoLite2` (or `GeoIP2`) **Country** database via the
//! `maxminddb` reader. The reader is loaded once at startup and is
//! `Send + Sync`, so lookups are lock-free reads shared across the
//! accountant. Resolution happens off the hot path — once per session,
//! when the accountant persists a `connection_session` row.
//!
//! The resolver is **entirely optional**: when no database path is
//! configured (or the file is absent), the runtime constructs no
//! resolver and every session records a `NULL` country. Dev and CI
//! therefore need no `MaxMind` license key.
//!
//! Privacy: only the ISO-3166-1 alpha-2 **country** code is ever read;
//! the IP is never persisted beyond the existing `remote_ip` column,
//! and the public API exposes country data only in aggregate
//! (`GET /api/v1/pool/geo`). See ADR-0025 for the EULA constraints
//! (country granularity, aggregate-only exposure, attribution).

use std::net::IpAddr;
use std::path::Path;

use maxminddb::{PathElement, Reader};

/// A loaded `GeoLite2`/`GeoIP2` country database reader.
pub struct GeoIp {
    reader: Reader<Vec<u8>>,
}

/// Failure to load a `GeoIP` database from disk.
#[derive(Debug, thiserror::Error)]
#[error("failed to open GeoIP database at `{path}`: {source}")]
pub struct GeoIpError {
    /// The configured database path that failed to load.
    pub path: String,
    /// Underlying `maxminddb` error.
    #[source]
    pub source: maxminddb::MaxMindDbError,
}

impl GeoIp {
    /// Load a `.mmdb` database into memory.
    ///
    /// # Errors
    /// Returns [`GeoIpError`] if the file cannot be opened or parsed as
    /// a `MaxMind` DB.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, GeoIpError> {
        let path_ref = path.as_ref();
        let reader = Reader::open_readfile(path_ref).map_err(|source| GeoIpError {
            path: path_ref.display().to_string(),
            source,
        })?;
        Ok(Self { reader })
    }

    /// Resolve the ISO-3166-1 alpha-2 country code for `ip`, if known.
    ///
    /// Returns `None` for private/reserved IPs, addresses absent from the
    /// database, or databases that don't carry a country field — never
    /// errors, so the cold session-write path stays infallible here.
    #[must_use]
    pub fn country(&self, ip: IpAddr) -> Option<String> {
        let result = self.reader.lookup(ip).ok()?;
        result
            .decode_path::<String>(&[PathElement::Key("country"), PathElement::Key("iso_code")])
            .ok()
            .flatten()
    }
}

impl std::fmt::Debug for GeoIp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GeoIp").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_missing_file_errors() {
        let err = GeoIp::open("/nonexistent/path/to/GeoLite2-Country.mmdb").unwrap_err();
        assert!(err.path.contains("GeoLite2-Country.mmdb"));
    }

    // Optional real-DB check: point KATPOOL_TEST_GEOIP_DB at any
    // GeoLite2/GeoIP2 Country (or City) .mmdb to verify a known IP resolves
    // to a 2-letter code and a private IP resolves to None. The probe IP is
    // configurable (KATPOOL_TEST_GEOIP_IP, default 8.8.8.8) so the test
    // works against both MaxMind's documentation test DBs and real GeoLite2
    // data. Skipped when the env var is unset so CI needs no licensed
    // artifact.
    #[test]
    fn resolves_known_ip_when_db_provided() {
        let Ok(path) = std::env::var("KATPOOL_TEST_GEOIP_DB") else {
            return;
        };
        let geo = GeoIp::open(&path).expect("open test mmdb");
        let probe = std::env::var("KATPOOL_TEST_GEOIP_IP").unwrap_or_else(|_| "8.8.8.8".to_owned());
        let public: IpAddr = probe.parse().expect("probe IP");
        let code = geo.country(public);
        assert!(
            code.as_deref().is_some_and(|c| c.len() == 2),
            "expected a 2-letter country for {probe}, got {code:?}"
        );
        let private: IpAddr = "10.0.0.1".parse().unwrap();
        assert_eq!(geo.country(private), None);
    }
}
