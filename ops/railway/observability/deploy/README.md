# Deployable artifacts for the Railway LGTM stack (B3)

Each service deploys from a **thin Dockerfile** that pins its upstream image by
digest and bakes in the config from this repo (Railway image services cannot
mount repo files). This makes the testnet stand-up — and the later mainnet
rebuild — a reproducible `git`-tracked deploy rather than dashboard click-ops.

All Dockerfiles share **one build context**: `ops/railway/observability/`. In
each Railway service set **Root Directory = `ops/railway/observability`** and
**Config File = `deploy/<service>/railway.toml`**.

## Image pins (resolved from the registry — bump tag + digest together)

| Service | Image |
|---|---|
| victoriametrics | `victoriametrics/victoria-metrics:v1.145.0@sha256:c014fb5a711d38cb24fd0673197592cd1394bb903dbb16aea565620c9c8a3d70` |
| vmauth | `victoriametrics/vmauth:v1.145.0@sha256:c816c52c4d9187566c3dc23298dbd7c273352bfbacc811c353ff6e7f41921e10` |
| vmalert | `victoriametrics/vmalert:v1.145.0@sha256:5f241da926a531c11f870e7fec6dfa29fc7e34a0bd9ac573d2c9200dd9ab5f36` |
| grafana | `grafana/grafana:13.0.2@sha256:5dad0df181cb644a14e13617b913b261a54f7d4fd4510721dba420929f35bea2` |
| loki | `grafana/loki:3.7.2@sha256:191d4fdfb7264f16989f0a57f320872620a5a7c2ceeec6229212c4190ec49b86` |
| tempo | `grafana/tempo:2.10.7@sha256:032b3acb51ed02c4b801473d54bb63e9e9f13738d215126d9843c30283794f4b` |
| alertmanager | `prom/alertmanager:v0.33.0@sha256:af26fbe4dd1886ac0efd7bd55cd9027da262e105b137a376522b7c14c3626e4a` |
| blackbox | `prom/blackbox-exporter:v0.28.0@sha256:e753ff9f3fc458d02cca5eddab5a77e1c175eee484a8925ac7d524f04366c2fc` |
| ntfy | `binwiederhier/ntfy:v2.24.0@sha256:f8a9b104313b87cc24ae4f775f39e6328205b57dff6ede3eaf098a91e5d79f59` |
| ntfy-alertmanager | `xenrox/ntfy-alertmanager:1.0.0@sha256:81788c7905774b7b0b2ed6833b2bc4826a90a42e4b738706edcedd5f489e7a73` |

> `ntfy-alertmanager` lives on **Docker Hub**, not GHCR.

## Per-service Railway settings

`numReplicas` is 1 everywhere (volume-backed services cannot use replicas).
Region: **Singapore (`asia-southeast1`)** for all. Internal traffic uses private
DNS (`<service>.railway.internal`); only the three "public" rows get a domain.

| Service | Listen | Volume (mount) | Public domain | `RAILWAY_RUN_UID=0` |
|---|---|---|---|---|
| victoriametrics | 8428 | `/victoria-metrics-data` | — | yes |
| vmauth | 8427 | — | **yes → :8427** (remote-write ingress) | — |
| vmalert | 8880 | — | — | — |
| grafana | 3000 | `/var/lib/grafana` | **yes → :3000** | yes |
| loki | 3100 | `/loki` | — | yes |
| tempo | 3200 / 4317 / 4318 | `/var/tempo` | — | yes |
| alertmanager | 9093 | `/alertmanager` | — | yes |
| blackbox | 9115 | — | — | — |
| ntfy | 80 | `/var/cache/ntfy` | **yes → :80** | yes |
| ntfy-alertmanager | 8080 | — | — | — |

`RAILWAY_RUN_UID=0` is required on volume-backed non-root images so they can
write the volume (Railway volumes reference).

## Variables & secrets

Secrets are **Railway service variables**, never committed. Cross-service values
use **reference variables** (`${{Service.VAR}}`) so a secret is defined once.

| Service | Variable | Value |
|---|---|---|
| victoriametrics | `KATPOOL_API_HOST` | `api-tn10.katpool.com` (Blackbox HTTP probe target) |
| victoriametrics | `KATPOOL_STRATUM_HOST` | stratum host/IP — tn10: `152.53.37.182` |
| victoriametrics | `KATPOOL_STRATUM_PORT` | stratum port — tn10: `15555` |
| vmauth | `VMAUTH_WRITE_USER` / `VMAUTH_WRITE_PASSWORD` | generated; also given to the origin vmagent |
| grafana | `GF_SECURITY_ADMIN_USER` | `admin` |
| grafana | `GF_SECURITY_ADMIN_PASSWORD` | generated secret |
| grafana | `GF_SERVER_ROOT_URL` / `GF_SERVER_DOMAIN` | the Grafana public domain |
| alertmanager | `ALERTMANAGER_WEBHOOK_PASSWORD` | generated secret |
| ntfy | `NTFY_BASE_URL` | the ntfy public domain |
| ntfy-alertmanager | `NTFY_SERVER` | `http://ntfy.railway.internal` |
| ntfy-alertmanager | `NTFY_TOPIC` | e.g. `katpool-tn10` |
| ntfy-alertmanager | `NTFY_ACCESS_TOKEN` | ntfy token (created post-deploy) |
| ntfy-alertmanager | `WEBHOOK_PASSWORD` | `${{alertmanager.ALERTMANAGER_WEBHOOK_PASSWORD}}` |

## Provisioning order

1. `victoriametrics`, `loki`, `tempo`, `blackbox` (no inbound deps).
2. `vmauth` (→ VM), `vmalert` (→ VM + Alertmanager), `ntfy`.
3. `alertmanager` (→ ntfy-alertmanager), `ntfy-alertmanager` (→ ntfy).
4. `grafana` (→ VM/Loki/Tempo).
5. Attach public domains to `grafana`, `ntfy`, `vmauth` (operator-provided
   custom domains; set each domain's **target port** per the table).

## Post-deploy

- **ntfy user + token (declarative; no shell):** ntfy has no CLI in the Railway
  UI, so provision via env on the `ntfy` service — `NTFY_AUTH_USERS` =
  `<user>:<bcrypt>:admin` and `NTFY_AUTH_TOKENS` = `<user>:<tk_…>:<label>` (token
  is `tk_` + 29 chars of `[a-z0-9]`; bcrypt is `$2a$`/`$2b$`). Set the same token
  as `NTFY_ACCESS_TOKEN` on `ntfy-alertmanager`, then redeploy both.
- **Origin agents (pool VPS):** the origin reaches Railway only via vmauth, so
  all three signals egress through it (`../origin/README.md`):
  - *Metrics* — vmagent with `../victoria-metrics/origin-vmagent.yml`,
    `-remoteWrite.url=https://<vmauth-domain>/api/v1/write` + `VMAUTH_WRITE_*`.
  - *Logs + traces* — Grafana Alloy with `../origin/alloy.alloy`, pushing to
    `https://<vmauth-domain>/loki/api/v1/push` and `…/v1/traces`.
- **Pool runtime:** set `KATPOOL_LOG_FORMAT=json` and
  `KATPOOL_OTLP_ENDPOINT=http://127.0.0.1:4317` (the LOCAL Alloy receiver — the
  gRPC-only, header-less exporter cannot authenticate to vmauth itself).
- **Verify:** Grafana → *katpool — Pool Overview* shows live metrics;
  `{network="…"} | json` returns logs (Loki) and `service.name="katpool-…"`
  returns spans (Tempo); fire a test alert and confirm it reaches the phone.

## Mainnet rebuild

Repeat in the mainnet project/environment with mainnet public domains, a mainnet
ntfy topic, and the origin vmagent on the mainnet VPS. The artifacts here are
environment-agnostic; only domains, topics, and probe hosts differ.

## Local validation

From `ops/railway/observability/`:

```bash
docker build -f deploy/<service>/Dockerfile -t obs-<service> .
```

All ten images are expected to build and parse their config on start.
