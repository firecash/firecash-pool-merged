---
status: accepted
date: 2026-05-26
deciders: argonmining
---

# ADR-0010: Multi-tenant kaspad on the pool VPS (mainnet + testnet-10 co-resident)

## Context and Problem Statement

Phase 1 acceptance requires a live smoke against testnet-10 (≥ 100
shares, ≥ 1 block, boot < 5 s — see
[`docs/phase-1-acceptance.md`](../phase-1-acceptance.md) rows 11–13).
There is no reliable public Toccata-aware gRPC endpoint for
testnet-10: `n-testnet-10.kaspa.ws:16210` was reachable at the
publication time of the
[port-selection docs](https://kaspa.aspectron.org/rpc/ports.html) but
the gRPC layer is unreachable today, and the kaspa-ng resolver
infrastructure has moved to wRPC v2 (Borsh-encoded WebSockets) per
[rusty-kaspa #506](https://github.com/kaspanet/rusty-kaspa/pull/506),
which our gRPC-only bridge does not speak.

The same VPS already runs the legacy production pool: a dockerized
`supertypo/rusty-kaspad:v1.0.1` against a 128 GB on-disk mainnet data
dir at `/root/docker_deployment/kaspad_mainnet`, plus the legacy
`katpool-app` (stratum on `:5555`), `katpool-payment`, postgres,
prometheus, victoria-metrics, and an nginx front. Replacing or
restarting that stack carries production risk and is unrelated to
Phase 1's deliverables.

Layered on top of those constraints is the **Toccata hardfork**:
[`tn10-toc2`](https://github.com/kaspanet/rusty-kaspa/releases/tag/tn10-toc2)
scheduled testnet-10's Toccata activation at DAA score 467,579,632
on 2026-05-18 16:00 UTC. The chain is post-fork as of writing
(virtual DAA 474M+). Our pinned `kaspa-*` crates are `v1.1.0` from
2026-03-04 — they predate Toccata. A vendored kaspad linked into our
own bridge (inprocess mode) would fork off the testnet-10 chain at
the Toccata DAA score; only the upstream `tn10-toc2` binary contains
the new consensus rules.

## Decision

1. **Keep the existing dockerized mainnet kaspad running, untouched,
   for the lifetime of Phase 1.** Production legacy pool depends on
   it. Migrating to a systemd-managed v1.1.0+ kaspad is deferred to
   Phase 7 (cutover) where it's scheduled to happen alongside the
   stratum-bridge switchover and shadow run.

2. **Run a second kaspad as a hardened systemd service** —
   `katpool-kaspad-tn10` — bound to the rusty-kaspa testnet-10 port
   convention (16210 gRPC, 16211 P2P, 17210/18210 wRPC) on the same
   VPS. Data dir at `/var/lib/kaspad-tn10/`, dedicated `kaspad-tn10`
   system user, no shared state with the legacy mainnet container.

3. **Run the testnet-10 service from the upstream `tn10-toc2`
   release zip with a pinned SHA-256.** Binary distribution is
   sufficient — there is no Phase 1 customisation of kaspad itself,
   only of our bridge. The pinned tag and digest are in
   `ops/kaspad/install-kaspad-tn10.sh` (one-line bump procedure on
   upstream rev).

4. **The Phase 1 acceptance smoke runs in `--node-mode external`
   only**, pointed at the local `katpool-kaspad-tn10`. The bridge
   never embeds a kaspad in inprocess mode for testnet-10 work
   during Phase 1, because the embedded crates predate Toccata.

5. **Mainnet migration ADR is deferred** to Phase 7's cutover plan
   ADR. That work needs to choose between (a) reusing the existing
   128 GB data dir under a new v1.1.0+ binary (saves IBD), or (b)
   full re-sync to a fresh appdir. The choice depends on whether
   data-format upgrades are needed across the v1.0.1 → v1.1.x+
   span, which we cannot answer until we know the target version.

## Capacity sanity-check

NetCup VPS (20 vCPU / 94 GiB RAM / 3 TB SSD, see
[`docs/capacity-plan.md`](../capacity-plan.md)). Current load is
~11 GiB RAM, ~10% CPU. Adding `kaspad-tn10`:

| Component | vCPU | RAM | Disk |
|---|---|---|---|
| existing dockerized mainnet kaspad | 2–3 | ~10 GB | 128 GB (live) |
| new `katpool-kaspad-tn10` | 1–2 | ~5 GB | ~30 GB (post-IBD; testnet-10 is much smaller than mainnet) |
| legacy katpool stack (katpool-app, monitor, payment, postgres, nginx, prometheus, vm) | 2 | 4 GB | ~50 GB |
| **Total at saturation** | ~7 | ~19 GB | ~210 GB |
| Headroom remaining | 13 vCPU | 75 GiB | 1.3 TB |

Comfortable. Adds a third tenant later (the new katpool process
during Phase 2–6) without revisiting this ADR.

## Consequences

- Smoke harness needs `--node-mode external` only — already true of
  `scripts/testnet10-smoke.sh`.
- Two kaspad processes write blockchain data to disk concurrently;
  pgBackRest scope explicitly excludes both kaspad data dirs (those
  resync from peers).
- `systemd-analyze security katpool-kaspad-tn10` ≥ 1.5 OK exposure
  level is part of the install acceptance (current measured: 1.2).
- When upstream cuts `tn10-toc3` (next testnet-10 maintenance
  release) the bump is a one-line tag + SHA-256 update in the
  installer; see `docs/runbooks/13-kaspad-tn10-bootstrap.md`.
- Phase 7 cutover ADR must address the mainnet migration. It is
  explicitly *not* in scope here.

## Considered alternatives

- **Single shared kaspad.** Rejected: production-legacy mainnet
  cannot serve testnet-10 traffic, and the v1.0.1 image predates
  Toccata anyway.
- **Smoke against a community public endpoint.** Rejected: the gRPC
  layer of the documented public node `n-testnet-10.kaspa.ws:16210`
  is currently down; the kaspa-ng resolver requires wRPC v2 which
  our bridge does not speak. Even if a working public endpoint were
  found, version drift across the Toccata fork would make smoke
  results non-deterministic.
- **Run a vendored Toccata-aware kaspad in the bridge's inprocess
  mode.** Rejected: re-vendoring rusty-kaspa to a Toccata-aware
  branch invalidates the lockfile pinning we built in
  [ADR-0002](0002-fork-rusty-kaspa-bridge.md). Phase 1's smoke can
  use external mode without that cost.

## Status

Accepted on 2026-05-26 alongside PR opening the kaspad-tn10
service + smoke run.
