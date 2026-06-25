# katpool observability stack (Railway LGTM) — provisioning guide

Self-hosted Grafana **LGTM** stack per
[ADR-0004](../../../docs/decisions/0004-self-host-observability.md): replace
Datadog/Telegram with Grafana + Loki + Tempo + VictoriaMetrics + Alertmanager +
Blackbox + GlitchTip + Uptime Kuma + ntfy, on Railway, in a **separate failure
domain** from the pool (a pool-VPS outage must not blind the monitoring).

## What is in this repo vs. what you do in Railway

This directory is the **config-as-code** for the stack. The component subfolders
(`victoria-metrics/`, `loki/`, `tempo/`, …) hold the raw configs each service
consumes; **[`deploy/`](deploy/README.md)** turns them into deployable Railway
services — a thin per-service Dockerfile (image pinned by digest, config baked
in) plus a `railway.toml`. Railway image services can't mount repo files, so the
config is baked at build; this also makes the testnet stand-up and the later
mainnet rebuild reproducible rather than click-ops.

> Railway config-as-code is *per-service* (each service sets its own Root
> Directory + Config File). Every service here shares **Root Directory =
> `ops/railway/observability`** and sets **Config File =
> `deploy/<service>/railway.toml`**. See [`deploy/README.md`](deploy/README.md)
> for the image pins, settings matrix, secrets, and provisioning order.

## Services

Internal traffic uses Railway private DNS (`<service>.railway.internal`) and is
free. Only Grafana, the ntfy server, and **vmauth** (the remote-write ingress)
need public domains; VictoriaMetrics itself stays private and unauthenticated on
the internal network. Each image is **pinned by digest** in `deploy/` (resolve
the current stable tag, then pin `@sha256:…` — same discipline as the CI
actions); the resolved pins are tabulated in [`deploy/README.md`](deploy/README.md).

| Service | Image (pin digest) | Internal port | Volume | Config from this repo |
|---|---|---|---|---|
| VictoriaMetrics | `victoriametrics/victoria-metrics` | 8428 | `/victoria-metrics-data` | `victoria-metrics/scrape.yml` |
| vmauth | `victoriametrics/vmauth` | 8427 | — | `deploy/vmauth/auth.yml` |
| vmalert | `victoriametrics/vmalert` | 8880 | — | `victoria-metrics/rules/*.yml` |
| Grafana | `grafana/grafana` | 3000 | `/var/lib/grafana` | `grafana/provisioning/**`, `grafana/dashboards/**` |
| Loki | `grafana/loki` | 3100 | `/loki` | `loki/loki-config.yaml` |
| Tempo | `grafana/tempo` | 3200 / 4317 | `/var/tempo` | `tempo/tempo.yaml` |
| Alertmanager | `prom/alertmanager` | 9093 | `/alertmanager` | `alertmanager/alertmanager.yml` |
| Blackbox exporter | `prom/blackbox-exporter` | 9115 | — | `blackbox/blackbox.yml` |
| ntfy | `binwiederhier/ntfy` | 80 | `/var/cache/ntfy` | (operator; holds token) |
| ntfy-alertmanager | `xenrox/ntfy-alertmanager` (Docker Hub) | 8080 | — | `deploy/ntfy-alertmanager/` (rendered from env) |
| Uptime Kuma | `louislam/uptime-kuma:1` | 3001 | `/app/data` | (operator; UI-configured) |
| GlitchTip | `glitchtip/glitchtip` (+ Postgres + Redis) | 8000 | DB volume | (operator; needs DB+broker) |
| **canary-miner** | *(deferred — see below)* | — | — | — |

## Metrics flow (B4)

The pool's `/metrics` binds **loopback** on mainnet
(`KATPOOL_PROM_PORT=127.0.0.1:9302`) and is instance-filtered, so it cannot be
scraped across the network. Therefore:

- **Origin → VM (pull-local, push-remote):** run **vmagent on the pool VPS** with
  `victoria-metrics/origin-vmagent.yml`; it scrapes `127.0.0.1:9302` and
  `-remoteWrite.url`s into the **vmauth** public domain on Railway
  (`https://<vmauth-domain>/api/v1/write`), which basic-auths the request and
  proxies it to the private VictoriaMetrics service. Creds come from the host
  EnvironmentFile (`VMAUTH_WRITE_USER`/`VMAUTH_WRITE_PASSWORD`).
- **Railway-side scrape:** VictoriaMetrics runs `scrape.yml` for its own metrics
  and the **Blackbox** synthetic probes of the pool's *public* surface
  (`/health` `/ready` `/started`, stratum TCP, indexer health).

Run VictoriaMetrics with `-promscrape.config=/etc/vm/scrape.yml
-retentionPeriod=90d`. Set the `%{KATPOOL_API_HOST}`, `%{KATPOOL_STRATUM_HOST}`,
and `%{KATPOOL_STRATUM_PORT}` env placeholders on the VM service.

Run vmalert with:

```
vmalert \
  -rule=/etc/vmalert/rules \
  -datasource.url=http://victoriametrics.railway.internal:8428 \
  -remoteWrite.url=http://victoriametrics.railway.internal:8428 \
  -remoteRead.url=http://victoriametrics.railway.internal:8428 \
  -notifier.url=http://alertmanager.railway.internal:9093 \
  -evaluationInterval=30s
```

## Logs (B4)

The unified runtime emits structured JSON when `KATPOOL_LOG_FORMAT=json`
(katpool-telemetry). Ship those journald logs from the origin to Loki with a
shipper (Promtail/Grafana Alloy/vector — operator choice) targeting
`http://loki.railway.internal:3100`. Loki retains 30 days and can run log-based
alert rules (for payout/treasury lines that have no metric yet).

## Traces (B4)

Set `KATPOOL_OTLP_ENDPOINT` on the pool to the Tempo distributor
(`http://tempo.railway.internal:4317`, OTLP/gRPC). Off by default until the
stack exists.

## Paging (ntfy) (B5)

Alertmanager → **ntfy-alertmanager bridge** → ntfy. The bridge
(<https://git.xenrox.net/~xenrox/ntfy-alertmanager>) maps the alert `severity`
label to ntfy priority/topic and holds the **ntfy token** — so its config is a
**secret**, kept in Railway service variables, not in this repo. Alertmanager
posts to the bridge with a webhook password read from a mounted file
(`/etc/alertmanager/secrets/webhook_password`); set the same credentials on the
bridge. See `alertmanager/alertmanager.yml`.

## Status page

Uptime Kuma provides the public status page and an independent (second-source)
uptime check; point it at the same public `/health` endpoint and add an ntfy
notification. GlitchTip (Sentry-compatible) receives application error events.

## Canary miner (deferred)

ADR-0004 calls for a small external miner submitting **real shares from outside
the VPS**, exporting `canary_last_credited_timestamp_seconds`; the
`CanaryMinerNotPaid` page (already in `katpool-alerts.yml`) fires when that
end-to-end accept→credit path breaks. The canary **binary** is a separate
deliverable — the alert is in place and inert until the metric exists.

## Provisioning checklist

The image pins, per-service settings (Root Directory, Config File, volume mount,
public domains, `RAILWAY_RUN_UID`, Singapore region), the variable/secret matrix,
and the dependency-ordered provisioning sequence all live in
**[`deploy/README.md`](deploy/README.md)**. In short:

1. For each service, create it on the pinned image's Dockerfile (`deploy/<svc>/`),
   set Root Directory + Config File, attach the volume, and set its variables
   (secrets as service variables; cross-service values as reference variables).
2. Attach the operator-provided public domains to `grafana`, `ntfy`, and `vmauth`.
3. Install vmagent on the pool VPS with `origin-vmagent.yml` + `VMAUTH_WRITE_*`
   creds, pointing at the vmauth domain.
4. Set `KATPOOL_LOG_FORMAT=json`, deploy a log shipper to Loki, set
   `KATPOOL_OTLP_ENDPOINT` for traces.
5. Create the ntfy token (post-deploy) and set it on the bridge.
6. Verify: Grafana shows the **katpool — Pool Overview** dashboard with live
   data; stop the canary (once built) and confirm `CanaryMinerNotPaid` fires.
