# ntfy-alertmanager bridge (B5) — maps Alertmanager severity → ntfy priority/tags
# and holds the ntfy token. NOTE: the canonical image is on Docker Hub
# (xenrox/ntfy-alertmanager), not GHCR. Build context is ops/railway/observability/.
# No volume (stateless; in-memory dedup cache).
FROM xenrox/ntfy-alertmanager:1.0.0@sha256:81788c7905774b7b0b2ed6833b2bc4826a90a42e4b738706edcedd5f489e7a73

COPY deploy/ntfy-alertmanager/entrypoint.sh /entrypoint.sh

USER root
RUN chmod 0755 /entrypoint.sh

ENTRYPOINT ["/entrypoint.sh"]
