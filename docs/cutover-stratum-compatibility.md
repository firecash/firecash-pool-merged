# Stratum Connection Compatibility & Geo-Edge Map

Read-only mapping + gap analysis for the mainnet cutover. Its single
goal: **miners take zero action at cutover** — every rig keeps the
hostname and port it already has, and just keeps mining against the new
pool. This is a *connection-compatibility* exercise, **not** a port of
legacy difficulty logic: the new pool keeps variable difficulty
everywhere; legacy ports become starting-difficulty *seeds* only.

Companion to [`cutover-plan.md`](cutover-plan.md). Supersedes the
stratum-edge portion of
[ADR-0005](decisions/0005-netcup-vps-railway-edge.md) (see Gap G2).

## 1. Legacy topology (verified)

Sources: live legacy `config/received_config.json`, the legacy
`katpool-app` `docker-compose.yml` (binds ports `1111–8888`), and the
operator.

- **Origin**: `kas.katpool.com` — the unified pool host on the NetCup
  VPS. Not one of the regional names.
- **Geo edge**: 7 regional subdomains, each a *thin TCP forwarder*
  hosted on **fly.io**, exposing **all 8 ports** and forwarding
  everything back to the origin.
- **TLD**: only `.xyz` is active in the legacy pool. `.com` is **not**
  used by the legacy pool.
- **Difficulty**: each port advertises a fixed starting difficulty;
  ports `1111–7777` run `varDiff: false` (pinned), port `8888` runs
  `varDiff: true`.

### Endpoint inventory

| Hostname | Region | fly.io region code¹ |
|---|---|---|
| `kas.katpool.com` (origin) | NetCup (Germany) — pool origin | n/a |
| `na-west.katpool.com` | California, US | `sjc` |
| `na-east.katpool.com` | Virginia, US | `iad` |
| `eu.katpool.com` | Germany | `fra` |
| `ap.katpool.com` | Singapore | `sin` |
| `hkg.katpool.com` | Hong Kong | `hkg` |
| `sa.katpool.com` | Brazil | `gru` |
| `au.katpool.com` | Australia | `syd` |

¹ Region codes are the expected fly.io mapping; confirm against
`flyctl platform regions` at implementation time (Gap-validation T1).

### Port → starting-difficulty seed (verified from `received_config.json`)

The new pool runs **vardiff on every port**. The legacy `difficulty`
value becomes the **initial** difficulty sent at authorize — a *start
only*, never a floor or ceiling. Vardiff then moves freely from there
(new-pool bounds apply, not the legacy per-port bands).

| Port | Seed (initial diff) | Legacy `varDiff` | Legacy band (min–max) |
|---|---|---|---|
| 1111 | 256 | false | 256–512 |
| 2222 | 1024 | false | 512–1024 |
| 3333 | 4096 | false | 2048–4096 |
| 4444 | 8192 | false | 4096–8192 |
| 5555 | 16384 | false | 16384–32768 |
| 6666 | 32768 | false | 32768–65536 |
| 7777 | 65536 | false | 65536–131072 |
| 8888 | 2048 | true | 64–131072 |

All legacy ports use `sharesPerMinute: 10`, `extraNonceSize: 2`,
`clampPow2: true`.

## 2. New-pool target topology

- **Origin** unchanged: NetCup VPS runs the unified `katpool` binary,
  listening on all 8 stratum ports.
- **Geo edge**: a single **fly.io anycast** app (one global IP) running
  a thin TCP forwarder, deployed to the 7 regions above. Anycast routes
  each miner to the nearest healthy region automatically — no per-region
  DNS logic. Validated: fly.io allows
  [any public TCP port](https://community.fly.io/t/new-feature-every-public-port-now-allowed-for-tcp-services-this-means-http-too/3624)
  (declare all 8 in `fly.toml`; no port-range syntax yet, so 8 explicit
  entries), with global anycast routing at low cost
  (~$5/region/mo `shared-cpu-1x` + ~$2 IPv4 + $0.02/GB; stratum traffic
  is tiny).
- **DNS**: all 7 legacy `.xyz` subdomains + the origin point at the new
  edge (A/AAAA to the fly anycast IP). Anycast means the 7 names can all
  resolve to **one** app — kept distinct purely for backward
  compatibility, not because separate forwarders are still required.
- **`.com` mirror**: the new pool adds `kas.katpool.com` and the 7
  regional `.com` subdomains, mirroring `.xyz` 1:1 for forward
  compatibility.

## 3. Verified bridge compatibility

- **Handshake**: `handle_authorize` parses `params[0]` as
  `address.worker[.canxium]` (bech32-validated) and **never reads the
  password (`params[1]`)** — a legacy `d=<diff>` password is silently
  ignored, never rejected. Existing miner configs authorize unchanged.
- **Vardiff**: `vardiff_compute_next_diff` moves difficulty up or down
  with only an absolute floor of `1.0`; the per-client `min_diff` field
  is the *current working difficulty*, not a clamp. A per-port seed is
  therefore a pure starting value — exactly the "start only" model.

> **Status (2026-06-02):** ratified and implemented in
> [ADR-0022](decisions/0022-multiport-stratum-and-flyio-anycast-edge.md).
> G1, G3, G5 are **done** in code (multi-port + per-port seed +
> PROXY-protocol v2, unit-tested); the fly.io edge lives in
> [`ops/edge/flyio/`](../ops/edge/flyio/README.md). Gaps below are kept
> for the audit trail; see the per-gap notes.

## 4. Gap analysis

### G1 — Unified runtime binds a single stratum port — ✅ DONE

`katpool/src/main.rs` reads one `KATPOOL_STRATUM_PORT`
(`KATPOOL_MIN_SHARE_DIFF` default 4096, `KATPOOL_VAR_DIFF` default true)
and calls `listen_and_serve_with_events` once. The legacy contract is 8
ports. The bridge core already supports multiple instances (standalone
`app_config.rs` instance array), so this is **wiring**, not new design:
the runtime must bind all 8 ports, each with its per-port seed
(Section 1 table) as the initial difficulty, sharing one event bus +
kaspad connection. Required change: accept a port→seed list and spawn a
listener per port (or a multi-port listener variant).

**Implemented:** `KATPOOL_STRATUM_PORTS=port:seed,…` builds one shared
pipeline and spawns one listener per port; `StratumContext.local_port`
selects the per-port seed via `ClientHandler::seed_for_port`. Empty =>
single-port (standalone binary unchanged). Unit-tested.

### G2 — Edge technology: Railway cannot serve these ports — ✅ DECIDED

ADR-0005 chose Railway TCP proxy for the edge. **Validated as
unsuitable**: Railway assigns an immutable, Railway-chosen proxy port
(e.g. `…:15140`) and
[does not allow choosing a custom port](https://station.railway.com/questions/railway-tcp-proxy-port-d05d9190);
custom domains via CNAME still force connecting on Railway's port. A
miner dialing `eu.katpool.com:7777` cannot be served. **Decision: use
fly.io anycast** (Section 2), which also matches the legacy edge. The
edge portion of ADR-0005 is superseded by ADR-0022; the fly app is in
`ops/edge/flyio/`.

### G3 — Client IP is lost behind any forwarder (no PROXY protocol) — ✅ DONE

The stratum listener takes `addr.ip()` directly from
`listener.accept()`; there is **no PROXY-protocol parsing** in the
bridge. Behind a TCP forwarder the origin sees the forwarder's IP, which
breaks:
- per-IP anti-abuse (bad-address ban, frame-rate limiting keyed on
  `ctx.remote_addr`), and
- the per-IP attribution recorded on each share (`share_handler.rs`).

Fix: forwarder sends PROXY protocol (e.g. HAProxy `send-proxy`) and the
bridge stratum listener parses the PROXY header to recover the real
miner IP. This is a required addition before the edge goes live.

**Implemented:** HAProxy `send-proxy-v2`; the bridge parses PROXY v2
(`ppp` crate) before anti-abuse/attribution, gated by
`KATPOOL_STRATUM_PROXY_PROTOCOL=true`, with the origin firewalled to the
fly egress (nftables template in the edge README). Unit-tested
(IPv4/IPv6 parse, payload preservation, non-PROXY rejection).

### G4 — `.com` is new — ◑ DNS documented

`.com` was never active in legacy. New pool must provision DNS + (for
HTTP API/dashboard) TLS for `katpool.com` and all subdomains. Stratum
itself is plain TCP (no TLS), so the `.com` stratum names need only
DNS to the anycast IP; TLS applies to the API/dashboard hosts.

### G5 — Seed must not become a clamp — ✅ HONORED

The per-port seed sets only the *initial* difficulty. Vardiff bounds
must remain independent so difficulty is free to move above/below the
seed. (Verified feasible in §3; called out so the G1 implementation does
not reintroduce the legacy fixed bands.)

## 5. Decisions captured

- **Difficulty**: vardiff everywhere; legacy port → **starting seed
  only** (Section 1 table), not a min/max. (Operator, 2026-06-02.)
- **Edge**: **fly.io anycast** thin TCP forwarders; supersedes ADR-0005
  edge. (Operator, 2026-06-02.)

## 6. Open items / validation TODO

- T1: confirm fly.io region codes against `flyctl platform regions`
  (edge README uses sjc/iad/fra/sin/hkg/gru/syd).
- ~~T2: multi-port + per-port-seed wiring (G1).~~ ✅ done (ADR-0022).
- ~~T3: PROXY-protocol on forwarder + bridge listener (G3).~~ ✅ done.
- T4: decide whether to keep 7 distinct names long-term or collapse to
  anycast behind fewer names (compat requires *answering* all 7
  regardless).
- T5: provision `.com` DNS/TLS — stratum DNS documented (edge README);
  API/dashboard TLS handled with the dashboard work.
- T6 (**pre-cutover, live**): allocate the dedicated anycast IPv4 +
  per-region egress IPs, apply the origin nftables allowlist, deploy the
  fly edge, then mine the tn10 Goldshell through it and confirm the
  origin logs the ASIC's real IP and the correct per-port seed. Then
  load/latency test against the legacy footprint.
- ~~T7: formalize the superseding edge ADR.~~ ✅ ADR-0022.
