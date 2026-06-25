# Architecture

> Authoritative summary of the target architecture. Decisions captured
> here are codified in the [`decisions/`](decisions/) ADRs. The full
> phased delivery plan lives in the workspace plan file; this document
> is the steady-state picture.

## 1. Target topology

```mermaid
flowchart TB
  Miner[ASIC miners worldwide] -->|"stratum TCP per-port"| edge

  subgraph edge [Railway TCP edge - 3 regions]
    rwUS[us-east TCP proxy]
    rwEU[eu-west TCP proxy]
    rwAP[ap-southeast TCP proxy]
  end

  edge -->|"public internet"| netcup

  subgraph netcup [NetCup VPS - single production host]
    nginxLb[nginx 1.27 - TLS for API and metrics]
    subgraph poolProc [katpool Rust binary - unprivileged uid]
      bridge[stratum-bridge - forked from rusty-kaspa v1.1.0]
      kaspadEmbed["kaspad embedded - in-process gRPC"]
      eventBus[(tokio broadcast - ShareCredited/BlockFound/BlockAccepted)]
      accountant[pool-accountant - PROP allocation]
      payoutKas[payout-kas - daily, mass-aware, idempotent]
      payoutKrc20[payout-krc20 - native Rust commit/reveal]
      api[api - axum read-only]
      antiabuse[anti-abuse - rate limit, ban list, withholding detector]
      bridge --> eventBus
      bridge <-->|"gRPC loopback"| kaspadEmbed
      bridge --> antiabuse
      eventBus --> accountant
      accountant --> pg[(PostgreSQL 17)]
      payoutKas --> pg
      payoutKas --> kaspadEmbed
      payoutKrc20 --> pg
      payoutKrc20 --> kaspadEmbed
      api --> pg
    end
    pg
    backup[pgBackRest - WAL streaming + nightly base]
    vmagent[vmagent + alloy - local collectors]
    pg --> backup
    poolProc --> vmagent
  end

  backup -->|"S3-compatible, encrypted"| backupBucket["Backblaze B2 backup bucket"]

  subgraph railwayObs [Railway: self-hosted observability project]
    grafana[Grafana]
    loki[(Loki)]
    tempo[(Tempo)]
    metrics[(VictoriaMetrics)]
    alertmgr[Alertmanager]
    blackbox[Blackbox Exporter]
    glitchtip[GlitchTip]
    uptime[Uptime Kuma]
    ntfy[ntfy.sh self-host]
    drv[DR validator - weekly cron]
    canaryMiner[canary miner]
  end

  vmagent -->|"remote_write"| metrics
  vmagent -->|"loki push"| loki
  vmagent -->|"OTLP"| tempo
  poolProc -->|"OTLP traces + JSON logs"| vmagent
  alertmgr --> ntfy
  alertmgr -->|"Telegram bot"| telegram
  blackbox -->|"probe /health, stratum TCP"| nginxLb
  canaryMiner -->|"stratum"| edge
  drv -->|"weekly restore drill"| backupBucket
  glitchtip <-- poolProc

  externalKasplex["api.kasplex.org"] <-- payoutKrc20
  externalKaspa["api.kaspa.org / api.kaspa.com"] <-- accountant
```

## 2. Single-binary model

The pool runs as **one Rust binary** (the `katpool` crate) that links
the bridge, accountant, payout engines, and API into a single process.
This removes the four sources of failure that the legacy stack exhibited
in production:

- **No Redis IPC**: the bridge and accountant share state via a
  `tokio::sync::broadcast` channel inside the same process, eliminating
  the stale-IP class of bugs we hit in April and May 2026.
- **No WASM**: native Rust against the `kaspa-*` crates (gRPC, txscript,
  consensus-core) eliminates the recurring
  `Unreachable code should not be executed` WASM exceptions and the
  `Couldn't deserialize u64` block-submission failure.
- **No Puppeteer**: the NACHO floor-price quote is a plain HTTPS GET with
  a circuit breaker.
- **No Bun**: pure Rust toolchain end-to-end; reproducible builds, signed
  releases.

The single-binary model is justified by capacity: the NetCup VPS (20 vCPU
EPYC 9634, 94 GB RAM, 3 TB SSD) has roughly 10× headroom over the
expected load. See [`capacity-plan.md`](capacity-plan.md) for measured
numbers.

## 3. Module responsibilities

| Module | Owns | Phase |
|---|---|---|
| `bridge/` | Stratum TCP, PoW verification, block templates, block submission. Forked from `rusty-kaspa` v1.1.0 commit `e97070f`; intrusive patches are the broadcast-channel hook and the anti-abuse hooks. | 1 |
| `crates/katpool-domain/` | Core types (`WalletAddress`, `Sompi`, `NachoUnits`, `ShareId`, `BlockTemplate`, `IdempotencyKey`). Pure types only — no I/O, no async. | 1 |
| `crates/katpool-storagemass/` | KIP-9 + KIP-13 mass calculator. Pure functions. Fuzz + property tested. | 4 |
| `crates/katpool-db/` | sqlx-managed schema, migrations, repository traits, distributed-lock primitive. | 2 |
| `crates/katpool-idempotency/` | Idempotency keys and dist-locks layered on `katpool-db`. | 2 |
| `crates/katpool-secrets/` | Sops/age decryption, `mlock`, `zeroize`, no `Debug` for key bytes. | 4 |
| `crates/katpool-telemetry/` | `tracing-subscriber` + OpenTelemetry OTLP wiring, correlation-id propagation. | 1 |
| `crates/katpool-metrics/` | Prometheus registry helpers with bounded-cardinality discipline. | 1 |
| `crates/katpool-config/` | Strongly-typed config loader; validates at boot or aborts. | 3 |
| `crates/katpool-fault-injection/` | Test-only chaos primitives (latency injection, RPC error sim). | 9 |
| `accountant/` | Subscribes to `PoolEvent` broadcast; deterministic PROP allocation. | 3 |
| `payout-kas/` | Daily KAS payout cycle; mass-aware batcher; idempotent; treasury-key holder. | 4 |
| `payout-krc20/` | Native Rust KRC-20 commit/reveal for NACHO rebate. | 5 |
| `api/` | Read-only HTTP API; rate-limited; TLS via nginx. | 6 |
| `katpool/` | Main binary: composes the modules, handles graceful shutdown. | 0+ |

## 4. Data flow

### 4.1 Share to balance

1. ASIC connects to Railway TCP proxy (port 1111 / 3333 / 5555 / etc).
2. Railway proxies to NetCup, terminating no protocol — raw TCP.
3. `bridge` accepts, parses stratum, validates address, applies vardiff.
4. On `mining.submit`: PoW verified against the cached job's target.
5. Accepted share is published on the in-process broadcast channel as
   `PoolEvent::ShareCredited { wallet, worker, difficulty, daa_score, ts,
   correlation_id }`.
6. `accountant` consumes the event, records it in `share_windows`.

### 4.2 Block found to allocation

1. `bridge::submit_block` constructs the candidate block and invokes
   `kaspad`'s `submit_block` gRPC.
2. On `submit_block` success: emit
   `PoolEvent::BlockFound { wallet, worker, hash, daa_score, template_id, ts }`.
3. Embedded `kaspad` emits `block-added` for our treasury address.
4. After coinbase maturity (1000 blocks on mainnet), `kaspad` reports
   the matured coinbase via `UtxoProcessor`.
5. `accountant` receives the maturity, emits
   `PoolEvent::BlockAccepted { hash, miner_reward_sompi, ts }`.
6. PROP allocation runs: `miner_reward * 9925/10000` distributed among
   shares with `daa_score <= block.daa_score`. Remainder is pool fee.
7. Pool fee split 33/67 into `nacho_rebate_kas` accrual and pool
   revenue (`miner_id = 'pool'`).
8. All balance mutations carry idempotency keys; replays are no-ops.

### 4.3 KAS payout cycle (daily)

1. Cron schedule (configured in `katpool.toml`, default `0 5 * * *`)
   wakes `payout-kas` on a single-instance distributed-lock-guarded
   schedule.
2. Eligible miners: `balance >= thresholdAmount` (5 KAS).
3. Storage-mass-aware batcher plans transactions so every batch
   satisfies `max(compute_mass, storage_mass, transient_storage_mass) <=
   block_mass_limit`.
4. Each planned transaction records an `idempotency_key` BEFORE signing.
5. Sign with treasury key (loaded via systemd
   `LoadCredentialEncrypted=`, mlocked, zeroized after use).
6. Submit via embedded kaspad's gRPC.
7. On confirmation: record `payments` row, zero `miners_balance.balance`
   for paid recipients.

### 4.4 NACHO rebate cycle (daily, separate cron)

1. Read accrued `nacho_rebate_kas` for each miner.
2. Fetch NACHO floor price from `api.kaspa.com/api/floor-price` with a
   circuit breaker; cache for cycle duration.
3. For each eligible recipient: build KRC-20 envelope, plan
   commit-and-reveal pair, run through `katpool-storagemass` batcher.
4. Same idempotency discipline as KAS payouts.
5. *Legacy-descriptive (superseded by [ADR-0016]).* The legacy pool
   applied a 3× "full rebate" multiplier at payout time. The new pool
   applies the tier rebate (standard 33% / elite 100%) **at allocation
   time** per matured block (ADR-0012), stored in
   `nacho_rebate_accrual.accrued_sompi`; payout converts that pending
   sompi to NACHO at the floor price with **no** further multiplier.

[ADR-0016]: decisions/0016-krc20-payout-conversion-and-floor-price.md

## 5. Observability

All telemetry is self-hosted on Railway in a separate project
(`katpool-observability`). The pool VPS pushes:

- **Logs**: structured JSON via `tracing-subscriber` → `vmagent` →
  Loki on Railway. Correlation IDs propagate across async boundaries.
- **Metrics**: Prometheus scrape on `localhost:9999` → `vmagent`
  remote_write → VictoriaMetrics on Railway.
- **Traces**: OpenTelemetry OTLP → `vmagent` → Tempo on Railway.
- **Errors**: Sentry-compatible payloads → GlitchTip on Railway.
- **Synthetic probes**: Blackbox Exporter on Railway → katpool API,
  stratum TCP, kaspad peer count.
- **Canary miner**: micro-binary on Railway submits real shares
  every 30s; alert if a share isn't credited within 5 min.

Alerting (Alertmanager) routes to ntfy (push) and Telegram. Every
alert rule embeds a runbook URL.

## 6. Deployment

- **Pool VPS**: NetCup, owned and managed by the operator. The pool
  binary runs as a systemd unit with intrusive hardening
  (`NoNewPrivileges`, `ProtectSystem=strict`, `SystemCallFilter`,
  unprivileged uid). See [`custody.md`](custody.md).
- **Railway TCP edge**: three minimal-resource services in
  us-east, eu-west, ap-southeast. CNAME records on
  `kas.katpool.com`, `kas-eu.katpool.com`, `kas-ap.katpool.com`.
- **Railway observability**: one project hosting the LGTM stack and
  the canary miner. Cross-service traffic is free within Railway.
- **Backups**: Backblaze B2, pgBackRest streaming WAL, weekly
  automated DR-restore validation.

## 7. Failure-domain map

| Failure | Detection | Mitigation | Recovery |
|---|---|---|---|
| Pool process crash | Liveness probe + systemd | Auto-restart; idempotency keys prevent double-pay on resume | Seconds |
| PostgreSQL corruption | pgBackRest health + slow queries | PITR from B2 | < 1h |
| NetCup VPS hardware | Probe failure across all services | Restore B2 backup on new VPS | < 4h |
| Treasury key compromise | n/a (no in-band detection) | OS-level isolation, fail2ban, signed audits | Out-of-band rotation per runbook 11 |
| Railway edge region down | Blackbox + miner reconnect telemetry | DNS multi-region; miners reconnect | Minutes |
| kaspad lost peers | `kaspad_peer_count < 5` alert | Manual peer-list refresh | Minutes |
| External API outage (kasplex) | Circuit breaker telemetry | Graceful cycle skip with alert | Next cycle |

## 8. Concrete out-of-scope

- Multi-VPS active/active HA. We have warm-restore DR instead — the
  cost/benefit doesn't justify a second pool host at our scale.
- Stratum V2 protocol. Kaspad upstream is still V1; revisit when
  upstream lands V2 support.
- Hardware-wallet-backed treasury custody. Not feasible for
  automated daily payouts on Kaspa as of May 2026.
- Direct-to-BTC payouts. Out of pool scope.

## 9. Reading order

If you are new to the project:

1. This file
2. [`onboarding.md`](onboarding.md) — get a dev environment running
3. [`threat-model.md`](threat-model.md) — what we worry about
4. [`custody.md`](custody.md) — how the treasury key is handled
5. [`kips.md`](kips.md) — the on-chain constraints that shape payouts
6. [`decisions/`](decisions/) — every architecturally-significant
   choice with rationale
7. [`runbooks/`](runbooks/) — incident response
