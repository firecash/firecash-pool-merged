#!/usr/bin/env bash
# Install kaspad-tn10 — Toccata-aware rusty-kaspa for testnet-10.
#
# Idempotent. Downloads the pinned upstream release zip, verifies
# its SHA-256 against the pinned digest below, extracts the kaspad
# binary, and installs the systemd unit. Reuses an existing checkout
# and skips download steps that have already completed successfully.
#
# Usage (must be run as root):
#
#   sudo ./install-kaspad-tn10.sh
#   sudo systemctl enable --now katpool-kaspad-tn10
#
# To upgrade later when upstream cuts a new tn10-capable release, bump
# the `TN10_RELEASE_TAG` + `TN10_LINUX_SHA256` constants below and
# re-run this script.

set -euo pipefail

# ---------- Pinned upstream release ----------------------------------
# Source: https://github.com/kaspanet/rusty-kaspa/releases/tag/v2.0.1
# Maintenance release on top of v2.0.0 (Toccata). Drop-in for v2.0.0 nodes;
# runs as testnet-10 via `--testnet --netsuffix=10`. Bumping these two
# constants is the only change required to track a future release.
TN10_RELEASE_TAG=v2.0.1
TN10_LINUX_SHA256=9d0ad0aedbe29670e3e2dde664462c526d30a2d2ff7274d18b1a310a127d1c13
TN10_LINUX_ZIP=rusty-kaspa-${TN10_RELEASE_TAG}-linux-amd64.zip
TN10_DOWNLOAD_URL=https://github.com/kaspanet/rusty-kaspa/releases/download/${TN10_RELEASE_TAG}/${TN10_LINUX_ZIP}

# ---------- Local layout ---------------------------------------------
SVC_USER=kaspad-tn10
SVC_HOME=/var/lib/kaspad-tn10
SVC_BIN_DIR=/usr/local/bin
SVC_BIN=${SVC_BIN_DIR}/kaspad-tn10
DOWNLOAD_DIR=/var/cache/kaspad-tn10
UNIT_SRC="$(cd "$(dirname "$0")" && pwd)/katpool-kaspad-tn10.service"
UNIT_DST=/etc/systemd/system/katpool-kaspad-tn10.service

if [[ ${EUID} -ne 0 ]]; then
    echo "must be run as root" >&2
    exit 1
fi

for tool in curl unzip sha256sum systemctl install; do
    if ! command -v "${tool}" >/dev/null 2>&1; then
        echo "missing required tool: ${tool}" >&2
        exit 2
    fi
done

# ---------- System user ----------------------------------------------
if ! id -u "${SVC_USER}" >/dev/null 2>&1; then
    echo "creating system user ${SVC_USER}"
    useradd --system --no-create-home --home-dir "${SVC_HOME}" --shell /usr/sbin/nologin "${SVC_USER}"
fi

# ---------- Directories ----------------------------------------------
install -d -o "${SVC_USER}" -g "${SVC_USER}" -m 0750 "${SVC_HOME}"
install -d -o root -g root -m 0755 "${DOWNLOAD_DIR}"

# ---------- Fetch + verify + extract --------------------------------
zip_path="${DOWNLOAD_DIR}/${TN10_LINUX_ZIP}"
need_install=1
if [[ -x "${SVC_BIN}" ]]; then
    installed_sha=$("${SVC_BIN}" --version 2>/dev/null | head -1 | awk '{print $NF}' || true)
    if [[ -n "${installed_sha}" ]]; then
        echo "kaspad-tn10 already installed (${installed_sha}); will reinstall to match pinned release"
    fi
fi

if [[ ! -f "${zip_path}" ]]; then
    echo "downloading ${TN10_DOWNLOAD_URL}"
    curl -fsSL -o "${zip_path}.partial" "${TN10_DOWNLOAD_URL}"
    mv "${zip_path}.partial" "${zip_path}"
fi

actual_sha=$(sha256sum "${zip_path}" | awk '{print $1}')
if [[ "${actual_sha}" != "${TN10_LINUX_SHA256}" ]]; then
    echo "SHA-256 mismatch for ${TN10_LINUX_ZIP}" >&2
    echo "  expected: ${TN10_LINUX_SHA256}" >&2
    echo "  actual:   ${actual_sha}" >&2
    rm -f "${zip_path}"
    exit 3
fi

# Extract just the kaspad binary; everything else in the zip
# (stratum-bridge, kaspa-wallet, rothschild) is upstream's bundle and
# not what this unit runs.
tmp_extract=$(mktemp -d)
trap 'rm -rf "${tmp_extract}"' EXIT
unzip -qo "${zip_path}" "bin/kaspad" -d "${tmp_extract}"

if [[ ${need_install} -eq 1 ]] || ! cmp -s "${tmp_extract}/bin/kaspad" "${SVC_BIN}" 2>/dev/null; then
    install -m 0755 -o root -g root "${tmp_extract}/bin/kaspad" "${SVC_BIN}"
    echo "installed ${SVC_BIN} from ${TN10_RELEASE_TAG}"
else
    echo "${SVC_BIN} unchanged (already matches ${TN10_RELEASE_TAG})"
fi

# ---------- systemd unit --------------------------------------------
install -m 0644 -o root -g root "${UNIT_SRC}" "${UNIT_DST}"
systemctl daemon-reload

if command -v systemd-analyze >/dev/null; then
    echo "--- systemd-analyze security ---"
    systemd-analyze security --no-pager katpool-kaspad-tn10 || true
fi

cat <<EOF
installed. Next steps:

    sudo systemctl enable --now katpool-kaspad-tn10
    journalctl -fu katpool-kaspad-tn10            # follow sync progress
    ss -ltnp | grep -E ':1[678]2[01]0'             # confirm bound ports

Default ports (rusty-kaspa testnet-10 convention):
    16210  gRPC
    16211  P2P
    17210  wRPC (borsh)
    18210  wRPC (json)

Sync to current tip typically takes 30–60 minutes on this VPS. The
Phase 1 acceptance smoke runs against this node when IBD completes.
EOF
