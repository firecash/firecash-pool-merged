# VictoriaMetrics single-node (B3/B4) — metrics store + Railway-side scraper.
# Pinned by digest (resolved from the registry; bump tag+digest together — same
# discipline as the CI actions). Build context is ops/railway/observability/.
FROM victoriametrics/victoria-metrics:v1.145.0@sha256:c014fb5a711d38cb24fd0673197592cd1394bb903dbb16aea565620c9c8a3d70

# Railway-side scrape config (VM self-metrics + Blackbox probes of the pool's
# public surface). `%{KATPOOL_API_HOST}` / `%{KATPOOL_STRATUM_HOST}` placeholders
# are expanded by VictoriaMetrics from the service env at startup.
COPY victoria-metrics/scrape.yml /etc/vm/scrape.yml

# Base ENTRYPOINT is /victoria-metrics-prod; these are its flags. The data path
# is the mounted Railway volume; 90d retention matches SLO.md.
CMD ["-promscrape.config=/etc/vm/scrape.yml", \
     "-storageDataPath=/victoria-metrics-data", \
     "-retentionPeriod=90d", \
     "-httpListenAddr=:8428"]
