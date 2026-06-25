---
status: accepted
date: 2026-05-25
deciders: argonmining
---

# ADR-0004: Self-host the observability stack on Railway

## Context and Problem Statement

The legacy pool ships logs to Datadog (paid SaaS), uses Telegram as
the only alerting channel, and has no distributed tracing. The
Datadog dependency is both a recurring cost and a vendor lock-in.
We can replace it with a self-hosted Grafana LGTM (Loki, Grafana,
Tempo, Mimir/Prometheus) stack on the same platform we already use
for other Nacho-the-Kat services (Railway).

## Decision Drivers

- Minimise recurring vendor costs (project-wide constraint)
- Avoid vendor lock-in for telemetry (we own the data)
- Long-term maintainability — pick a stack the operator already
  knows from other Railway projects
- Decouple observability from the pool's own failure domain (a pool
  VPS outage must not take out the monitoring that diagnoses it)
- Standard open-source primitives so any operator can step in

## Considered Options

1. **Self-host Grafana LGTM + supporting OSS on Railway** —
   Grafana, Loki, Tempo, Prometheus/VictoriaMetrics, Alertmanager,
   Blackbox Exporter, GlitchTip (Sentry-compatible), Uptime Kuma,
   ntfy, plus a canary miner.
2. **Continue with Datadog + Telegram.**
3. **Self-host on the same NetCup VPS as the pool.** Removes
   Railway cost, but couples observability failure domain to pool
   failure domain.
4. **Use Grafana Cloud free tier.** Vendor dependency we can live
   without; free tier has limits.

## Decision Outcome

**Chosen option: 1.** Self-hosted LGTM on Railway. Strikes the right
balance of cost (~$30–40/month), failure-domain separation,
operator familiarity, and full data ownership.

Stack:

- **Grafana** — dashboards, queries
- **Loki** — log aggregation
- **Tempo** — distributed tracing (OpenTelemetry OTLP)
- **VictoriaMetrics** — metrics storage (the existing pool already
  has vmagent + VM, simplifies migration)
- **Alertmanager** — alert routing
- **Blackbox Exporter** — synthetic probes (HTTP + TCP)
- **GlitchTip** — Sentry-compatible error tracking
- **Uptime Kuma** — uptime + public status page front-end
- **ntfy.sh (self-hosted)** — push notifications
- **Canary miner** — small Rust binary submitting real shares to
  our pool from outside our VPS

Cross-service traffic within a Railway project is free, which
suits an observability project full of internal scrapes and pushes.

### Consequences

- Positive: no monthly Datadog bill (~$50–200 saved depending on
  log retention)
- Positive: we own all telemetry data; can export, retain, or
  delete at will
- Positive: operator already manages Railway services for other
  projects — operational consistency
- Positive: observability stack on a different host than the pool —
  pool outage doesn't blind us
- Negative: we run ~9 additional services. Mitigation: pinned
  digests, declarative `railway.toml`, restorable from git
- Negative: Railway costs ~$30–40/month for the observability
  project (still net cheaper than Datadog)

### Confirmation

- `ops/railway/observability/railway.toml` defines the project
- Every alert in [`alertmanager/`](../../ops/railway/observability/alertmanager/)
  links to a runbook URL in `docs/runbooks/`
- Synthetic canary miner runs and the `CanaryMinerNotPaid` alert
  fires correctly when its shares are stopped (verified in Phase 9)

## Pros and Cons of the Options

### Option 1: Self-host LGTM on Railway

- Good: data ownership, no vendor lock-in
- Good: failure-domain separation from the pool
- Good: ~$30–40/month is well within budget
- Good: standard OSS — abundant documentation and community support
- Bad: more moving parts than a hosted SaaS

### Option 2: Continue Datadog + Telegram

- Bad: recurring cost
- Bad: vendor lock-in for logs and APM
- Bad: no traces today
- Rejected

### Option 3: Co-host observability on the pool VPS

- Good: zero Railway cost
- Bad: a pool VPS outage takes out the monitoring that should be
  paging us about it
- Bad: violates the failure-domain-separation driver
- Rejected

### Option 4: Grafana Cloud free tier

- Good: hosted Grafana saves Railway hosting
- Bad: free-tier retention and quotas force tradeoffs over time
- Bad: vendor dependency we can avoid

## More Information

- Grafana LGTM: <https://grafana.com/docs/loki/latest/>, etc.
- GlitchTip (Sentry-compatible OSS): <https://glitchtip.com/>
- Uptime Kuma: <https://github.com/louislam/uptime-kuma>
- Companion ADRs: [0005 (Railway + NetCup)](0005-netcup-vps-railway-edge.md)
