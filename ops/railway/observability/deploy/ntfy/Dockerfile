# ntfy (B5) — push-notification server (the on-call paging target). Build context
# is ops/railway/observability/. Volume at /var/cache/ntfy; the service sets
# RAILWAY_RUN_UID=0 so the image can write the volume. Public domain on :80.
FROM binwiederhier/ntfy:v2.24.0@sha256:f8a9b104313b87cc24ae4f775f39e6328205b57dff6ede3eaf098a91e5d79f59

# ntfy refuses to start unless its cache dir already exists; create it so the
# service comes up cleanly (the Railway volume mounts over this path).
RUN mkdir -p /var/cache/ntfy/attachments

# ntfy reads /etc/ntfy/server.yml by default; NTFY_* env vars override it.
COPY deploy/ntfy/server.yml /etc/ntfy/server.yml

# Base ENTRYPOINT is `ntfy`; run the server.
CMD ["serve"]
