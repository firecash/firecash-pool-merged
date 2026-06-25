# katpool road-to-mainnet roadmap

The single tracked source of truth for **what is done and what remains** before
the mainnet cutover. Update it in the same PR that changes a status. It replaces
the previously-untracked "road-to-mainnet plan" the README and CHANGELOG
referenced.

Two planning axes run in parallel:

- **Phases 1–10** — the lifecycle milestones (build → harden → cutover). Each
  closed phase has a `docs/phase-N-acceptance.md`; cutover is gated by
  [`docs/cutover-plan.md`](cutover-plan.md).
- **Workstreams A–H** — the post-Phase-6 hardening tracks (config, telemetry,
  edge, mainnet node, money-safety, deploy). Tracked in `CHANGELOG.md` and here.

Status legend: ✅ done · ◑ in progress / code-landed-not-live · ❌ not started.

> **Current state (2026-06-22):** mainnet cutover is **complete** — the Rust
> pool serves production stratum, KAS + NACHO payouts, and the public API.
> Remaining work is operational hardening (edge, observability SLIs, resilience
> drills), not core money-path features.

## Phases

| Phase | Scope | Status | Evidence |
|---|---|---|---|
| 1 | Stratum bridge / shares | ✅ | `docs/phase-1-acceptance.md` (Goldshell soak) |
| 2 | Legacy import | ✅ | `docs/phase-2-acceptance.md` |
| 3 | Accountant / maturity / allocation | ✅ | `docs/phase-3-acceptance.md`; ADR-0014 |
| 4 | KAS payouts | ✅ live on mainnet | `docs/phase-4-acceptance.md` |
| 5 | KRC-20 / NACHO payouts | ✅ live on mainnet | `docs/phase-5-acceptance.md` |
| 6 | Public read-only HTTP API | ✅ live on mainnet | `docs/phase-6-acceptance.md` |
| 7 | Production edge + hardening | ◑ code landed; live bring-up pending | §"Open gaps" |
| 8 | Multi-region edge + key-rotation auditor | ◑ edge → fly.io (ADR-0022); auditor pending | ADR-0022; Runbook 11 |
| 9 | Resilience (chaos / load / DR / on-call) | ❌ | `docs/cutover-plan.md` gates |
| 10 | Cutover (72h shadow + importer + rollback) | ✅ | `docs/cutover-plan.md`; evidence under `cutover-evidence/` |

## Workstreams A–H

| Code | Item | Status | Evidence |
|---|---|---|---|
| A2 | Graceful shutdown drains event backlog | ✅ | PR #82 |
| A3 | Layered YAML/TOML config (`katpool-config`) | ✅ | PR #87 |
| A4 | Dedicated liveness/readiness probe port | ✅ | PR #85 |
| A6 | Session-recording cleanup / test hardening | ✅ | PR #86 |
| **A7** | Re-vendor bridge from upstream v2.0.0 source | ❌ deferred | `bridge/UPSTREAM.md` |
| B1 | Structured telemetry wiring (JSON logs + OTLP) | ✅ | PR #83 |
| B2 | Treasury/wallet address redaction | ✅ | PR #83 |
| B3 | LGTM deploy artifacts (thin Dockerfiles + `railway.toml`) | ✅ | PR #93 |
| B4 | LGTM provisioned + live on Railway (11 services) | ✅ | `ops/railway/observability/deploy/` |
| B5 | Origin metrics → `vmauth` → VictoriaMetrics | ✅ live | origin `vmagent` |
| B6 | Origin logs + traces → `vmauth` → Loki/Tempo (Alloy + span instrumentation) | ✅ live | PR #95, #96 |
| B7 | Payout/treasury metrics + share-accept latency histogram + canary miner | ❌ | `ops/railway/observability/SLO.md` |
| C2 | Origin stratum firewall (nftables) | ✅ code | PR #91 |
| C | Multi-port stratum + fly.io anycast edge | ◑ code done; live deploy + load test pending | ADR-0022 |
| E2 | Mainnet API rate-limit default fix | ✅ | PR #92 |
| G1 | Per-cycle treasury spend cap (money circuit breaker) | ✅ | PR #82 |
| H1 | Cosign-verified deploys | ✅ | PR #88 |
| H2 | Deploy readiness gate | ✅ | PR #88 |

(The mainnet kaspad node is governed by **ADR-0010**, not an "E1" workstream: a
dockerized mainnet kaspad already runs co-resident on the VPS and is left
untouched until cutover.)

## Observability completion (B7) — remaining SLIs

The stack is live, but two SLOs in `ops/railway/observability/SLO.md` are not yet
backed by emitted signals:

- **Payout-cycle + treasury-balance metrics** from the origin (today payout
  health is inferred via Loki log rules + the canary probe).
- **Share-accept latency histogram** (the API/bridge latency SLI).
- **Canary miner** binary — the `CanaryMinerNotPaid` SLO depends on it.

## Mainnet cutover (complete)

Phase 10 cutover to this rebuild is **done** (see
[`docs/cutover-plan.md`](cutover-plan.md) and evidence under `cutover-evidence/`).
Remaining gates below are **post-cutover hardening**, not blockers for production
stratum or payouts:

- Phase 9 acceptance: load test, chaos drills, custody `EPERM` suite, on-call
  paging dry-run, **all runbooks signed off**.
- DR validator: **4 consecutive weekly passes** (ADR-0009; Runbook 10).

## Known deferrals (mainnet-only or future work)

- **A7** bridge re-vendor (deps are on v2.0.0; bridge *source* is still v1.1.0).
- **fly.io anycast edge** live allocation + load test (ADR-0022; testnet stays
  origin-direct on `152.53.37.182:15555`).
- **Kasplex KRC-20 crediting** (NACHO does not exist on tn10; L1 acceptance is
  the tn10 success criterion).
- Live `KasplexTierClassifier` at allocation (tn10 uses `static`).
- `fail2ban` jail on 4xx bursts; least-privilege read-only DB role for the API
  (Phase 7/8 edge hardening).
- Key-rotation auditor (Phase 8) referenced by Runbook 11.

## Open gaps (post-cutover hardening)

1. **B7**: emit payout/treasury metrics + share-accept latency histogram; build
   the canary miner.
2. **A7**: re-vendor the bridge against rusty-kaspa v2.0.0.
3. **Phase 7/8 hardening**: read-only DB role, `fail2ban`, key-rotation auditor.
4. **fly.io edge**: allocate anycast IPv4 + per-region egress, apply origin
   nftables, load/latency test with a real ASIC.
5. **Phase 9**: DR validator 4× weekly, chaos/load/soak, on-call dry-run,
   runbook sign-off.
6. **Observability durability/scale** (operability): persist the origin Alloy
   WAL, add trace sampling before mainnet API volume, right-size
   Loki/Tempo/VM retention.
