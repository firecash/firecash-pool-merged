#!/bin/sh
# Render the bridge's scfg config from env at startup. The config holds two
# secrets — the inbound webhook password (must equal Alertmanager's
# ALERTMANAGER_WEBHOOK_PASSWORD) and the ntfy access token — so it is never baked
# or committed; both come from Railway service variables (the webhook password is
# shared with Alertmanager via a reference variable). scfg format + key names per
# the upstream config.scfg (git.xenrox.net/~xenrox/ntfy-alertmanager).
set -eu

: "${NTFY_SERVER:?NTFY_SERVER must be set}"
: "${NTFY_TOPIC:?NTFY_TOPIC must be set}"
: "${NTFY_ACCESS_TOKEN:?NTFY_ACCESS_TOKEN must be set}"
: "${WEBHOOK_PASSWORD:?WEBHOOK_PASSWORD must be set}"
WEBHOOK_USER="${WEBHOOK_USER:-alertmanager}"

mkdir -p /etc/ntfy-alertmanager
cat > /etc/ntfy-alertmanager/config <<EOF
http-address :8080
log-format json
log-level info
alert-mode single
user ${WEBHOOK_USER}
password ${WEBHOOK_PASSWORD}

labels {
    order "severity"

    severity "page" {
        priority 5
        tags "rotating_light"
    }

    severity "warning" {
        priority 4
        tags "warning"
    }
}

resolved {
    update-notification true
    tags "white_check_mark"
    priority 2
}

ntfy {
    server ${NTFY_SERVER}
    topic ${NTFY_TOPIC}
    access-token ${NTFY_ACCESS_TOKEN}
    markdown true
}

cache {
    type memory
    duration 24h
    cleanup-interval 1h
}
EOF

# Base ENTRYPOINT is ./ntfy-alertmanager (run from its WORKDIR); preserve that.
exec ./ntfy-alertmanager --config /etc/ntfy-alertmanager/config "$@"
