# vmauth (B4) — public, basic-auth'd reverse proxy for the remote-write path so
# VictoriaMetrics can stay private. Same release line as VM (pin together).
FROM victoriametrics/vmauth:v1.145.0@sha256:c816c52c4d9187566c3dc23298dbd7c273352bfbacc811c353ff6e7f41921e10

COPY deploy/vmauth/auth.yml /etc/vmauth/auth.yml

# Base ENTRYPOINT is /vmauth-prod. Listen on the port Railway exposes publicly.
CMD ["-auth.config=/etc/vmauth/auth.yml", "-httpListenAddr=:8427"]
