#!/usr/bin/env bash
# Install the katpool-bridge systemd unit and its hardened drop-in
# directory. Idempotent.
#
# Usage (must be run as root):
#
#   sudo ./install.sh
#   sudo systemctl enable --now katpool-bridge
#
# Operators tune anti-abuse limits and network ACLs by copying the
# `.example` files in `katpool-bridge.conf.d/` to `.conf` in
# /etc/systemd/system/katpool-bridge.service.d/.

set -euo pipefail

if [[ ${EUID} -ne 0 ]]; then
    echo "must be run as root" >&2
    exit 1
fi

UNIT_SRC="$(cd "$(dirname "$0")" && pwd)/katpool-bridge.service"
UNIT_DST=/etc/systemd/system/katpool-bridge.service
DROPIN_DIR=/etc/systemd/system/katpool-bridge.service.d
STATE_DIR=/var/lib/katpool-bridge
ETC_DIR=/etc/katpool
SVC_USER=katpool

if ! id -u "${SVC_USER}" >/dev/null 2>&1; then
    echo "creating system user ${SVC_USER}"
    useradd --system --no-create-home --shell /usr/sbin/nologin "${SVC_USER}"
fi

install -m 0644 "${UNIT_SRC}" "${UNIT_DST}"
install -d -o root -g root -m 0755 "${DROPIN_DIR}"
install -d -o "${SVC_USER}" -g "${SVC_USER}" -m 0750 "${STATE_DIR}"
install -d -o root -g "${SVC_USER}" -m 0750 "${ETC_DIR}"

# Verify hardening surface before declaring success.
if command -v systemd-analyze >/dev/null; then
    echo "--- systemd-analyze security ---"
    systemd-analyze security --no-pager katpool-bridge || true
fi

systemctl daemon-reload
echo "installed. Enable with: systemctl enable --now katpool-bridge"
