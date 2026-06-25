# katpool SLOs, retention, and escalation (B6)

This is the contract the alerts in `victoria-metrics/rules/` are derived from.
Thresholds in the rule files should always trace back to a line here.

## Service-level objectives

| SLO | SLI (recorded series) | Objective | Window |
|---|---|---|---|
| **API availability** | `katpool:api_availability:ratio5m` (fraction of `/ready` blackbox probes that succeed) | ≥ 99.9% | 30 days |
| **Share quality** | `katpool:share_accept_ratio:rate5m` (valid / total shares) | ≥ 95% | 7 days |
| **Block confirm rate** | `katpool:block_confirm_rate:ratio1h` (node-accepted blocks that confirm blue) | ≥ 95% | 7 days |
| **Accounting integrity** | `ks_accountant_events_lagged_total` increase + `ks_accountant_event_errors_total` rate | 0 dropped/errored events | continuous |

Alert thresholds are intentionally looser than the SLO target (e.g. invalid-share
ratio pages at >10% while the SLO is 5%) so a page means the budget is being
actively burned, not merely touched.

## Payout & treasury metrics (B7)

The KAS/KRC-20 payout engines and the consolidation engine emit these via
`katpool-metrics` (on the global registry the exporter gathers), each carrying an
`instance` label so the exporter's instance filter keeps them:

- `ks_payout_cycles_total{instance, engine, status}` — one increment per leader
  tick, by engine (`kas`/`krc20`) and terminal `PayoutCycleStatus`
  (`settled` / `partially_settled` / `broadcasting` / `planned` / `failed`, plus
  `error` for a failed tick). `PayoutCycleFailing` pages on a `failed`/`error`
  increase.
- `ks_payout_last_success_timestamp_seconds{instance, engine}` — last cycle that
  settled (fully or partially). For dashboards/stall detection; deliberately
  **not** paged on, to avoid false alarms on legitimately idle cycles (the canary
  miner is the end-to-end "are we actually paying" truth).
- `ks_treasury_balance_sompi{instance}` / `ks_treasury_spendable_utxos{instance}`
  — from the latest consolidation snapshot; `TreasuryBalanceLow` warns below an
  operator-tunable floor (see the rule). Absent if consolidation is disabled.

## Share-accept latency (B7)

The bridge emits `ks_share_accept_latency_seconds{instance}` (histogram, observed
on the accepted-share path in `bridge/src/share_handler.rs`), recorded as
`katpool:share_accept_latency:p99_5m`. Exposed for dashboards; **no alert yet** —
a latency objective has to be set here first (do not page on a guessed number).

## Canary (end-to-end credit probe)

`CanaryMinerNotPaid` depends on `canary_last_credited_timestamp_seconds`,
published by the **local canary tool** (`ops/canary/`): a real off-VPS miner
submits shares with a dedicated wallet, and a dependency-free watcher publishes
that wallet's last credited time to VictoriaMetrics via vmauth. The alert stays
inert until the canary is running. This is the ground-truth "are we actually
paying miners" SLI — it exercises the full accept → validate → account → credit
path from where a real miner sits.

## Known instrumentation gaps (do NOT alert on guessed metrics)

None outstanding from the B6/B7 SLI list — share-accept latency, payout/treasury
metrics, and the canary are all emitted. A share-accept *latency* SLO/alert is
intentionally deferred until an objective is set (see above); do not page on a
guessed threshold.

## Retention policy

| Signal | Store | Retention | Where set |
|---|---|---|---|
| Metrics | VictoriaMetrics | 90 days | `-retentionPeriod=90d` (README) |
| Logs | Loki | 30 days | `limits_config.retention_period: 720h` |
| Traces | Tempo | 14 days | `compactor.compaction.block_retention: 336h` |

Traces are sampled and short-lived (debugging aid); metrics are the long-term
record; logs sit in between. All three fit the ~$30–40/month ADR-0004 budget.

## Mainnet scaling levers

The retention windows above are correct for mainnet; what scales with load is
**ingest volume**, which grows with miner/worker count, not retention. The
defaults here are sized for the tn10 soak (a handful of miners). Before mainnet,
capture a 24h baseline of the three rates below, then size each lever to ~3× the
observed peak (headroom for growth) rather than guessing now:

| Lever | Where | Default (tn10) | Scales with | Note |
|---|---|---|---|---|
| Log ingest cap | `loki/loki-config.yaml` `limits_config.ingestion_rate_mb` / `ingestion_burst_size_mb` | 8 / 16 | log lines/s ≈ shares/s + workers | Loki 429s above the cap — raise it, or the origin Alloy WAL backs up |
| Trace volume | `origin/alloy.alloy` `tail_sampling … sampling_percentage` | 100 (keep all) | API req/s | Lower (e.g. 20) once API request volume is real; error+slow traces are always kept |
| Metrics storage | `deploy/victoriametrics/Dockerfile` `-retentionPeriod=90d` + the Railway volume size | 90d | active series ≈ workers × per-worker series | 90d is the window; size the VM volume for 90d × peak series |
| Loki/Tempo volume | their Railway volumes | — | log/trace bytes | grow the volumes with the ingest rates above |

Observe the baselines with: `sum(rate(ks_valid_share_counter[5m]))` (share/log
rate proxy), the API request-span rate in Tempo, and VictoriaMetrics
`/api/v1/status/tsdb` (active series). Re-tune the alert thresholds in
`rules/` against the same baseline (e.g. the `StratumAbuseBurst` 50/s figure).

## Escalation policy

Two severities, both routed to ntfy via Alertmanager:

- **`page`** — wake on-call now. `repeat_interval: 1h`, `group_wait: 10s`,
  ntfy `urgent` priority. Used for outages and money-path failures
  (exporter/API/stratum down, no shares, accountant errors/lag, canary unpaid).
- **`warning`** — handle next business hour. `repeat_interval: 4h`, ntfy `high`
  priority. Used for degradations (high invalid-share ratio, red-block ratio,
  abuse bursts, indexer dependency down).

A firing `page` inhibits the matching `warning` (one signal, not two). Every
alert links to a `docs/runbooks/` page; if an alert has no runbook, that is a
bug in the rule, not the runbook set.
