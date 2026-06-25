# Origin observability agents (netcup)

The katpool pool host produces telemetry; the Railway LGTM stack stores and
alerts on it (ADR-0004 failure-domain split). Three signals leave the origin,
all through the single public, basic-auth'd **vmauth** door — VictoriaMetrics,
Loki, and Tempo themselves stay private and unauthenticated on Railway's
internal network:

| Signal  | Origin agent | vmauth path          | Backend            |
|---------|--------------|----------------------|--------------------|
| Metrics | vmagent      | `/api/v1/write`      | VictoriaMetrics    |
| Logs    | **Alloy**    | `/loki/api/v1/push`  | Loki               |
| Traces  | **Alloy**    | `/v1/traces`         | Tempo (OTLP/HTTP)  |

`vmagent` is documented in `../victoria-metrics/origin-vmagent.yml`. This
directory covers the **Alloy** agent (`alloy.alloy`) that ships logs + traces.

## Why Alloy, and why local

The pool writes structured JSON logs to journald and exports OTLP/**gRPC** spans.
Its OTLP exporter is gRPC-only and sends **no auth headers**, so it cannot reach
vmauth (HTTP + basic auth) directly, and gRPC cannot traverse vmauth at all.
Alloy therefore runs on the origin as a local collector: it receives the pool's
spans on `127.0.0.1:4317`, tails the pool's journald unit, and egresses both to
Railway over the authenticated OTLP/HTTP + Loki-push paths on vmauth.

## Prerequisites

1. The vmauth routes for `/loki/api/v1/push` and `/v1/traces` must be deployed
   (they live in `../deploy/vmauth/auth.yml`; redeploy vmauth after merging).
2. The pool must emit JSON logs and OTLP to localhost. In the per-network env
   file (`ops/env/<network>.env`):
   ```
   KATPOOL_LOG_FORMAT=json
   KATPOOL_OTLP_ENDPOINT=http://127.0.0.1:4317
   OTEL_SERVICE_NAME=katpool-tn10   # optional; otherwise KATPOOL_INSTANCE_ID
   ```
   Then `scripts/deploy.sh --network <network>` (or restart the service).
3. The vmauth write credentials (same identity the vmagent uses).

## Run (Docker, pinned)

Host networking is required so Alloy can receive on `127.0.0.1:4317` from the
host pool process; the host journal is mounted read-only (works whether the host
stores it persistently under `/var/log/journal` or volatile under
`/run/log/journal`).

```sh
# Secrets live in an env file, not on the command line (mode 0600):
#   /etc/katpool/obs-tn10/alloy.env
#     VMAUTH_URL=https://vmauth-stage-testnet-10.up.railway.app
#     VMAUTH_USER=katpool-tn10-write
#     VMAUTH_PASSWORD=...                     # same as the vmagent's
#     KATPOOL_NETWORK=testnet-10
#     KATPOOL_INSTANCE=tn10-phase5
#     KATPOOL_JOURNAL_UNIT=katpool-tn10.service

JOURNAL_DIR=/var/log/journal; [ -d "$JOURNAL_DIR" ] || JOURNAL_DIR=/run/log/journal

# Install the config to a stable path (decoupled from the git working tree) and
# create a persistent data dir for Alloy's WAL so a restart never drops buffered
# logs/traces:
install -m 0644 /root/katpool/ops/railway/observability/origin/alloy.alloy \
  /etc/katpool/obs-tn10/alloy.alloy
mkdir -p /etc/katpool/obs-tn10/alloy-data

docker run -d --name katpool-alloy-tn10 --restart unless-stopped --network host \
  --env-file /etc/katpool/obs-tn10/alloy.env \
  -v /etc/katpool/obs-tn10/alloy.alloy:/etc/alloy/config.alloy:ro \
  -v /etc/katpool/obs-tn10/alloy-data:/var/lib/alloy/data \
  -v "$JOURNAL_DIR":/var/log/journal:ro \
  -v /etc/machine-id:/etc/machine-id:ro \
  grafana/alloy:v1.17.0 \
  run --server.http.listen-addr=127.0.0.1:12345 --storage.path=/var/lib/alloy/data /etc/alloy/config.alloy
```

The `loki.write` WAL and the `--storage.path` host volume together make log
delivery durable across agent restarts and transient vmauth outages. Trace
volume is bounded by the `otelcol.processor.tail_sampling` block (keeps error +
slow traces in full; `sampling_percentage` is 100 on tn10, lower it for mainnet).

## Verify

```sh
# Alloy components healthy (no "unhealthy" rows):
curl -s http://127.0.0.1:12345/api/v0/web/components | python3 -m json.tool | grep -i health

# Pool actually exporting to the local receiver (after the env change + restart):
ss -ltnp | grep 4317

# Logs in Loki / traces in Tempo (query through Grafana's datasource proxy):
#   {network="testnet-10"} | json            # LogQL
#   service.name = "katpool-tn10"             # TraceQL
```

The same agent serves mainnet — only the `alloy.env` values change.
