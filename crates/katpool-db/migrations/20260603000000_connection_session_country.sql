-- ADR-0025: country-level geo distribution.
--
-- Adds an optional ISO-3166-1 alpha-2 country code to each stratum
-- session, resolved by the accountant from the existing remote_ip via a
-- MaxMind GeoLite2-Country database at session-persist time. The column is
-- nullable: it stays NULL when the GeoIP resolver is unconfigured, the IP
-- is private/unknown, or for rows written before this migration.
--
-- Exposure is aggregate-only (GET /api/v1/pool/geo) — the raw IP is never
-- surfaced and no per-miner geo field exists. See ADR-0025 for the
-- GeoLite2 EULA constraints (country granularity, aggregate-only,
-- attribution).

ALTER TABLE connection_session
    ADD COLUMN country CHAR(2);

-- Supports the windowed GROUP BY country aggregate; partial because only
-- non-null countries are ever aggregated.
CREATE INDEX idx_connection_session_country
    ON connection_session (country)
    WHERE country IS NOT NULL;
