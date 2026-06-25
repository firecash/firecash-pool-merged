---
status: accepted
date: 2026-06-02
deciders: argonmining
---

# ADR-0021: Public read-only HTTP API — embedded axum service, versioned data surface, and DoS posture

## Context and Problem Statement

Phases 1–5 made the money path feature-complete and L1-verified on tn10:
the bridge credits shares, the accountant allocates matured coinbase rewards,
and both payout engines (KAS, KRC-20/NACHO) run live. What the system has *no*
external read surface for is **any of that data**. The `api/` crate is a
doc-only scaffold (`api/src/lib.rs` lists seven endpoints; `api/Cargo.toml`
reserves `axum`/`tower`/`tower-http`/`katpool-db`/`serde`/`insta`).

Phase 6's job is to ship that read surface. Two forces shape it:

1. **It is the only publicly-reachable HTTP attack surface** the pool process
   exposes (Prometheus `:9302` is loopback/firewalled; stratum is a different
   protocol). It therefore inherits the threat-model's Phase 6 DoS controls:
   per-IP rate limiting, bounded request body, hard query timeouts, an
   in-memory cache, `fail2ban` on invalid bursts, and `/balance/:address`
   pseudonymity (`docs/threat-model.md` §4.4/§4.5).
2. **It is the data foundation for a future enterprise-grade dashboard**
   (handoff §5.1). The operator has explicitly endorsed expanding the surface
   beyond the seven scaffold endpoints into a full, versioned **v1** data
   product — per-worker and per-wallet granularity, historical time-series,
   pool aggregates, payout/rebate history — *provided* every addition stays
   under the same discipline (read-only, secret-free, bounded/cached/
   rate-limited, deterministically tested, documented).

Several decisions on this path are architecturally significant (a new public
trust boundary, two new dependency categories, a JSON wire contract a frontend
will depend on, and the reconciliation of an env knob that never worked). This
ADR records them. The runtime wiring also surfaces a latent bug
(`KATPOOL_HEALTH_CHECK_PORT` is carried but never served — the same class as
the #54 `prom_port` defect) that Phase 6 must reconcile rather than inherit.

## Decision Drivers

* **Single-process model is mandatory** (`docs/architecture.md` §2): the pool
  is one binary linking bridge + accountant + payouts + API. The API must
  embed, not become a second deployable.
* **Read-only and secret-free, structurally** — the API must be incapable of
  moving funds or touching the treasury key; it reads PostgreSQL only.
* **Verification posture (ADR-0013)** — deterministic tests, no network in unit
  tests, `insta` snapshots for serialized output, testcontainer Postgres for
  DB-backed paths, time/RNG injected. Reuse the existing `katpool-db` repo
  layer; any new query is a *new* read-only repo function with its own
  testcontainer coverage, never hand-written SQL duplicating an existing fn.
* **DoS resistance at the process edge**, not only at nginx — defence in depth.
* **A wire contract a dashboard can build on for years** — versioned paths,
  stable field names, JS-safe numeric encoding, consistent pagination and
  time-range semantics.
* **Minimal-but-complete** — expand to what a serious dashboard consumes; do
  not speculatively gold-plate.

## Considered Options

For each genuine fork (the handoff instructs stopping on these):

A. **Process model** — (A1) embed behind `KATPOOL_API_PORT` as an env-gated
   task, mirroring the prom exporter; (A2) standalone `api` binary.
B. **Surface scope** — (B1) the seven scaffold endpoints only; (B2) a curated,
   versioned `/api/v1` dashboard-foundation surface.
C. **Rate limiting** — (C1) `tower-governor`; (C2) nginx-only, no in-app limit.
D. **Cache** — (D1) `moka` async TTL cache; (D2) hand-rolled TTL map.
E. **Health/readiness** — (E1) the API owns `/health` `/ready` `/started` and
   `KATPOOL_HEALTH_CHECK_PORT` stays present in the unified runtime but
   documented as a no-op; (E2) retire `KATPOOL_HEALTH_CHECK_PORT` from the
   unified runtime entirely; (E3) also wire the bridge's TCP health server into
   the runtime (two health surfaces).
F. **Numeric encoding** — (F1) sompi/base-unit integers as decimal **strings**;
   (F2) as JSON numbers (i64).

## Decision Outcome

**Chosen: A1 + B2 + C1 + D1 + E1 + F1.**

The API embeds in the `katpool` binary as an env-gated `tokio` task bound to
`KATPOOL_API_PORT` (empty = disabled), exactly like the `KATPOOL_PROM_PORT`
exporter spawn. It exposes three unversioned liveness/readiness probes and a
versioned `/api/v1` read-only data surface composed entirely from `katpool-db`
repo functions (existing ones, plus new read-only ones added under the same
test discipline). Per-IP rate limiting uses `tower-governor`; expensive
aggregates and series are served through a bounded `moka` TTL cache; every
request carries a hard timeout layered over a Postgres `statement_timeout`; the
request body is bounded; and addresses are redacted to their last four
characters in every log/trace. The API owns the health probes;
`KATPOOL_HEALTH_CHECK_PORT` stays present in the unified runtime but is
documented as a no-op (it was never served there, and removing a config key is
a sharper change than the value it adds). Integer money amounts serialize as
decimal strings so a JavaScript dashboard never loses precision above 2^53.

### Surface (v1)

Probes (unversioned — load-balancer / k8s convention):

| Method/Path | Meaning | Codes |
|---|---|---|
| `GET /health` | Liveness: process is up. Static body. | 200 |
| `GET /ready` | Readiness: DB reachable **and** kaspad synced. | 200 / 503 |
| `GET /started` | Startup: initial kaspad sync observed once (latched). | 200 / 503 |

Pool-wide (`/api/v1/pool/...`):

| Path | Source repo fn(s) |
|---|---|
| `GET /pool/stats` | `share_stats::accepted_pool_wide`, `share_reject::count_by_reason_pool_wide`, `block` counts, `payout` aggregates, `treasury::latest` (balances only) |
| `GET /pool/hashrate?window=` | `share_stats::hashrate_estimate_pool_wide` |
| `GET /pool/hashrate/history?from=&to=&bucket=` | **new** `share_stats::hashrate_series_pool_wide` |
| `GET /pool/blocks?limit=&before_id=` | **new** `block::list_recent` (paginated, all statuses) |
| `GET /pool/payouts?limit=&before_id=` | **new** `payout::list_recent_cycles` |

Per-wallet (`/api/v1/...`, address in path → on-chain-equivalent pseudonymity):

| Path | Source repo fn(s) |
|---|---|
| `GET /balance/:address` | **new** `payout::kas_payable_for_wallet` + `nacho_rebate::get` |
| `GET /miners/:address` | profile: `wallet::find_by_address`, `share_stats::accepted_and_rejected_for_wallet`, hashrate, `worker::list_for_wallet`, balances, recent payouts/allocations |
| `GET /miners/:address/workers` | **new** `share_stats::{accepted,hashrate}_for_worker` over `worker::list_for_wallet` |
| `GET /miners/:address/hashrate/history?from=&to=&bucket=` | **new** `share_stats::hashrate_series_for_wallet` |
| `GET /miners/:address/payouts?limit=&before_id=` | `payout::list_for_wallet` (+ cycle join) |
| `GET /miners/:address/rejects?window=` | `share_reject::count_by_reason_for_wallet` |
| `GET /full_rebate/:address` | most-recent `share_allocation.applied_tier` for the wallet (**new** `share_allocation::latest_applied_tier_for_wallet`); `full_rebate = (tier == elite)` |

`/full_rebate/:address` reports the tier that was **actually applied to the
wallet's money** (the persisted `applied_tier`), not a live classifier opinion.
Rationale: the live `KasplexTierClassifier` is an *allocation-time* concern, is
not currently wired into the runtime (`StaticTierClassifier::standard()` is),
and NACHO does not exist on tn10 — so a live lookup would be both
non-deterministic and, on tn10, meaningless. Reporting the persisted
`applied_tier` is honest, deterministic, and testable. Wiring the live
classifier is a separate (allocation-side) decision tracked for mainnet.

The scaffold docstring's bare paths (`/balance/:address`, `/api/pool/stats`,
`/full_rebate/:address`) are **superseded** by their `/api/v1` forms; the stub
docstring is updated on implementation. The threat-model's pseudonymity
argument for `/balance/:address` holds identically under the versioned path.

### Wire contract

- Versioned prefix `/api/v1`; additive changes only within a version; a
  breaking change opens `/api/v2`. Probes are intentionally unversioned.
- **Integer money amounts (sompi, NACHO base units) serialize as decimal
  strings.** 90M KAS ≈ 9×10¹⁵ sompi brushes the 2^53 JS-safe-integer ceiling,
  and NACHO base units exceed it; strings make the dashboard precision-safe.
  Each money field is paired with a human-readable decimal where it aids
  display (e.g. `payable_sompi: "1739516000000"`, `payable_kas: "17395.16"`).
- Hashrate is a JSON number (f64) — it is a rate, lossy-by-definition (see the
  `share_stats` module note); never a money figure.
- Timestamps are RFC3339 UTC strings.
- List endpoints use keyset pagination (`before_id` cursor + `limit`, capped),
  not offset — stable under concurrent inserts and index-friendly on the
  high-volume tables.
- Time-range endpoints take `from`/`to` (RFC3339) + `bucket` (enum:
  `1m`/`5m`/`1h`/`1d`), with a server-enforced max span and bucket count.
- Errors share one shape: `{ "error": { "code": <machine-string>, "message":
  <human> } }`. Codes: `not_found`, `bad_request`, `too_many_requests`,
  `timeout`, `unavailable`, `internal`. HTTP status mirrors the code.

### DoS / security posture

- **Per-IP rate limit** via `tower-governor` (GCRA token bucket), configurable
  burst + per-second refill (`KATPOOL_API_RATE_*`, conservative defaults);
  client IP resolved from the connection, honoring a single trusted
  `X-Forwarded-For` hop (nginx) only. Over-limit ⇒ `429 too_many_requests`.
- **Bounded body** via `tower_http::limit::RequestBodyLimitLayer` (GETs carry
  no body; this is belt-and-suspenders against abuse).
- **Hard timeouts**: `tower_http::timeout::TimeoutLayer` per request, *and* a
  Postgres `statement_timeout` set on the API's connections, so a slow query
  is killed at the DB, not just abandoned at the HTTP layer.
- **In-memory `moka` TTL cache** for pool aggregates, hashrate, and series
  (short TTL, e.g. 5–15s; keyed by route+normalized params; bounded entry
  count). Per-wallet reads get a shorter TTL. The cache both cuts DB load and
  flattens burst amplification.
- **Address redaction**: a `redact(&WalletAddress) -> "…abcd"` helper; the API
  never logs or traces a full address. Enforced by a test that scans emitted
  spans/logs.
- **Read-only by construction**: the API layer calls only read repo functions;
  it never imports the payout/secret crates. On mainnet the API connections use
  a least-privilege DB role (threat-model §4.6) — provisioning that role is a
  Phase 7/8 deploy concern, noted there; the embed shares the existing pool on
  tn10.
- **CORS** via `tower_http::cors` — configurable allowed origin
  (`KATPOOL_API_CORS_ALLOW_ORIGIN`), defaulting to disabled/none until the
  dashboard origin exists.
- **`fail2ban` on invalid bursts** is an operational control (jail on
  4xx-burst log lines); documented in the acceptance doc, not code.

### Readiness signal

`/ready` and `/started` read a shared `ReadinessState` handle (an `Arc` of
atomics / `watch`) injected into the API state. The runtime updates it from
work it already does — DB reachability from a periodic `SELECT 1`, kaspad-sync
from the maturity tracker's existing kaspad polling — so the API opens **no
second gRPC connection**. `/started` is a one-way latch set the first time sync
is observed. In tests the state is a mock, so `/ready`/`/started` are fully
deterministic (no network), satisfying the handoff's "mock/blackbox for
`/ready` kaspad-sync".

### Health-port reconciliation

`KATPOOL_HEALTH_CHECK_PORT` is carried into `BridgeServerConfig.health_check_port`
by the runtime, but the library entrypoint `listen_and_serve_with_events`
(unlike the standalone `bridge/src/main.rs`) never starts a health server — so
in the unified runtime the knob is dead, exactly the #54 `prom_port` class of
bug. Phase 6 makes the **API** own `/health`/`/ready`/`/started`.
`KATPOOL_HEALTH_CHECK_PORT` **stays present** in the unified runtime but is
**documented as a no-op** there (the bridge struct field and the standalone
bridge binary keep using it): the operator's call is that retiring a config key
is a sharper, more surprising change than the clarity it buys, and a documented
no-op avoids both over-engineering and a second redundant health surface. The
runtime keeps passing the value into `BridgeServerConfig.health_check_port`
(harmless — the library ignores it); the module docs and `ops/env` templates
state plainly that liveness/readiness now come from the API on
`KATPOOL_API_PORT`.

### Consequences

- Positive: the pool gains a production-grade, versioned, dashboard-ready data
  surface with a single embedding, no new deployable, and no fund/secret reach.
- Positive: the public HTTP edge is rate-limited, bounded, timed-out, and
  cached at the process — defence in depth above nginx.
- Positive: readiness is real and reuses existing kaspad polling (no extra
  connection); the latent dead `KATPOOL_HEALTH_CHECK_PORT` is now clearly
  documented as a no-op in the runtime rather than silently misleading.
- Positive: string-encoded money is precision-safe for a JS dashboard from day
  one — no painful wire-breaking migration later.
- Negative: two new dependency categories (`tower-governor`, `moka`). Mitigation:
  both are widely-used, MIT/Apache-licensed (clean under `cargo deny`), and pull
  modest trees; pinned via the workspace and reviewed in the PR.
- Negative: new read-only repo functions enlarge `katpool-db`. Mitigation: each
  is small, read-only, and testcontainer-covered; none duplicate an existing fn.
- Negative: a public surface invites scraping. Mitigation: rate limit + cache +
  `fail2ban`; all data is already on-chain-equivalent in sensitivity.

### Confirmation

- `docs/phase-6-acceptance.md` GREEN matrix backed by live tn10 evidence: every
  endpoint exercised against `katpool_tn10`, with the Goldshell soak's real
  rows (the single live wallet/worker) and `/metrics`-cross-checked hashrate.
- `insta` JSON snapshots pin every response shape (success **and** every error
  code); testcontainer Postgres covers all DB-backed endpoints with seeded
  fixtures and an injected fixed clock; a mock `ReadinessState` covers
  `/ready`/`/started`; explicit tests cover unknown address (`404`), malformed
  address (`400`), over-limit (`429`), and statement-timeout (`503 timeout`).
- A log/trace test asserts no full address ever appears in emitted output.
- Quality gates green: `cargo fmt`, `clippy -D warnings`, `cargo doc -D
  warnings`, `cargo test --workspace`, `cargo deny check`, `typos`,
  `cargo machete` (the `api` machete-ignore block is replaced by real use).

## Pros and Cons of the Options

### A — Process model
- **A1 embed (chosen)**: Good: matches architecture.md's single-binary mandate;
  shares the DB pool and the runtime's kaspad polling; one deploy artifact.
  Bad: API load shares the process with the money path (mitigated by rate
  limit + cache + timeouts + bounded pool).
- A2 standalone: Good: blast-radius isolation. Bad: violates the documented
  architecture; duplicates config/DB wiring; a second unit to operate.

### B — Surface scope
- **B2 versioned `/api/v1` (chosen)**: Good: the dashboard foundation the
  operator asked for; stable, paginated, time-ranged. Bad: more endpoints and
  repo functions now. Mitigated: all read-only, all tested, none speculative.
- B1 seven endpoints only: Good: smallest. Bad: forces a near-term second pass
  and a wire-contract churn the moment the dashboard work starts.

### C — Rate limiting
- **C1 `tower-governor` (chosen)**: Good: the threat-model's named control;
  per-IP GCRA in-process. Bad: new dep.
- C2 nginx-only: Good: no dep. Bad: no defence if nginx is bypassed or
  misconfigured; not defence-in-depth.

### D — Cache
- **D1 `moka` (chosen)**: Good: bounded, concurrent, async TTL, battle-tested.
  Bad: new dep.
- D2 hand-rolled: Good: no dep. Bad: re-implements eviction/TTL/bounding — more
  code to test for a worse result.

### E — Health/readiness
- **E1 API owns probes; keep `KATPOOL_HEALTH_CHECK_PORT` as a documented no-op
  (chosen)**: Good: one real health surface; no behavioral/config change to an
  existing deployment; the dead knob is clearly documented rather than silently
  misleading. Bad: a no-op key lingers in the config surface (acceptable —
  cheaper than a removal that surprises an existing `ops/env`).
- E2 retire the runtime knob: Good: removes the dead key. Bad: a sharper,
  more surprising change than the clarity it buys (operator declined).
- E3 wire the bridge health server too: Bad: two competing health surfaces;
  keeps a redundant code path alive.

### F — Numeric encoding
- **F1 strings (chosen)**: Good: precision-safe in JS for amounts above 2^53.
  Bad: slightly less ergonomic for non-JS clients (mitigated by paired
  human-decimal fields).
- F2 JSON numbers: Bad: silent precision loss in the exact consumer (the
  browser dashboard) this API exists to feed.

## More Information

- Implements: handoff §5 + §5.1; `docs/architecture.md` §2/§3 (`api` = "axum
  read-only", embedded); `docs/threat-model.md` §4.4/§4.5 Phase 6 rows.
- Reuses: `crates/katpool-db` repo layer (`wallet`, `worker`, `share_stats`,
  `share_reject`, `share_allocation`, `payout`, `nacho_rebate`, `block`,
  `treasury`, `pool_meta`); `katpool_domain::WalletAddress` validation.
- Related: ADR-0013 (verification posture), ADR-0012/0016 (tiers & rebate
  semantics behind `/full_rebate`), and the #54 wiring lesson (env-gated task
  spawns must actually be spawned).
- Follow-ups (not Phase 6): least-privilege read-only DB role provisioning
  (Phase 7/8 deploy); wiring the live `KasplexTierClassifier` at allocation
  time (mainnet); a websocket/SSE live-push channel for the dashboard (separate
  ADR if pursued).
- Open question for the operator: default per-IP rate (burst/refill) and cache
  TTLs — proposed conservative defaults in the PR; tune against measured load.
