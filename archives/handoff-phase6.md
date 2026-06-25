> **Archived.** This file is kept for history only. For current docs see
> [`docs/README.md`](../docs/README.md).

# Katpool Mining Pool — Engineering Handoff (Phase 6 onward)

You are taking over an in-flight, enterprise-grade rebuild of the **katpool**
Kaspa mining pool. A long prior session completed Phases 1–5; your job is to
carry the same deterministic, production-grade discipline through Phase 6 and
onward to a mainnet cutover that replaces the legacy pool. Read this entire
prompt, then read the referenced files BEFORE writing any code.

## 0. Non-negotiable working rules (follow on every action)

1. Thoroughly check all related, dependency, and reliant code before making
   changes — achieve complete contextual awareness first.
2. Never produce boilerplate or example code. Every change must be expert-grade,
   follow the latest best practices, be error-free, and production-ready for
   THIS codebase.
3. Never assume. If you are not certain, research it (read the code, the docs,
   the ADRs; use web search or the context7 MCP for library docs) or ask the
   operator for a deterministic answer. Guesses are unacceptable.
4. Always double-check your work for completeness, accuracy, and optimization.
5. Solutions must be as minimal as possible to achieve the goal.
6. Reply concisely; avoid filler and repetition.
7. Always validate assumptions/expectations against documentation (web or
   context7 MCP), or ask the operator directly.

Quality gates that must be green before any PR is "done":
`cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets`
(workspace lints treat warnings as errors — `RUSTFLAGS=-D warnings`),
`cargo test --workspace`, and `cargo deny check`. CI enforces all of these
(`.github/workflows/ci.yml`, `security.yml`, `release.yml`). Tests must be
DETERMINISTIC: pure/chain-free unit tests, time/RNG injected, testcontainer
Postgres + mock kaspad for integration, `insta` snapshots for serialized
output. No flaky tests, no network in unit tests. See
`docs/decisions/0013-verification-posture.md` — read it first.

## 1. What this is and where it runs

- **Repo:** `/root/katpool` (git; remote `github.com/Nacho-the-Kat/katpool`;
  default branch `main`, currently at PR #54 / commit `a2007ea`). Rust-first
  monorepo; one network-agnostic binary, network chosen at runtime by the
  kaspad endpoint + address prefix.
- **You are operating ON the live testnet-10 VPS** (`root@v2202412228712306888`).
  This box runs the live soak — treat it as production-adjacent.
- **Live services:**
  - `katpool-tn10.service` → binary `/root/katpool-tn10/katpool`, instance
    `tn10-phase5`. Stratum on `0.0.0.0:15555`; Prometheus on
    `0.0.0.0:9302/metrics`.
  - `katpool-kaspad-tn10` (docker), gRPC `127.0.0.1:16210`, pinned to
    **`tn10-toc3`** (kaspad `1.2.1-toc.3`) — DO NOT change the pin without
    Runbook 13/20. A version mismatch forks the node off the network.
  - Postgres 17 (docker) `postgres://postgres:postgres@127.0.0.1:55432/katpool_tn10`.
  - Observability docker stack: Prometheus `:9090`, VictoriaMetrics `:8428`.
  - A real **Goldshell ASIC** (`BzMiner/v14.0.2`, worker `goldshell-rig`,
    from `23.148.36.54`) is mining the live soak.
- **Payouts are LIVE on tn10** (KAS and KRC-20/NACHO), 6h cadence
  (216,000 DAA), 10-KAS thresholds. Treasury key at
  `/etc/katpool/treasury-key.hex` (root-only). Pool/treasury address:
  `kaspatest:qr6mqnvwf2e2m6hlxkzje5tqczn67rx2ht3v32t352a82qzs6qrjjleqdsnfl`.
- **Deployed env:** `/etc/katpool/tn10.env`. The repo copy `ops/env/tn10.env`
  is **gitignored**; the tracked templates are `ops/env/tn10.env.example` and
  `ops/env/mainnet.env.example` — update those when you add config keys.
- **Deploy:** `scripts/deploy.sh --network tn10` (builds `--profile dist
  --locked`, installs binary+unit+env, restarts; keeps `.bak` for rollback —
  see `docs/runbooks/09-deploy-and-rollback.md`). The Goldshell auto-reconnects
  within seconds and the payout engine is crash-safe (record-before-broadcast,
  idempotent re-broadcast, advisory locks), so a clean restart is safe — but
  don't restart needlessly, and confirm the soak resumes (shares in the `share`
  table) after any deploy.

## 2. Crate map (read `docs/onboarding.md` + `docs/architecture.md`)

- `bridge/` — vendored rusty-kaspa stratum bridge (`kaspa-stratum-bridge`);
  divergences tracked in `bridge/UPSTREAM.md`. Emits `PoolEvent`s on a
  broadcast bus; anti-abuse + Prometheus in `src/anti_abuse.rs`, `src/prom.rs`.
- `accountant/` — consumes the `PoolEvent` bus, does share allocation, maturity
  tracking, custodial PROP accounting; writes the DB.
- `payout-kas/` — KAS payout engine (adaptive node fee-estimate, exact-fee
  finalization; ADR-0018).
- `payout-krc20/` — KRC-20/NACHO commit/reveal engine (adaptive frozen fees
  ADR-0019; sweep-coherent UTXO chaining ADR-0020).
- `katpool/` — the unified runtime binary (`src/main.rs`) wiring bridge +
  accountant + payout engines + (soon) API. Config is env-driven; see the
  big docstring at the top of `katpool/src/main.rs`.
- `api/` — **Phase 6 scaffold** (your next task). Stub `api/src/lib.rs`
  documents the endpoints; `api/Cargo.toml` already reserves the stack
  (`axum`, `tower`, `tower-http`, `katpool-db`, `katpool-domain`,
  `katpool-metrics`, `katpool-telemetry`, `serde`, `insta` for snapshots).
- `crates/` — `katpool-db` (sqlx + migrations), `katpool-domain` (validated
  types incl. `PoolEvent`), `katpool-secrets` (sops/age treasury custody),
  `katpool-storagemass` (mass/fee planner, `FeeRate`), `katpool-idempotency`
  (Postgres advisory locks), `katpool-metrics`, `katpool-telemetry`.
## 3. Status: what is DONE

- **Phases 1–5 complete.** Acceptance evidence: `docs/phase-1-acceptance.md`
  … `docs/phase-5-acceptance.md`. Phase 1 rows 3/6/7/12/13 were closed live
  from the Goldshell soak in PR #54; Phase 5 (KRC-20) is live on tn10 with L1
  commit/reveal acceptance (Kasplex crediting deferred to mainnet per
  ADR-0019 §3 — NACHO doesn't exist on tn10).
- Money path is feature-complete and L1-verified: KAS + KRC-20 payouts run
  live on tn10 with adaptive fees.
- Recent PRs: #50 (merger classification), #51 (KAS adaptive fee + `payout
  run-now` CLI), #52 (KRC-20 adaptive fee), #53 (KRC-20 sweep UTXO chaining),
  #54 (runtime Prometheus exporter + Phase 1 live closeout).
- ADRs 0001–0020 in `docs/decisions/` (index `docs/decisions/README.md`).
- 20 runbooks in `docs/runbooks/` (on-call, payout rehearsals 18/19, deploy 09,
  DR 04/10, kaspad bootstrap/version 13/20, importer 14, smokes 12/15/16).

## 4. Status: what is LEFT (the road to mainnet)

The cutover gates are defined in `docs/cutover-plan.md` §0 — they require
Phases 6–9 complete plus a clean shadow run. Phases (per
`docs/phase-5-acceptance.md` "Out of scope" + `docs/threat-model.md` +
ADR references):

- **Phase 6 — Public HTTP API (YOUR NEXT TASK; not started).**
- **Phase 7 — Production edge + hardening:** log/trace treasury redaction,
  mainnet-migration ADR (deferred by ADR-0010/0011), pre-cutover load testing.
  Release signing already exists (`release.yml`: musl + CycloneDX SBOM + cosign
  keyless).
- **Phase 8 — Railway 3-region TCP edge (ADR-0005); key-rotation auditor
  (Runbook 11, `db-schema.md` auditor).**
- **Phase 9 — Resilience:** chaos drills, load test, custody `EPERM` suite
  (ADR-0008), 4 consecutive weekly DR-validator passes (ADR-0009/Runbook 10),
  all runbooks signed off, on-call paging dry-run.
- **Phase 10 — Cutover:** executed 2026-06; reconcile green; evidence under
  `cutover-evidence/` and `replay-evidence/` (see `docs/cutover-plan.md`).

## 5. YOUR IMMEDIATE TASK — Phase 6: Public read-only HTTP API

Goal: implement the `api/` crate per its scaffold, wire it into the unified
runtime, and produce `docs/phase-6-acceptance.md` with a GREEN acceptance
matrix that mirrors the Phase 1–5 docs.

Before coding, READ: `api/src/lib.rs`, `api/Cargo.toml`,
`docs/architecture.md` (api is "axum read-only", rate-limited, TLS via nginx;
line ~123 table), `docs/threat-model.md` (Phase 6 rows: per-IP rate limit via
tower-governor, bounded JSON body, hard query timeouts, in-memory cache,
fail2ban for invalid bursts, pseudonymity of `/balance/:address`),
`docs/phase-5-acceptance.md` (use it as the acceptance-doc template),
`crates/katpool-db` repo layer (reuse existing queries; do NOT hand-write SQL
that duplicates repo functions), `katpool/src/main.rs` (how subsystems are
wired; note `KATPOOL_HEALTH_CHECK_PORT` and `KATPOOL_PROM_PORT` are env-gated
task spawns — follow that pattern for a `KATPOOL_API_PORT`; verify whether the
health-check port is actually wired and reconcile if not).

Endpoints (from the scaffold; confirm exact contracts against the code, don't
invent fields):
- `GET /health` — liveness (process up).
- `GET /ready` — readiness (kaspad synced + DB reachable).
- `GET /started` — startup probe (initial sync complete).
- `GET /balance/:address` — miner balance (path carries the address;
  pseudonymity model per threat-model).
- `GET /api/pool/stats` — aggregate pool stats.
- `GET /api/pool/hashrate` — current pool hashrate.
- `GET /full_rebate/:address` — full-rebate eligibility.

Constraints: reads PostgreSQL only; no funds, no secrets ever; all queries
bounded by hard timeouts + an in-memory cache; per-IP rate limiting; bounded
request body; addresses sanitized (last-4) in logs/traces. Decide single-
process-embed vs separate binary by reading `architecture.md` (it specifies a
single process embedding bridge+accountant+payout+API) — embed behind
`KATPOOL_API_PORT`, mirroring the existing env-gated task spawns. If you hit a
genuine architectural fork (e.g., embed vs standalone, cache strategy, rate-
limit library choice), STOP and present options to the operator rather than
guessing.

Tests: `insta` snapshot tests for every JSON response shape; testcontainer
Postgres for DB-backed endpoints; mock/blackbox for `/ready` kaspad-sync.
Cover error paths (unknown address, DB timeout, malformed path) deterministically.

Deliverables for the PR: the `api` implementation, runtime wiring, env keys
added to BOTH `.example` templates, `docs/phase-6-acceptance.md` (GREEN
matrix + how-to-re-run), a CHANGELOG entry under `[Unreleased]`, and a new ADR
in `docs/decisions/` (next number 0021) IF you make any non-trivial decision
(rate-limit lib, cache TTL, embed vs standalone) — register it in
`docs/decisions/README.md`. Add a runbook only if the API introduces a new
operational surface.

### 5.1 Operator latitude — build toward a world-class dashboard

The operator EXPLICITLY welcomes expanding the API and metrics surface beyond
the minimal scaffold endpoints, to fully enterprise-grade, if you see fit. A
near-future task is a complete rebuild of the miner dashboard web app into an
**enterprise-grade, innovatively gorgeous (UI/UX), and data-dense** dashboard
that makes every other mining pool look like a rookie attempt — and a robust,
rich data API is the foundation that makes that possible. So:

- Treat the listed endpoints as the floor, not the ceiling. Design the API as
  a first-class, well-shaped data product: clean resource modeling, consistent
  pagination/filtering/time-range semantics, stable versioned response schemas,
  per-worker and per-wallet granularity, historical time-series (hashrate,
  shares, earnings, rejects, block lifecycle), pool-wide aggregates, payout
  history, and rebate/NACHO accounting — anything a dense, real-time, beautiful
  dashboard would need.
- Expand the Prometheus/metrics surface in the same spirit where it serves
  observability or dashboard data needs (the exporter wiring landed in #54;
  `katpool-metrics`/`katpool-telemetry` are the homes for this).
- Keep every addition under the SAME rules: deterministic tests, read-only and
  secret-free, bounded/cached/rate-limited, documented (ADR + CHANGELOG +
  acceptance), and minimal-but-complete. Do not gold-plate speculatively beyond
  what a serious dashboard would consume; when a larger expansion is a judgment
  call, propose the shape to the operator first.
- The dashboard rebuild itself is a SEPARATE future phase — do not start
  frontend work now. Phase 6's job is to make the data layer so good that the
  dashboard becomes straightforward. If a design choice now would meaningfully
  help or hinder that future dashboard, optimize for the dashboard.

## 6. Workflow & discipline

- **Branch → PR → CI → operator merges.** Never push to `main` directly; never
  force-push; never touch git config; never skip hooks. Create a feature
  branch, commit with conventional-commit messages (e.g.
  `feat(api): ...`, `fix(runtime): ...`), push, open a PR with a Summary +
  Test plan, and let the OPERATOR review and merge (they merge and delete the
  branch, then tell you). After they confirm, `git switch main && git pull
  --ff-only && git branch -d <branch>`.
- **Documentation is part of "done":** every behavioral change updates
  `CHANGELOG.md` (`[Unreleased]`, Keep-a-Changelog sections); every decision is
  an ADR (status/date/deciders front-matter, Context/Drivers/Decision/
  Consequences/Confirmation) registered in `docs/decisions/README.md`; every
  phase closes with a `docs/phase-N-acceptance.md` GREEN matrix backed by real
  evidence (prefer live tn10 evidence measured from the DB/logs/`/metrics`,
  not assertions). Mirror the existing docs' tone and structure exactly.
- **Live-evidence gathering** (you are on the box): query the DB
  (`psql "$KATPOOL_DATABASE_URL"` after sourcing `ops/env/tn10.env`; tables
  `share`, `share_reject`, `block`, `payout`, `payout_cycle`,
  `krc20_pending_transfer`, `nacho_rebate_accrual`, `worker`, `wallet`),
  read logs (`journalctl -u katpool-tn10`), scrape `:9302/metrics`, and check
  tn10 L1 via `https://api-tn10.kaspa.org/...`. Never fabricate numbers.
- **Operator interaction:** for scope changes, destructive actions, or live-
  deployment changes (restart, config, re-deploy, resetting stuck rows), ASK
  first with concrete options. For reversible local choices (naming, file
  layout), decide and note it.

## 7. Hard-won gotchas (do not relearn these the hard way)

- The unified runtime spawns Prometheus/health/API as env-gated tasks; a config
  field existing on a struct does NOT mean it's wired (that was the #54 bug:
  `prom_port` was carried but never started; `start_prom_server` is also what
  runs `init_metrics()`, so all `record_*` were silent no-ops). Verify wiring,
  don't assume.
- `/metrics` is **instance-filtered** by the `instance` label; pass the same
  `instance_id` the counters are recorded with (`tn10-phase5`) or series vanish.
- Counters with labels only appear on `/metrics` AFTER the first increment.
- KRC-20 record-before-broadcast means commit/reveal txids must be reproducible
  across restarts — fees are frozen per-transfer (ADR-0019); never re-quote on
  resume. UTXO selection in a settle sweep must chain via the `SweepLedger`
  (ADR-0020) or sibling commits double-spend.
- `treasury_utxos`/`get_utxos_by_addresses` return only CONFIRMED coins.
- The kaspad tn10 pin (`tn10-toc3`) is consensus-critical; bumps follow
  Runbook 20.
- Env files with values are gitignored; edit the `.example` templates for
  anything that must land in the repo.
- Many third-party GitHub Actions are SHA-pinned; keep that discipline.

## 8. First moves

1. Read §0 references + ADR-0013 + `docs/dev-workflow.md` + `docs/onboarding.md`.
2. Read the Phase 6 references in §5; inspect the live DB schema and the
   `katpool-db` repo functions you'll reuse.
3. Produce a short Phase 6 implementation plan (endpoints, wiring, tests,
   security, docs) and confirm any architectural forks with the operator.
4. Implement on a feature branch with deterministic tests; keep the quality
   gates green; open a PR; let the operator merge.

Confirm you've read the referenced files and present your Phase 6 plan before
writing code.
