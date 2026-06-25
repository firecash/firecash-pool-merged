#!/usr/bin/env bash
# Network-aware deploy for the unified katpool runtime.
#
# One identical binary serves every network — the network is selected purely
# at runtime by the kaspad endpoint and the kaspatest:/kaspa: address prefix
# (see katpool/src/main.rs). This script therefore takes a *deploy-target*
# flag, not a build-time network: it builds (or accepts) the binary once and
# installs it, the tracked systemd unit, and the per-network env file into the
# correct per-network location, then restarts that network's service.
#
# Layout (symmetric; see docs/runbooks/09-deploy-and-rollback.md):
#   testnet : /root/katpool-tn10/katpool      katpool-tn10.service
#   mainnet : /root/katpool-mainnet/katpool   katpool-mainnet.service
#
# Usage (run as root):
#   scripts/deploy.sh --network <tn10|mainnet> [options]
#
# Options:
#   --network <tn10|mainnet>   target network (required)
#   --release <tag>            download + cosign-verify the signed release
#                              artifact (binary + bundle) from GitHub, then
#                              install it (needs the `gh` CLI)
#   --binary <path>            install this prebuilt binary instead of building
#                              (e.g. a manually downloaded signed release artifact)
#   --bundle <path>            cosign Sigstore bundle for --binary
#                              (default: "<binary>.sigstore-bundle.json")
#   --no-build                 reuse the existing dist binary; do not cargo build
#   --no-verify                skip cosign verification of a prebuilt artifact
#                              (NOT recommended; for offline/pre-verified flows)
#   --skip-restart             install everything but do not restart the service
#   -h, --help                 show this help
#
# Prebuilt artifacts (--release / --binary) are cosign-verified against the
# release workflow's keyless signature before install; a locally built binary
# is unsigned and installed as-is. After restart the deploy waits for /ready
# (DB-reachable AND kaspad-synced). See docs/runbooks/09-deploy-and-rollback.md.
#
# Examples:
#   sudo scripts/deploy.sh --network tn10                       # build from source
#   sudo scripts/deploy.sh --network mainnet --release v1.2.0   # signed release
#   sudo scripts/deploy.sh --network mainnet --binary /tmp/katpool

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
UNIT_TEMPLATE="${REPO_ROOT}/ops/systemd/katpool.service.in"
ETC_DIR=/etc/katpool
KEEP_BACKUPS=5
RELEASE_REPO="${KATPOOL_RELEASE_REPO:-Nacho-the-Kat/katpool}"
READY_TIMEOUT_SECS=30

network=""
binary=""
bundle=""
release_tag=""
do_build=1
do_restart=1
do_verify=1

# Temp files cleaned up on exit (set as they are created).
tmp_unit=""
fetch_dir=""
cleanup() {
    [[ -n "${tmp_unit}" ]] && rm -f "${tmp_unit}"
    [[ -n "${fetch_dir}" ]] && rm -rf "${fetch_dir}"
    return 0
}
trap cleanup EXIT

die() {
    echo "deploy: $*" >&2
    exit 1
}

usage() {
    # Print the contiguous header comment block (after the shebang), stripping
    # the leading "# "; robust to the block's length changing.
    awk 'NR>1 && /^#/ {sub(/^# ?/, ""); print; next} NR>1 {exit}' "${BASH_SOURCE[0]}"
    exit "${1:-0}"
}

# Extract the readiness-probe port from an installed env file: the dedicated
# health port if set, else the public API port. Values are "host:port" or
# ":port"; echoes the bare numeric port, or nothing if neither is configured.
probe_port_from_env() {
    local file="$1" key val
    [[ -f "${file}" ]] || return 0
    for key in KATPOOL_HEALTH_CHECK_PORT KATPOOL_API_PORT; do
        val="$(sed -n "s/^[[:space:]]*${key}=//p" "${file}" | tail -n1 | tr -d '\r' | xargs)"
        if [[ -n "${val}" ]]; then
            echo "${val##*:}"
            return 0
        fi
    done
    return 0
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --network)
            network="${2:-}"
            shift 2
            ;;
        --release)
            release_tag="${2:-}"
            do_build=0
            shift 2
            ;;
        --binary)
            binary="${2:-}"
            do_build=0
            shift 2
            ;;
        --bundle)
            bundle="${2:-}"
            shift 2
            ;;
        --no-build)
            do_build=0
            shift
            ;;
        --no-verify)
            do_verify=0
            shift
            ;;
        --skip-restart)
            do_restart=0
            shift
            ;;
        -h | --help)
            usage 0
            ;;
        *)
            die "unknown argument: $1 (use --help)"
            ;;
    esac
done

if [[ -n "${release_tag}" && -n "${binary}" ]]; then
    die "--release and --binary are mutually exclusive"
fi

case "${network}" in
    tn10 | mainnet) ;;
    "") die "missing required --network <tn10|mainnet>" ;;
    *) die "invalid --network '${network}' (expected tn10 or mainnet)" ;;
esac

if [[ ${EUID} -ne 0 ]]; then
    die "must be run as root (installs to /etc, /root, and restarts systemd)"
fi

deploy_dir="/root/katpool-${network}"
service="katpool-${network}"
env_src="${REPO_ROOT}/ops/env/${network}.env"
env_dst="${ETC_DIR}/${network}.env"
unit_dst="/etc/systemd/system/${service}.service"

[[ -f "${UNIT_TEMPLATE}" ]] || die "unit template not found: ${UNIT_TEMPLATE}"

if [[ ! -f "${env_src}" ]]; then
    die "missing ${env_src}: copy ops/env/${network}.env.example to ops/env/${network}.env and fill it in (the real env is host-local / gitignored)"
fi

# ----- Resolve the binary (and its signature bundle) -----------------------
# `prebuilt` marks an artifact that should carry a release signature (a fetched
# --release asset or a supplied --binary), as opposed to a local source build.
prebuilt=0
if [[ -n "${release_tag}" ]]; then
    command -v gh >/dev/null 2>&1 || die "--release needs the GitHub CLI (gh) on PATH"
    fetch_dir="$(mktemp -d)"
    echo "==> downloading signed release ${release_tag} from ${RELEASE_REPO}"
    gh release download "${release_tag}" --repo "${RELEASE_REPO}" \
        --pattern katpool --pattern katpool.sigstore-bundle.json \
        --dir "${fetch_dir}" ||
        die "failed to download katpool + bundle for ${release_tag} from ${RELEASE_REPO}"
    src_bin="${fetch_dir}/katpool"
    bundle="${fetch_dir}/katpool.sigstore-bundle.json"
    chmod +x "${src_bin}" 2>/dev/null || true
    prebuilt=1
elif [[ -n "${binary}" ]]; then
    [[ -f "${binary}" ]] || die "--binary path not found: ${binary}"
    src_bin="${binary}"
    prebuilt=1
else
    target_dir="${CARGO_TARGET_DIR:-${REPO_ROOT}/target}"
    src_bin="${target_dir}/dist/katpool"
    if [[ ${do_build} -eq 1 ]]; then
        echo "==> building dist binary (cargo build --profile dist --locked --bin katpool)"
        (cd "${REPO_ROOT}" && cargo build --profile dist --locked --bin katpool)
    fi
    [[ -f "${src_bin}" ]] || die "dist binary not found at ${src_bin} (drop --no-build, or pass --binary)"
fi

# ----- Verify the cosign signature of a prebuilt artifact ------------------
# Refuse to install an unverified release artifact. A locally built binary has
# no release signature, so verification applies only to --release / --binary.
if [[ ${prebuilt} -eq 1 && ${do_verify} -eq 1 ]]; then
    bundle="${bundle:-${src_bin}.sigstore-bundle.json}"
    [[ -f "${bundle}" ]] || die "signature bundle not found: ${bundle}
    download <artifact>.sigstore-bundle.json from the release beside the binary,
    pass it with --bundle <path>, or (NOT recommended) re-run with --no-verify"
    KATPOOL_RELEASE_REPO="${RELEASE_REPO}" "${SCRIPT_DIR}/verify-release.sh" "${src_bin}" "${bundle}" ||
        die "cosign verification failed for ${src_bin}; refusing to deploy"
elif [[ ${prebuilt} -eq 1 ]]; then
    echo "==> WARNING: --no-verify set; installing UNVERIFIED prebuilt artifact"
else
    echo "==> note: locally built binary (unsigned); cosign verification skipped"
fi

echo "==> deploying $(basename "${src_bin}") to ${deploy_dir} for service ${service}"

# ----- Install env + rendered unit -----------------------------------------
install -d -m 0750 "${ETC_DIR}"
install -m 0640 "${env_src}" "${env_dst}"
echo "    installed env  -> ${env_dst}"

tmp_unit="$(mktemp)"
sed -e "s|__NETWORK__|${network}|g" -e "s|__DEPLOY_DIR__|${deploy_dir}|g" \
    "${UNIT_TEMPLATE}" > "${tmp_unit}"
install -m 0644 "${tmp_unit}" "${unit_dst}"
echo "    installed unit -> ${unit_dst}"

# Treasury key-rotation audit timer (Phase 8 / Runbook 11): renders the same
# __NETWORK__/__DEPLOY_DIR__ placeholders. Optional — only installed when the
# templates exist — so older checkouts still deploy cleanly.
audit_svc_tmpl="${REPO_ROOT}/ops/systemd/katpool-treasury-audit.service.in"
audit_timer_tmpl="${REPO_ROOT}/ops/systemd/katpool-treasury-audit.timer.in"
if [[ -f "${audit_svc_tmpl}" && -f "${audit_timer_tmpl}" ]]; then
    for kind in service timer; do
        tmpl="${REPO_ROOT}/ops/systemd/katpool-treasury-audit.${kind}.in"
        dst="/etc/systemd/system/katpool-treasury-audit-${network}.${kind}"
        tmp_f="$(mktemp)"
        sed -e "s|__NETWORK__|${network}|g" -e "s|__DEPLOY_DIR__|${deploy_dir}|g" \
            "${tmpl}" > "${tmp_f}"
        install -m 0644 "${tmp_f}" "${dst}"
        rm -f "${tmp_f}"
        echo "    installed unit -> ${dst}"
    done
fi

# ----- Back up + install the binary ----------------------------------------
install -d -m 0755 "${deploy_dir}"
dst_bin="${deploy_dir}/katpool"
if [[ -f "${dst_bin}" ]]; then
    backup="${dst_bin}.bak-$(date -u +%Y%m%dT%H%M%SZ)"
    cp -p "${dst_bin}" "${backup}"
    echo "    backed up old  -> ${backup}"
    # Prune to the most recent ${KEEP_BACKUPS} backups.
    mapfile -t old_backups < <(ls -1t "${dst_bin}".bak-* 2>/dev/null | tail -n "+$((KEEP_BACKUPS + 1))")
    for f in "${old_backups[@]:-}"; do
        [[ -n "${f}" ]] && rm -f "${f}" && echo "    pruned backup  -> ${f}"
    done
fi
install -m 0755 "${src_bin}" "${dst_bin}"
echo "    installed bin  -> ${dst_bin}"

# ----- Activate ------------------------------------------------------------
systemctl daemon-reload
systemctl enable "${service}" >/dev/null 2>&1 || true

# Enable + start the treasury-audit timer if it was installed above.
audit_timer="katpool-treasury-audit-${network}.timer"
if [[ -f "/etc/systemd/system/${audit_timer}" ]]; then
    systemctl enable --now "${audit_timer}" >/dev/null 2>&1 \
        && echo "    enabled timer  -> ${audit_timer}" \
        || echo "    warn: could not enable ${audit_timer}"
fi

if [[ ${do_restart} -eq 0 ]]; then
    echo "==> --skip-restart: not restarting ${service}"
    echo "    start it with: systemctl restart ${service}"
    exit 0
fi

echo "==> restarting ${service}"
systemctl restart "${service}"
sleep 2

if systemctl is-active --quiet "${service}"; then
    echo "==> ${service} is active"
else
    echo "deploy: ${service} failed to start; last logs:" >&2
    journalctl -u "${service}" -n 40 --no-pager >&2 || true
    die "service not active after restart (binary backup retained for rollback)"
fi

# ----- Readiness gate ------------------------------------------------------
# is-active only proves the process is up. Poll /ready (DB-reachable AND
# kaspad-synced) on the same probe the orchestrator uses, so the deploy is not
# declared done until the new binary is actually serving. Skipped only when no
# probe port is configured or curl is unavailable.
ready_port="$(probe_port_from_env "${env_dst}")"
if [[ "${ready_port}" =~ ^[0-9]+$ ]] && command -v curl >/dev/null 2>&1; then
    ready_url="http://127.0.0.1:${ready_port}/ready"
    echo "==> waiting up to ${READY_TIMEOUT_SECS}s for ${ready_url}"
    ready=0
    for _ in $(seq 1 "${READY_TIMEOUT_SECS}"); do
        if curl -fsS -o /dev/null --max-time 2 "${ready_url}"; then
            ready=1
            break
        fi
        sleep 1
    done
    if [[ ${ready} -eq 1 ]]; then
        echo "==> ${service} reports ready"
    else
        echo "deploy: ${service} not ready after ${READY_TIMEOUT_SECS}s; last logs:" >&2
        journalctl -u "${service}" -n 40 --no-pager >&2 || true
        die "readiness probe failed at ${ready_url} (binary backup retained for rollback)"
    fi
else
    echo "==> no health/API probe port configured (or curl missing); skipping readiness gate"
fi

echo "--- recent logs (${service}) ---"
journalctl -u "${service}" -n 15 --no-pager || true
echo
echo "Rollback: cp ${deploy_dir}/katpool.bak-<ts> ${dst_bin} && systemctl restart ${service}"
