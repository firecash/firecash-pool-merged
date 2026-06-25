# Phase 6 acceptance evidence

Phase 6 (public read-only HTTP API) closes when every row below is GREEN for a
release-candidate commit. It ships the external read surface decided in
[ADR-0021](decisions/0021-public-read-only-http-api.md): an env-gated `axum`
service embedded in the `katpool` runtime behind `KATPOOL_API_PORT`, composed
entirely from `katpool-db` read-only repo functions, under the threat-model's
Phase 6 DoS controls (`docs/threat-model.md` §4.4/§4.5).

Prerequisites: Phases 1–5 complete and L1-verified on tn10 (the bridge credits
shares, the accountant allocates matured coinbase, both payout engines run).
The API adds **no** new money path and **no** second kaspad connection.

## Acceptance matrix

| # | Criterion | Verification | Status |
|---|---|---|---|
| 1 | **Embedded, env-gated process model** (ADR-0021 A1): the API runs as a `tokio` task inside `katpool` bound to `KATPOOL_API_PORT` (empty = disabled), mirroring the prom exporter; no second deployable. | `katpool` runtime wiring spawns `api::serve_on` only when `KATPOOL_API_PORT` parses to a bind address; disabled otherwise. `cargo check`/`clippy` green. | GREEN — M6.5 |
| 2 | **Versioned `/api/v1` surface** (B2): pool aggregates + per-wallet views, each backed by a `katpool-db` repo fn (existing or new read-only). | New repo fns (`block::list_recent`/`count_by_status`, `payout::{kas_payable_for_wallet,pool_payout_totals,list_recent_cycles,list_for_wallet_detailed}`, `share_stats::{accepted/hashrate}_for_worker`, `active_participant_counts`, `hashrate_series_*`, `share_allocation::latest_applied_tier_for_wallet`) each covered by `crates/katpool-db/tests/repo_api_reads.rs` on testcontainer Postgres. | GREEN — M6.1 |
| 3 | **Stable wire contract**: money as decimal strings, hashrate as JSON number, RFC3339 timestamps, keyset pagination, fixed `bucket` enum, single error shape. | `insta` JSON snapshots pin every response model (`api/tests/wire_contract.rs`) including each error code. | GREEN — M6.3 |
| 4 | **Per-IP rate limiting** (C1, `tower-governor` GCRA): over-limit ⇒ `429 too_many_requests`; client IP from the connection (one trusted `X-Forwarded-For` hop via nginx). | Real-listener burst test trips the limiter (`api/tests/rate_limit.rs`). | GREEN — M6.4 |
| 5 | **Bounded body + hard timeout + `moka` TTL cache** (D1): request body capped; per-request timeout layered over the DB `statement_timeout` (timeout ⇒ `503`); pool-wide vs. per-wallet TTLs; bounded entry count. | `RequestBodyLimitLayer` + `TimeoutLayer::with_status_code(503)` in `api::app`; `moka` caches sized from `ApiConfig`; `DbError` query-cancel → `ApiError::Timeout` (503) unit-tested. | GREEN — M6.2 |
| 6 | **Health/readiness** (E1): API owns `/health` `/ready` `/started`; `/ready` = DB-reachable AND kaspad-synced; `/started` latches first sync; kaspad-sync reuses the maturity tracker's existing poll (no second gRPC). `KATPOOL_HEALTH_CHECK_PORT` documented as a runtime no-op. | `MaturityTracker::with_sync_observer` → `watch` → `ReadinessHandle`; endpoint tests toggle a mock `ReadinessHandle` and assert 200/503 (`api/tests/endpoints.rs`). | GREEN — M6.5 |
| 7 | **Read-only + secret-free by construction**: the `api` crate links neither the payout nor secret crates and calls only read repo fns. | `api/Cargo.toml` deps reviewed; no `payout-*`/`katpool-secrets` import. | GREEN |
| 8 | **Address redaction**: no full wallet address ever appears in emitted logs/traces. | `api::redact::address` + a test scanning emitted output. | GREEN — M6.2 |
| 9 | **Error/status paths**: unknown address `404`, malformed address `400`, over-limit `429`, statement-timeout `503`. | Endpoint + unit tests for each. | GREEN — M6.3/M6.4 |
| 10 | **Quality gates green**: `cargo fmt`, `clippy -D warnings`, `cargo doc -D warnings`, `cargo test --workspace`, `cargo deny check`, `typos`, `cargo machete` (the `api` machete-ignore entry removed). | CI + local run. | GREEN |
| 11 | **Live tn10 evidence**: every endpoint exercised against `katpool_tn10` with the Goldshell soak's real rows, hashrate cross-checked against `/metrics`. | `KATPOOL_API_PORT=127.0.0.1:18080` enabled on `tn10`; all probes + `/api/v1` endpoints + 400/404/429 captured, hashrate cross-checked. Archived under `api-evidence/2026-06-02T21-09Z-tn10-api-acceptance.md`. | GREEN — 2026-06-02 |

## Milestone map (PR-sized)

| Milestone | Delivers | Closes rows |
|---|---|---|
| **M6.1** | New read-only `katpool-db` repo fns + testcontainer coverage | 2 |
| **M6.2** | `api` crate primitives: money/redact/error/config/state/params | 5, 8 |
| **M6.3** | Models + handlers + `insta` wire-contract snapshots | 3, 9 |
| **M6.4** | Rate-limit + cache + timeout layers; 429 test | 4, 9 |
| **M6.5** | Runtime wiring behind `KATPOOL_API_PORT` + readiness bridge | 1, 6 |

## Out of scope for Phase 6

- **Least-privilege read-only DB role** provisioning (Phase 7/8 deploy concern;
  the tn10 embed shares the existing pool).
- **Live `KasplexTierClassifier`** at allocation time (mainnet; `/full_rebate`
  reports the persisted `applied_tier`, per ADR-0021).
- **Websocket/SSE live-push** for the dashboard (separate ADR if pursued).
- **`fail2ban` jail** on 4xx bursts — an operational control documented here,
  not code.

## Sign-off

Phase 6 closes when:

1. Every test-backed row (1–10) is GREEN on a release-candidate commit. ✅
2. The operator enables `KATPOOL_API_PORT` on `tn10` (loopback bind + nginx),
   exercises every endpoint against `katpool_tn10` with the live Goldshell
   wallet/worker, cross-checks `/pool/hashrate` against `/metrics`, and archives
   the captures under `api-evidence/` (row 11 → GREEN). ✅
3. `fail2ban` jail config for 4xx bursts is staged for the Phase 7/8 edge.
   ⏳ deferred to the Phase 7/8 production edge (out of scope above).

## Live tn10 go-live (2026-06-02)

The API was enabled on `tn10-phase5` at `KATPOOL_API_PORT=127.0.0.1:18080` and
the unified runtime redeployed (`scripts/deploy.sh --network tn10`). The
startup log confirmed `public read-only API enabled addr=127.0.0.1:18080` and
the service came up active alongside the live KAS + KRC-20 payout engines. All
three probes, every `/api/v1` endpoint, the 400/404 error paths, and the 429
rate limit were verified against live Goldshell-soak data; the hashrate was
cross-checked against `/metrics` (`:9302`). Evidence:
`api-evidence/2026-06-02T21-09Z-tn10-api-acceptance.md`.

Isolation from the legacy mainnet pool (a separate Docker stack on distinct
ports/services) was confirmed before deploy: `18080` was free and is
loopback-only, and the deploy touched only `katpool-tn10.service`.
