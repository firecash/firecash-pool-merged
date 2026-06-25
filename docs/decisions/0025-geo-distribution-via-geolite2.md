---
status: accepted
date: 2026-06-03
deciders: argonmining
consulted: argonmining
informed: argonmining
---

# ADR-0025: Country-level miner geo distribution via MaxMind GeoLite2

## Context and Problem Statement

The dashboard wants a pool-wide **geo distribution** view — "where are our
miners?" — as the last of the two visualizations deferred in ADR-0024
(the other, the pool reject stream, shipped in #64).

We already persist the **real client IP** of every stratum connection:
`connection_session.remote_ip` (Postgres `INET`), populated by the
accountant when it handles `PoolEvent::SessionClosed`. The IP is the
genuine miner endpoint — PROXY-protocol v2 resolution at the fly.io edge
restores it before it reaches the bridge (ADR-0022). What we do **not**
have is any way to turn an IP into a country: there is no GeoIP database,
no resolver, and no `country` column anywhere in the stack.

The API is public, read-only, and privacy-conscious (addresses are
redacted in telemetry; on-chain amounts are exact-integer). Any geo
feature must respect that posture and the legal constraints of whatever
IP→country source we adopt.

## Decision Drivers

* **Accuracy without guessing** — country mapping must come from a
  maintained dataset, not heuristics.
* **Privacy + legality** — GeoLite2's EULA forbids using the data to
  identify an individual/household; we must store and expose only
  aggregate, country-level data, and carry the required attribution.
* **No hot-path cost** — share crediting must stay untouched; resolution
  happens off the hot path, once per session.
* **Graceful absence** — the database is a license-gated artifact, not in
  git; the runtime must run normally when it is missing (geo simply goes
  empty), so dev and CI need no license key.
* **Minimal surface** — reuse the existing session-write path and the
  established `/pool/firmware`-style aggregate endpoint + repo shape.

## Considered Options

1. **MaxMind GeoLite2-Country, resolved in the accountant at session
   persist, stored as a `country` column, exposed aggregate-only.**
2. **A third-party GeoIP web service** (per-IP HTTP lookup at ingest).
3. **Resolve at query time in the API** (look up every `remote_ip` in the
   window on each request).
4. **Do not build geo.**

## Decision Outcome

**Chosen option: 1 (GeoLite2-Country, resolve-at-persist, aggregate-only).**
The accountant is already the single writer of `connection_session` rows
and already parses `remote_ip` into an `IpAddr` in `handle_session_closed`;
adding a country lookup there is one call on a cold path. We store the
ISO-3166-1 alpha-2 code in a new nullable `country` column and expose only
**aggregate counts per country** through a new `GET /api/v1/pool/geo`
endpoint — never the IP, never a per-miner geo field. This keeps us inside
the GeoLite2 EULA (no individual identification) and the project's
privacy posture, while reusing the exact `firmware_breakdown` pattern.

GeoLite2-Country (not City) is sufficient and is the smaller artifact. The
`maxminddb` crate (v0.28.x, ISC) exposes a `Send + Sync` `Reader` loaded
once at startup, so lookups are lock-free reads shared across the
accountant.

### Mechanics

- **Dependency:** `maxminddb` (workspace dep), used only by the runtime
  layer that owns the resolver. `katpool-domain` and `katpool-db` stay
  GeoIP-free (the country arrives as a plain `Option<&str>`).
- **Resolver:** a small `geoip` module wrapping
  `maxminddb::Reader<Vec<u8>>` (or `mmap`). Loaded at startup from
  `KATPOOL_GEOIP_DB` (e.g. `/etc/katpool/GeoLite2-Country.mmdb`). If the
  env var is unset or the file is missing, the resolver is `None` and
  every lookup returns `None` — geo degrades to empty, nothing errors.
- **Ingest:** the accountant holds `Option<Arc<GeoIp>>`. In
  `handle_session_closed`, after `remote_ip.parse::<IpAddr>()`, it
  resolves the alpha-2 code via `decode_path(["country","iso_code"])` and
  passes `country: Option<&str>` to `connection_session::record_closed`.
- **Schema:** migration adds `country CHAR(2)` (nullable) to
  `connection_session`, plus an index supporting the windowed aggregate
  (`(country, disconnected_at)` or reuse the existing window predicate).
  Existing rows stay `NULL` (back-fill is out of scope; the window view
  fills in as new sessions close).
- **Repo:** new `country_breakdown(since) -> Vec<{ country, workers,
  sessions }>`, mirroring `firmware_breakdown` (distinct non-null
  `worker_id`, session count, `country IS NOT NULL`, ordered desc).
- **API:** `GET /api/v1/pool/geo?window=<secs>` →
  `{ window_secs, entries: [{ country, workers, sessions }] }`, pool-cached
  like the other pool aggregates. Aggregate-only; no IP, no per-wallet geo.
- **Dashboard:** a ranked country-distribution panel (flag + name +
  share bar/count) on the Overview/Status surface, with the **required
  MaxMind attribution** added to the "About this data" section
  ("This product includes GeoLite Data created by MaxMind, available from
  https://www.maxmind.com.").
- **Ops:** the `.mmdb` is provisioned out-of-band to `/etc/katpool/`
  (already `ReadOnlyPaths` in the systemd unit). A `geoipupdate` (or
  scripted download with the account license key) cron keeps it current
  and **destroys versions >30 days old** per the EULA. The license key is
  host-local, never committed.

### Consequences

- Positive: deterministic, maintained country data; reuses the proven
  session-write + aggregate-endpoint patterns; zero hot-path impact.
- Positive: privacy- and EULA-safe by construction (country granularity,
  aggregate-only exposure, attribution carried).
- Positive: fully optional — dev/CI/hosts without the DB just see empty
  geo; no key needed to build or run.
- Negative: a new license-gated operational artifact + update cron.
  Mitigation: env-gated optional load, documented in the deploy runbook,
  cron mirrors the established ops pattern.
- Negative: only newly-closed sessions get a country (no back-fill).
  Mitigation: the windowed view converges within one window; back-fill can
  be a later one-off if desired.
- Negative: VPN/proxy IPs geolocate to the exit country, not the miner.
  Accepted: this is inherent to IP geo and acceptable for an aggregate
  "where in the world" view.

### Confirmation

- Unit test: `geoip` resolver returns the right alpha-2 for known test
  IPs using MaxMind's bundled test `.mmdb`, and returns `None` cleanly
  when unconfigured.
- DB test: `country_breakdown` aggregates seeded sessions correctly and
  excludes `NULL`-country rows.
- Wire test: `pool_geo_wire` snapshot locks the JSON shape; endpoint test
  returns 200 with an empty `entries` array when no geo data is seeded.
- Live: after deploy with the DB provisioned, `GET /pool/geo` returns
  non-empty entries and the dashboard panel renders with attribution.

## Pros and Cons of the Options

### Option 1: GeoLite2-Country, resolve-at-persist, aggregate-only

- Good: one cold-path lookup at the single existing session writer; lock-free
  shared `Reader`; smallest viable dataset; privacy/EULA-aligned.
- Good: no schema churn beyond one nullable column; endpoint mirrors firmware.
- Bad: license-gated artifact + 30-day update obligation.

### Option 2: GeoIP web service at ingest

- Good: no local DB to maintain.
- Bad: a network call on every session close (cold path, but adds a
  dependency + failure mode + rate limits + per-lookup cost); sends miner
  IPs to a third party continuously — worse privacy posture.

### Option 3: Resolve at query time in the API

- Good: no schema change.
- Bad: re-looks-up potentially thousands of IPs per request; couples the
  read-only API to a GeoIP reader and to raw IPs; cache-hostile; worse
  privacy (IPs handled on the public read path).

### Option 4: Don't build geo

- Good: zero new dependency, zero license/ops burden.
- Bad: leaves the ADR-0024 geo visualization permanently unshipped.

## Implementation status

Shipped env-gated and verified end-to-end (resolver checked against
MaxMind's official test `.mmdb`; migration + aggregate query exercised
against Postgres; wire snapshot locked):

- `accountant::geoip::GeoIp` (optional `maxminddb` reader), wired into
  `EventConsumer` via `with_geoip`; constructed in `katpool` main from
  `KATPOOL_GEOIP_DB` (missing/unreadable ⇒ logged, geo disabled).
- Migration `20260603000000_connection_session_country.sql` adds the
  nullable `country` column + partial index; `record_closed` persists it;
  `connection_session::country_breakdown` aggregates it.
- `GET /api/v1/pool/geo` (`GeoBreakdown`), cached like other pool
  aggregates; dashboard `GeoPanel` (ranked country bars) + required
  MaxMind attribution in the Status "About this data" panel.

**Operational dependency (not code):** live country data needs a
GeoLite2-Country `.mmdb` provisioned to `/etc/katpool/` on the host plus a
`geoipupdate`/download cron with a MaxMind license key, and
`KATPOOL_GEOIP_DB` set in `ops/env/<network>.env`. Until then the column
stays NULL and the panel shows its empty state.

## More Information

- ADR-0021 (public read-only API) — endpoint conventions, redaction posture.
- ADR-0022 (multiport stratum + fly.io edge) — PROXY-protocol IP restoration
  that makes `remote_ip` the real client IP.
- ADR-0024 (dashboard overhaul) — geo deferred here for lack of a data source.
- MaxMind GeoLite2: https://dev.maxmind.com/geoip/geolite2-free-geolocation-data/
- GeoLite EULA (attribution, 30-day destruction, no individual ID):
  https://www.maxmind.com/en/geolite/eula
- `maxminddb` crate: https://docs.rs/maxminddb
- Open question: surface placement (Overview vs Status) and whether to add
  a choropleth later vs the initial ranked-list panel.
