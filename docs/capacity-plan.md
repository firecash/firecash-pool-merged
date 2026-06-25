# Capacity Plan

Measured numbers for the NetCup production VPS, with budgets and
headroom analysis. Refined during Phase 9's capacity load test; the
section labelled "Phase 9 measurement" gets filled in when the load
test runs.

## 1. Production VPS (as of 2026-05-25)

Captured from `lscpu`, `free -h`, `df -h` on the production host.

| Resource | Value |
|---|---|
| CPU | 20 vCPU exposed (AMD EPYC 9634 84-core, virtualised, 2.0 GHz baseline) |
| RAM | 94 GiB |
| Swap | 0 B (disabled — required for treasury custody hardening) |
| Root disk | 3.0 TB SSD (`/dev/vda3`), 1.4 TB used, 1.5 TB free (48% used) at audit time |
| Network | NetCup-class (saturating gigabit available; specific bandwidth allowance per the NetCup plan) |
| OS | Debian-derived Linux; init: containerised host detected during audit |

## 2. Budgets for the new pool

Each component's expected steady-state allocation on the same VPS.
These are budgets, not measurements; measurements land in §4.

| Component | RAM | vCPU | Disk |
|---|---|---|---|
| Embedded `kaspad` (mainnet, future) | 4–6 GiB | 2–4 | ~50–100 GiB (consensus state + UTXO index + pruned chain) |
| `katpool` pool process (bridge + accountant + payout-kas + payout-krc20 + api) | ~500 MB | 1–2 | < 1 GiB binary + caches |
| PostgreSQL 17 | 4 GiB shared_buffers + work_mem | 2 | 100 GiB initially (today's pool DB is far less); grows with `block_details` and share history |
| `vmagent` + log shipping | 200 MB | 0.2 | minimal |
| nginx + acme.sh + helpers | 100 MB | < 0.1 | minimal |
| pgBackRest workspace | 500 MB | < 0.1 | up to 200 GiB local WAL spool |
| `katpool-kaspad-tn10` (Phase 1 acceptance, see [ADR-0010](decisions/0010-multi-tenant-kaspad-on-pool-vps.md)) | ~5 GiB | 1–2 | ~30 GiB (testnet-10 chain is much smaller than mainnet) |
| **Total** | **~15–17 GiB** | **~7–10** | **~230 GiB** |
| **Available** | 94 GiB | 20 | 1500 GiB free |
| **Headroom** | **~6×** | **~2.5×** | **~6×** |

During the build period (Phase 1–6) the **legacy production stack
remains running** in addition to the budgets above: the existing
dockerized mainnet kaspad (~10 GiB RAM, 128 GB live data dir),
katpool-app (legacy stratum), katpool-monitor, katpool-payment,
katpool-db, victoria-metrics, prometheus, nginx. Empirically that
adds ~14 GiB RAM and ~150 GB disk. Combined load with both stacks
co-resident:

- RAM: ~30 GiB used of 94 GiB → 32% used
- Disk: ~370 GiB used of 3 TB → 12% used
- CPU: well below 50% utilisation in measured production

Cutover (Phase 7) decommissions the legacy stack and reclaims its
footprint; capacity will drop back to the ~16 GiB / ~230 GB profile
shown above.

Headroom is intentional. We can comfortably run a long-lived **shadow
pool** instance alongside production for the entire build period (not
just the final 72 h pre-cutover), with separate DB, observability,
and embedded kaspad data dir.

## 3. Load model (assumed; validated in Phase 9)

These numbers are estimated from the legacy pool's recent operating
data. Phase 9 will confirm.

| Variable | Estimate |
|---|---|
| Concurrent stratum sessions at peak | 100–300 |
| Average shares / sec at peak | 50–200 |
| Block-template fanout rate | one event per accepted block (~10/s during steady-state Kaspa) |
| Block-submission rate from pool | ~20 / hour at recent network hashrate |
| Payment recipients per cycle | 25–60 |
| NACHO rebate recipients per cycle | 25–60 |
| Postgres write rate | ~1000 rows/s peak (shares + share_windows) |
| Public API queries | low; cache hides most |

Phase 9 target: sustain **5×** these numbers for 24 h with no memory
leaks, no share rejection regression, and headroom remaining on every
budget above.

## 4. Phase 9 measurement (to be populated)

The first iteration of this section is empty by design. The Phase 9
load test in [phase-9 of the plan](../) records measured numbers here
along with the load harness configuration that produced them.

| Date | Sessions | Shares/s | RAM used (pool) | RAM used (kaspad) | RAM used (pg) | CPU % | Notes |
|---|---|---|---|---|---|---|---|
| _TBD_ | | | | | | | |

## 5. Sizing triggers (when to scale)

Sustained crossing of any of these thresholds, for > 24 h, triggers a
review:

| Metric | Threshold | Action |
|---|---|---|
| RAM usage | > 70% of 94 GiB | Investigate; if not a leak, plan a VPS plan bump |
| CPU usage | > 70% of 20 vCPU | Investigate; profile hot paths |
| Disk usage | > 80% of 3 TB | Investigate kaspad retention; consider archive node split |
| pg write latency | > 50 ms p99 | Investigate; consider read replicas |
| Backup archive lag | > 5 min | Pager-level alert |
| Stratum p99 latency | > 200 ms | Investigate Railway region health |

## 6. Co-location boundaries

We deliberately split the following onto Railway (separate failure
domain from the pool VPS), accepting the small Railway monthly cost:

- Observability stack (Grafana, Loki, Tempo, Alertmanager, Blackbox,
  GlitchTip, Uptime Kuma, ntfy). A pool outage must not take down
  the monitoring that diagnoses it.
- Edge TCP proxies (us-east, eu-west, ap-southeast). Reduces miner
  reconnect storms when a single Railway region or our NetCup VPS
  briefly fails.
- DR validator. Restoring a backup on the same VPS that holds the
  primary defeats the purpose of the drill.

Everything else (pool process, postgres, vmagent, pgBackRest agent,
nginx) runs co-located on the NetCup VPS.
