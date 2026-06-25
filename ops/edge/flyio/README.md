# katpool fly.io anycast stratum edge

Thin per-region TCP forwarder that fronts the NetCup origin for a
zero-action mainnet cutover (see
[ADR-0022](../../../docs/decisions/0022-multiport-stratum-and-flyio-anycast-edge.md)
and
[`cutover-stratum-compatibility.md`](../../../docs/cutover-stratum-compatibility.md)).

## What it does

- One fly.io app, one **dedicated anycast IPv4**, deployed to the 7
  legacy regions. fly anycast routes each miner to the nearest healthy
  machine.
- Each of the 8 legacy stratum ports (`1111`–`8888`) is a fly TCP
  service with the `proxy_proto` (v2) handler. fly prepends a PROXY v2
  header carrying the real miner IP+port.
- HAProxy (`haproxy.cfg`) consumes that header (`accept-proxy`) and
  re-emits it to the origin (`send-proxy-v2`) on the **same** port, so
  the origin's per-port difficulty seed stays correct and the bridge
  recovers the real miner IP for anti-abuse + share attribution.

```
miner ──TCP:7777──▶ fly anycast (nearest region)
                      └─ proxy_proto v2 ─▶ HAProxy :7777 (accept-proxy)
                                              └─ send-proxy-v2 ─▶ origin:7777
```

## Regions (legacy parity)

| Hostname | Region | fly code |
|---|---|---|
| `na-west.katpool.com` | California | `sjc` |
| `na-east.katpool.com` | Virginia | `iad` |
| `eu.katpool.com` | Germany | `fra` |
| `ap.katpool.com` | Singapore | `sin` |
| `hkg.katpool.com` | Hong Kong | `hkg` |
| `sa.katpool.com` | Brazil | `gru` |
| `au.katpool.com` | Australia | `syd` |

`kas.katpool.com` (origin name) and all `*.katpool.com` mirrors also
resolve to the anycast IP — every miner connection arrives PROXY-fronted
(uniform path; ADR-0022).

## Deploy

`bring-up.sh` orchestrates the steps below idempotently (skips anything already
present, prompts before each mutating step):

```bash
KATPOOL_ORIGIN_HOST=kas-origin.katpool.com ops/edge/flyio/bring-up.sh
```

Or run them by hand:

```bash
cd ops/edge/flyio

# 1. Create the app (no deploy yet).
fly apps create katpool-edge

# 2. Point the forwarder at the origin (a name that resolves to the
#    NetCup origin's REAL IP — never the anycast/public name, or you loop).
fly secrets set KATPOOL_ORIGIN_HOST=kas-origin.katpool.com

# 3. Dedicated anycast IPv4 (raw TCP fails on fly's shared IPv4) + v6.
fly ips allocate-v4
fly ips allocate-v6

# 4. Stable per-region egress IPs for the origin allowlist (one per region).
for r in sjc iad fra sin hkg gru syd; do fly ips allocate-egress -r "$r"; done
fly ips list   # record the egress IPs -> origin nftables allowlist (below)

# 5. Deploy and spread across regions.
fly deploy
fly scale count 7 --region sjc,iad,fra,sin,hkg,gru,syd

# 6. Verify the anycast IP and per-region machines.
fly ips list
fly status
```

## Origin firewall (nftables) — REQUIRED

The origin must accept the stratum ports **only** from the fly egress
IPs, and the bridge must require PROXY v2 there
(`KATPOOL_STRATUM_PROXY_PROTOCOL=true`). Without this, anyone could open
the origin ports and spoof a PROXY header to forge a client IP.

The ruleset lives in [`nftables/katpool-stratum.nft`](nftables/katpool-stratum.nft)
(ships with RFC 5737/3849 documentation IPs as placeholders, so it is
syntactically valid but matches no real host). Fill in the real egress IPs and
apply it with the helper, which pulls them from `fly ips list`, validates with
`nft -c`, installs to `/etc/nftables.d/`, and loads them:

```bash
# Auto-collect egress IPs from `fly ips list` and apply (run on the origin):
sudo ops/edge/flyio/nftables/apply-origin-firewall.sh

# Or pass them explicitly (e.g. from step 4 above):
sudo ops/edge/flyio/nftables/apply-origin-firewall.sh 1.2.3.4 5.6.7.8 2a09:0:1::1

# Validate-only (no changes), or preview the rendered ruleset:
ops/edge/flyio/nftables/apply-origin-firewall.sh --check 1.2.3.4
ops/edge/flyio/nftables/apply-origin-firewall.sh --print 1.2.3.4
```

The ruleset touches **only** the stratum ports (chain policy is `accept`, so
SSH/API/kaspad are untouched) and fast-paths established connections so a
reload never severs an in-flight miner. Re-run the script after any
`fly ips allocate-egress` change.

`stratum_ports` lists the pool's **origin** listen ports. Under mainnet
co-residency the legacy pool still owns `1111-8888` on the shared host, so the
new pool binds the `21111-28888` alt band and the ruleset locks only that band —
legacy's `1111-8888` stays fully open. The fly edge presents the canonical
public `1111-8888` and forwards `1111->21111 … 8888->28888`
(see [`haproxy.cfg`](haproxy.cfg)). Once the legacy pool is decommissioned the
new pool can move back to `1111-8888` and `stratum_ports` should follow.

### Legacy MiningPoolStats API (`:8080`)

miningpoolstats.stream polls `http://kas.katpool.xyz:8080/api/pool/miningPoolStats`.
Miners also use `kas.katpool.xyz` on the **same anycast IP** for stratum, so DNS
must **not** be repointed at the origin. Instead:

1. **fly edge** — `fly.toml` exposes TCP `:8080` (no `proxy_proto`; HTTP clients
   do not send PROXY headers). `haproxy.cfg` forwards plain TCP to
   `origin:8080`.
2. **origin nginx** — `ops/nginx/kas.katpool.xyz-legacy-api.conf` proxies
   `/api/pool/miningPoolStats` to the loopback pool API (`:18081`).

Port `8080` is **not** in the nftables `stratum_ports` set (input policy is
`accept`), so fly egress can reach origin nginx without a firewall change.
After editing `haproxy.cfg` / `fly.toml`, redeploy the edge:

```bash
cd ops/edge/flyio && fly deploy -a katpool-edge
```

Persist across reboots with the committed oneshot unit (preferred over an
`include` in the distro `nftables.conf`, whose `flush ruleset` would wipe
Docker's tables — the unit reloads only the self-contained `inet katpool`
table):

```bash
sudo install -m 0644 ops/edge/flyio/nftables/katpool-origin-firewall.service \
  /etc/systemd/system/katpool-origin-firewall.service
sudo systemctl daemon-reload
sudo systemctl enable --now katpool-origin-firewall.service
```

## DNS

Point every hostname's `A`/`AAAA` at the anycast IPs from `fly ips list`:

- `.xyz`: `kas`, `na-west`, `na-east`, `eu`, `ap`, `hkg`, `sa`, `au`.
- `.com`: the same set, mirrored (new in the rebuild; backward-compat).
- `kas-origin.katpool.com` ⇒ the NetCup origin's real IP (forwarder
  target only; not advertised to miners).

## Validation

1. `nc -vz <anycast-ip> 7777` from several geos — connect succeeds.
2. Point a tn10 hostname at a 2-region edge, mine with the Goldshell, and
   confirm on the origin that the logged client IP is the **ASIC's** IP
   (not a fly egress IP), and the `mining.set_difficulty` seed matches the
   port table.
3. Kill the nearest region; confirm anycast fails the miner over to the
   next region with reconnect only.

## Load + failover test (pre-mainnet gate)

Run before advertising the edge to real miners. Targets reflect legacy peak
(~5–10k concurrent stratum sockets across ports):

1. **Connection ramp** — open concurrent TCP connections to the anycast IP on a
   live stratum port and hold them, ramping to ≥10k:
   `for i in $(seq 1 10000); do nc -w0 <anycast-ip> 7777 & done` (or a proper
   load tool). Confirm: no connection refusals, HAProxy `maxconn` (200000) not
   approached, and origin `ks_anti_abuse_connection_reject_total` does not spike
   (the real miner IPs arrive via PROXY v2, so the per-IP guard sees distinct
   clients, not the fly egress IP).
2. **Share latency** — with a real ASIC mining through the edge, compare
   `katpool:share_accept_latency:p99_5m` to a direct-to-origin baseline; the
   added edge hop should be a few ms, not tens.
3. **Sustained soak** — keep the ASIC mining through the edge ≥1h; confirm
   blocks are still found+accepted and `CanaryMinerNotPaid` stays green.
4. **Failover** — `fly scale count` down the nearest region (or stop its
   machine) mid-mine; confirm miners reconnect to the next region within
   seconds and no shares are lost beyond the reconnect gap.
5. **Origin firewall** — from a non-fly host, confirm the stratum ports are
   refused (only fly egress IPs are allowlisted), while SSH/API/kaspad are
   unaffected.

See ADR-0022 "Confirmation" for the full acceptance list.
