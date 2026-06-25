# vmalert (B3) — evaluates the katpool alerting + SLO recording rules against
# VictoriaMetrics and routes firing alerts to Alertmanager. Same release line as
# VM (pin together). Build context is ops/railway/observability/.
FROM victoriametrics/vmalert:v1.145.0@sha256:5f241da926a531c11f870e7fec6dfa29fc7e34a0bd9ac573d2c9200dd9ab5f36

# Both the alerting rules and the SLO recording rules.
COPY victoria-metrics/rules/ /etc/vmalert/rules/

# Internal Railway DNS for datasource/notifier (free, private). Recording-rule
# results are written back to VM so Grafana/SLO panels can read precomputed SLIs.
# `-rule` takes a file or glob, not a bare directory (vmalert does its own glob
# matching, so no shell is needed for the `*`).
CMD ["-rule=/etc/vmalert/rules/*.yml", \
     "-datasource.url=http://victoriametrics.railway.internal:8428", \
     "-remoteWrite.url=http://victoriametrics.railway.internal:8428", \
     "-remoteRead.url=http://victoriametrics.railway.internal:8428", \
     "-notifier.url=http://alertmanager.railway.internal:9093", \
     "-evaluationInterval=30s", \
     "-httpListenAddr=:8880"]
