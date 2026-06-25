# Alertmanager (B5) — routes vmalert alerts to the ntfy bridge. Build context is
# ops/railway/observability/. Storage on a Railway volume at /alertmanager; the
# service sets RAILWAY_RUN_UID=0 so the (otherwise non-root) image can write the
# volume (per Railway volumes reference).
FROM prom/alertmanager:v0.33.0@sha256:af26fbe4dd1886ac0efd7bd55cd9027da262e105b137a376522b7c14c3626e4a

COPY alertmanager/alertmanager.yml /etc/alertmanager/alertmanager.yml
COPY deploy/alertmanager/entrypoint.sh /entrypoint.sh

USER root
RUN chmod 0755 /entrypoint.sh

ENTRYPOINT ["/entrypoint.sh"]
