# Tempo (B4) — single-binary trace store, local backend on a Railway volume
# mounted at /var/tempo. Receives OTLP/gRPC on :4317 from the runtime when
# KATPOOL_OTLP_ENDPOINT is set. Build context is ops/railway/observability/.
FROM grafana/tempo:2.10.7@sha256:032b3acb51ed02c4b801473d54bb63e9e9f13738d215126d9843c30283794f4b

COPY tempo/tempo.yaml /etc/tempo/tempo.yaml

# Base ENTRYPOINT is /tempo; point it at the mounted config.
CMD ["-config.file=/etc/tempo/tempo.yaml"]
