#!/bin/sh
# Alertmanager's config references the ntfy-bridge webhook password via
# `password_file` (a secret must never be baked into the image or committed).
# Railway has no file mounts, so we materialize the file from a secret env var
# at startup, then exec Alertmanager. The variable is set on the Railway service
# (and shared with the bridge via a reference variable so both sides match).
set -eu

: "${ALERTMANAGER_WEBHOOK_PASSWORD:?ALERTMANAGER_WEBHOOK_PASSWORD must be set}"

mkdir -p /etc/alertmanager/secrets
printf '%s' "$ALERTMANAGER_WEBHOOK_PASSWORD" > /etc/alertmanager/secrets/webhook_password
chmod 0600 /etc/alertmanager/secrets/webhook_password

exec /bin/alertmanager \
  --config.file=/etc/alertmanager/alertmanager.yml \
  --storage.path=/alertmanager \
  "$@"
