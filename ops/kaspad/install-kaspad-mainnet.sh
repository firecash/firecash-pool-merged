#!/usr/bin/env bash
# Install kaspad-mainnet — rusty-kaspa v2.0.0 for the NEW pool's dedicated
# mainnet node (run alongside, never touching, the legacy dockerized v1.0.1
# mainnet node). See katpool-kaspad-mainnet.service for the rationale.
#
# Idempotent. Downloads the pinned upstream release zip, verifies its SHA-256,
# extracts the kaspad binary to /usr/local/bin/kaspad-mainnet, and installs the
# systemd unit. The binary is identical to kaspad-tn10 (rusty-kaspa is
# network-agnostic; mainnet just omits --testnet); installed under a distinct
# name + user + datadir so the two nodes are fully independent.
#
# Usage (must be run as root):
#
#   sudo ./install-kaspad-mainnet.sh
#   sudo systemctl enable --now katpool-kaspad-mainnet
#
# To upgrade later, bump MAINNET_RELEASE_TAG + MAINNET_LINUX_SHA256 and re-run.

set -euo pipefail

# ---------- Pinned upstream release ----------------------------------
# Source: https://github.com/kaspanet/rusty-kaspa/releases/tag/v2.0.1
# Maintenance release on top of v2.0.0 (Toccata). Drop-in for v2.0.0 nodes;
# same binary as kaspad-tn10 — run without --testnet it is a mainnet node.
MAINNET_RELEASE_TAG=v2.0.1
MAINNET_LINUX_SHA256=9d0ad0aedbe29670e3e2dde664462c526d30a2d2ff7274d18b1a310a127d1c13
MAINNET_LINUX_ZIP=rusty-kaspa-${MAINNET_RELEASE_TAG}-linux-amd64.zip
MAINNET_DOWNLOAD_URL=https://github.com/kaspanet/rusty-kaspa/releases/download/${MAINNET_RELEASE_TAG}/${MAINNET_LINUX_ZIP}

# ---------- Local layout ---------------------------------------------
SVC_USER=kaspad-mainnet
SVC_HOME=/var/lib/kaspad-mainnet
SVC_BIN_DIR=/usr/local/bin
SVC_BIN=${SVC_BIN_DIR}/kaspad-mainnet
DOWNLOAD_DIR=/var/cache/kaspad-mainnet
UNIT_SRC="$(cd "$(dirname "$0")" && pwd)/katpool-kaspad-mainnet.service"
UNIT_DST=/etc/systemd/system/katpool-kaspad-mainnet.service
# Reuse path: the tn10 install already fetched + verified this exact binary.
EXISTING_BIN=/usr/local/bin/kaspad-tn10

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

# ---------- Obtain the binary ----------------------------------------
# Fast path: if a v2.0.1 kaspad is already installed (kaspad-tn10), reuse it
# verbatim instead of re-downloading — same upstream binary.
reuse=0
if [[ -x "${EXISTING_BIN}" ]]; then
    ver=$("${EXISTING_BIN}" --version 2>/dev/null | head -1 | awk '{print $NF}' || true)
    if [[ "${ver}" == "2.0.1" ]]; then
        echo "reusing existing verified v2.0.1 binary from ${EXISTING_BIN}"
        install -m 0755 -o root -g root "${EXISTING_BIN}" "${SVC_BIN}"
        reuse=1
    fi
fi

if [[ ${reuse} -eq 0 ]]; then
    zip_path="${DOWNLOAD_DIR}/${MAINNET_LINUX_ZIP}"
    if [[ ! -f "${zip_path}" ]]; then
        echo "downloading ${MAINNET_DOWNLOAD_URL}"
        curl -fsSL -o "${zip_path}.partial" "${MAINNET_DOWNLOAD_URL}"
        mv "${zip_path}.partial" "${zip_path}"
    fi
    actual_sha=$(sha256sum "${zip_path}" | awk '{print $1}')
    if [[ "${actual_sha}" != "${MAINNET_LINUX_SHA256}" ]]; then
        echo "SHA-256 mismatch for ${MAINNET_LINUX_ZIP}" >&2
        echo "  expected: ${MAINNET_LINUX_SHA256}" >&2
        echo "  actual:   ${actual_sha}" >&2
        rm -f "${zip_path}"
        exit 3
    fi
    tmp_extract=$(mktemp -d)
    trap 'rm -rf "${tmp_extract}"' EXIT
    unzip -qo "${zip_path}" "bin/kaspad" -d "${tmp_extract}"
    install -m 0755 -o root -g root "${tmp_extract}/bin/kaspad" "${SVC_BIN}"
    echo "installed ${SVC_BIN} from ${MAINNET_RELEASE_TAG}"
fi

# ---------- systemd unit --------------------------------------------
install -m 0644 -o root -g root "${UNIT_SRC}" "${UNIT_DST}"
systemctl daemon-reload

if command -v systemd-analyze >/dev/null; then
    echo "--- systemd-analyze security ---"
    systemd-analyze security --no-pager katpool-kaspad-mainnet || true
fi

cat <<EOF
installed. Next steps:

    sudo systemctl enable --now katpool-kaspad-mainnet
    journalctl -fu katpool-kaspad-mainnet            # follow sync progress
    ss -ltnp | grep -E ':1[678]12[01]'               # confirm bound ports

TEMPORARY non-default mainnet ports (legacy node holds the defaults):
    16120  gRPC        (the pool's KASPAD_GRPC_URL=grpc://127.0.0.1:16120)
    16121  P2P
    17120  wRPC (borsh)
    18120  wRPC (json)

After the legacy node is decommissioned at cutover, rebind to the default
ports (16110/16111/17110/18110) and update KASPAD_GRPC_URL — a plain restart,
no resync. A fresh mainnet IBD typically takes a few hours on this VPS.
EOF
