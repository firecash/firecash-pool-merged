---
status: accepted
date: 2026-06-02
deciders: argonmining
consulted: argonmining
informed: argonmining
---

# ADR-0022: Multi-port stratum + fly.io anycast edge for zero-action mainnet cutover

## Context and Problem Statement

The mainnet cutover from the legacy `Nacho-the-Kat/katpool-app` stack
to this rebuild must require **zero action from miners**: every rig
keeps the hostname and port already in its config and just keeps mining
against the new pool. See
[`cutover-stratum-compatibility.md`](../cutover-stratum-compatibility.md)
for the verified endpoint map.

Two facts from that map create the problem:

1. **Legacy exposes 8 stratum ports** (`1111–8888`), each advertising a
   fixed starting difficulty (verified from the live legacy
   `received_config.json`). The unified `katpool` runtime binds a
   **single** `KATPOOL_STRATUM_PORT` via one
   `listen_and_serve_with_events` call.
2. **Legacy geo edge** is 7 regional **fly.io** thin TCP forwarders
   (`na-west`, `na-east`, `eu`, `ap`, `hkg`, `sa`, `au` — all `.xyz`),
   each exposing all 8 ports and forwarding to origin `kas.katpool.com`.
   [ADR-0005](0005-netcup-vps-railway-edge.md) chose **Railway TCP
   proxy** for the new edge — but Railway assigns an immutable,
   Railway-chosen proxy port and
   [cannot expose a chosen port](https://station.railway.com/questions/railway-tcp-proxy-port-d05d9190)
   like `7777`. A miner dialing `eu.katpool.com:7777` cannot be served.

The new pool keeps **variable difficulty**; legacy ports must become
*starting-difficulty seeds only* (a start, never a floor/ceiling —
verified feasible: the bridge vardiff loop moves freely above/below the
seed down to an absolute floor of 1.0). The new bridge already accepts
the legacy handshake unchanged (`addr.worker` login; the `d=` password
is ignored, never rejected).

This ADR settles (a) how the runtime serves all 8 ports with per-port
seeds, and (b) what technology replaces the Railway edge.

## Decision Drivers

- **Zero miner action** at cutover (the overriding constraint).
- **Keep variable difficulty**; port/password are seeds at most.
- **Global low latency** at **very low cost** (operator constraint).
- **Preserve per-IP anti-abuse + per-IP share attribution** — both key
  off the TCP peer IP, which any forwarder hides unless the real client
  IP is carried through.
- **Minimal, low-risk change to the mainnet hot path** (share/vardiff/
  job code is delicate).
- **Backward compatibility** with the standalone bridge binary (its
  multi-instance YAML path must keep working unchanged).

## Considered Options

Runtime (serving 8 ports + per-port seed):

1. **N independent bridge instances.** Call
   `listen_and_serve_with_events` once per port; each builds its own
   `ShareHandler` + `ClientHandler` (seeded with that port's diff) +
   block-template listener + anti-abuse guard.
2. **One shared pipeline, N port listeners, per-port seed by local
   port.** Build the pipeline once; spawn one `StratumListener` per
   port; select the per-connection initial difficulty from the
   connection's *local* (listening) port.

Edge:

A. **fly.io anycast** forwarder (HAProxy) with `proxy_proto`.
B. **Self-managed micro-VPS** forwarders (HAProxy on cheap regional
   VPS / Oracle free tier).
C. **Keep Railway** (ADR-0005).
D. **Cloudflare Spectrum** (L4 anycast).

## Decision Outcome

**Runtime: Option 2.** **Edge: Option A (fly.io anycast).**

### Runtime — one shared pipeline, per-port seed

`listen_and_serve_with_events` keeps building exactly one
`ShareHandler`, one `ClientHandler`, one block-template listener, one
vardiff thread, and one anti-abuse guard. It is extended to bind a
**list** of ports and spawn one `StratumListener` per port over the
shared handler set. The per-connection starting difficulty is selected
by the **local listening port**:

- `StratumContext` gains a `local_port: u16`, captured at accept time.
- `ClientHandler`'s single `min_share_diff: f64` becomes a
  `port_seeds: HashMap<u16, f64>` plus a `default_seed` fallback. The
  two existing seed-application sites (the first-job
  `send_client_diff` + `set_client_vardiff` calls) look up
  `seed_for(ctx.local_port)` instead of a scalar. Vardiff is otherwise
  untouched, so the seed remains a pure start.
- `BridgeConfig` gains `stratum_ports: Vec<(String, u32)>` (port,
  seed). When empty, behavior is identical to today's single
  `stratum_port` / `min_share_diff` — so the **standalone binary and
  its multi-instance YAML are unchanged**.

This reuses the `new_block_available`/vardiff hot path verbatim (all
clients live in the one `ClientHandler`'s map regardless of port), keeps
anti-abuse and attribution **global per-IP**, and avoids N duplicate
block-template subscriptions to kaspad. Rejected Option 1 precisely
because it duplicates the block pipeline N× and fragments anti-abuse
per-port.

The per-port seed table (from legacy `received_config.json`):

| Port | Seed | Port | Seed |
|---|---|---|---|
| 1111 | 256 | 5555 | 16384 |
| 2222 | 1024 | 6666 | 32768 |
| 3333 | 4096 | 7777 | 65536 |
| 4444 | 8192 | 8888 | 2048 |

### Edge — fly.io anycast forwarders with PROXY protocol

A single fly.io app runs a thin **HAProxy** TCP forwarder, deployed to
the regions matching the legacy footprint (`sjc`, `iad`, `fra`, `sin`,
`hkg`, `gru`, `syd`), fronted by one **anycast** IP. Validated:
fly.io allows
[any public TCP port](https://community.fly.io/t/new-feature-every-public-port-now-allowed-for-tcp-services-this-means-http-too/3624)
(8 explicit `[[services.ports]]` entries; no range syntax yet) and
anycast routes each miner to the nearest healthy region. All 7 legacy
`.xyz` hostnames + the new `.com` mirror point at the anycast IP; they
stay distinct only for backward compatibility.

**Client-IP preservation (required, or per-IP anti-abuse blinds):**
fly's [`proxy_proto` handler](https://fly.io/docs/networking/services/)
prepends the real client IP+port to each connection reaching the fly
machine. HAProxy is configured `accept-proxy` (from fly) and
`send-proxy-v2` to the NetCup origin. The origin's stratum listener
parses the PROXY v2 header (via the mature
[`ppp`](https://crates.io/crates/ppp) crate) **before** building
`StratumContext` and **before** the anti-abuse connection cap, so the
real miner IP drives both attribution and abuse limits.

**PROXY trust model (decided):** the origin's stratum ports accept
PROXY headers **only** from the fly forwarder egress, enforced by
**nftables allowlisting the fly egress addresses + PROXY-required** on
those ports. (WireGuard peering was considered as a hardened variant and
deferred.) PROXY parsing is gated to trusted sources; a PROXY header
from a non-allowlisted peer is rejected. **All public hostnames —
including the origin name `kas.katpool.com` — resolve to the fly edge**,
so every miner connection arrives PROXY-fronted and the origin runs a
single uniform code path (the raw origin stratum ports are never the
miner-facing entry point in production). The bridge's PROXY parsing is
behind a per-listener flag (`KATPOOL_STRATUM_PROXY_PROTOCOL`), default
off so tn10/direct and the standalone binary are unaffected; mainnet
sets it on.

### Consequences

- Positive: existing miner configs (hostname + any of 8 ports +
  `d=` password) connect and mine unchanged — zero action.
- Positive: vardiff retained; per-port seed only softens the t0 share
  rate for big rigs.
- Positive: one block-template pipeline, one kaspad connection for the
  bridge, global per-IP anti-abuse.
- Positive: standalone bridge untouched (empty `stratum_ports`).
- Positive: low recurring cost (~$10–25/mo for the fly edge).
- Negative: new PROXY-protocol parse path on the hot accept loop.
  Mitigation: gated behind a per-listener flag (off by default → origin
  behaves exactly as today when not fronted), trusted-source-only,
  covered by unit + integration tests.
- Negative: edge adds a hop; job-delivery latency is still
  origin-RTT-bound (fly node → NetCup), so the edge mainly stabilizes
  connections/handshake rather than eliminating distance. Accepted;
  matches legacy behavior.
- Negative: fly egress trust requires maintaining an allowlist (or
  WireGuard). Mitigation: documented in the cutover runbook.

### Confirmation

- Unit tests: port→seed selection; PROXY v1/v2 parse + trusted-source
  rejection.
- Integration test: connect on each of the 8 ports, assert the
  `mining.set_difficulty` seed matches the table, then assert vardiff
  moves off the seed.
- Integration test: a PROXY-fronted connection yields the real client
  IP in `StratumContext.remote_addr` and in the recorded share `ip`.
- Live tn10: bring up the fly edge in ≥2 regions, point a tn10 hostname
  at it, confirm the Goldshell mines through it and the origin logs the
  ASIC's real IP (not the fly egress).
- Cutover load/latency test before scheduling (T6 in the compat doc).

## Pros and Cons of the Options

### Runtime Option 1: N independent instances

- Good: smallest code change; reuses the entry point as-is.
- Bad: N duplicate block-template subscriptions to kaspad (N× template
  work); N anti-abuse guards fragment per-IP caps per-port; N
  ShareHandlers/vardiff threads.

### Runtime Option 2: shared pipeline + per-port seed (chosen)

- Good: one block pipeline; global per-IP anti-abuse/attribution; hot
  path reused verbatim; backward compatible.
- Bad: touches `StratumContext`, the listener, `ClientHandler`, and the
  server entry point. Mitigated by tests and the no-op-when-single
  default.

### Edge Option A: fly.io anycast (chosen)

- Good: any TCP port; anycast global routing; `proxy_proto` preserves
  client IP; matches the proven legacy edge; low cost.
- Bad: requires a forwarder process (HAProxy) on the fly machine and an
  egress trust model for PROXY.

### Edge Option B: self-managed micro-VPS forwarders

- Good: cheapest; full control.
- Bad: manual geo-DNS (no anycast); more hosts to operate/patch.

### Edge Option C: keep Railway (ADR-0005)

- Bad: cannot expose ports `1111–8888`; breaks zero-action. Rejected.

### Edge Option D: Cloudflare Spectrum

- Good: anycast L4.
- Bad: Enterprise-tier pricing; violates the low-cost driver. Rejected.

## More Information

- Supersedes the **stratum-edge** portion of
  [ADR-0005](0005-netcup-vps-railway-edge.md) (NetCup origin +
  observability decisions stand).
- [`cutover-stratum-compatibility.md`](../cutover-stratum-compatibility.md),
  [`cutover-plan.md`](../cutover-plan.md).
- fly.io services + `proxy_proto`:
  <https://fly.io/docs/networking/services/>;
  PROXY protocol spec:
  <https://www.haproxy.org/download/2.9/doc/proxy-protocol.txt>.
- Resolved (2026-06-02): fly→origin trust = nftables allowlist +
  PROXY-required (WireGuard deferred); `kas.katpool.com` resolves to the
  edge (uniform PROXY path).
