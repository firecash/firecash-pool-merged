# Blackbox exporter (B5) — synthetic HTTP/TCP probes of the pool's public surface
# (health endpoints, stratum ports, KRC-20 indexer deps). Targets + relabeling
# live in victoria-metrics/scrape.yml; this image just carries the probe modules.
# Build context is ops/railway/observability/.
FROM prom/blackbox-exporter:v0.28.0@sha256:e753ff9f3fc458d02cca5eddab5a77e1c175eee484a8925ac7d524f04366c2fc

COPY blackbox/blackbox.yml /etc/blackbox_exporter/config.yml

# Base ENTRYPOINT is /bin/blackbox_exporter with this default config path; listen
# on :9115 (internal only — VM scrapes it over private DNS).
