# katpool fly.io anycast stratum edge (ADR-0022).
# Pinned HAProxy LTS on Alpine; config is baked in and parsed at boot.
FROM haproxy:3.0-alpine

# Bundle the forwarder config. KATPOOL_ORIGIN_HOST is injected at runtime
# (fly secret) and substituted by HAProxy's native env expansion.
COPY haproxy.cfg /usr/local/etc/haproxy/haproxy.cfg

# Fail fast on a bad config at build time. `-c` checks; the env var must be
# present for the check to succeed, so provide a throwaway value here only.
RUN KATPOOL_ORIGIN_HOST=build-check.invalid \
    haproxy -c -f /usr/local/etc/haproxy/haproxy.cfg

# The base image's ENTRYPOINT already runs haproxy with this config.
