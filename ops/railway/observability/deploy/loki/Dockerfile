# Loki (B4) — single-binary log store, filesystem-backed on a Railway volume
# mounted at /loki. Build context is ops/railway/observability/.
FROM grafana/loki:3.7.2@sha256:191d4fdfb7264f16989f0a57f320872620a5a7c2ceeec6229212c4190ec49b86

COPY loki/loki-config.yaml /etc/loki/loki-config.yaml

# Base ENTRYPOINT is /usr/bin/loki; override the default config path.
CMD ["-config.file=/etc/loki/loki-config.yaml"]
