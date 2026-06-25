# Grafana (B5) — dashboards + datasources provisioned from this repo so the UI is
# reproducible (no click-ops). Build context is ops/railway/observability/.
FROM grafana/grafana:13.0.2@sha256:5dad0df181cb644a14e13617b913b261a54f7d4fd4510721dba420929f35bea2

# Datasources (VM/Loki/Tempo on internal DNS) + the dashboard provider. Loaded
# from /etc/grafana/provisioning at boot.
COPY grafana/provisioning/ /etc/grafana/provisioning/

# Dashboards baked OUTSIDE /var/lib/grafana (the persistent volume) so the volume
# does not shadow them; the provider in dashboards.yaml points here.
COPY grafana/dashboards/ /etc/grafana/dashboards/

# Admin credentials, public root URL, SMTP etc. are supplied as GF_* env vars in
# the Railway service (admin password is a secret variable, never baked).
