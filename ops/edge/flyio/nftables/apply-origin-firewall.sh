#!/usr/bin/env bash
# Fill the fly.io egress IPs into katpool-stratum.nft and apply it on the origin
# (C2; ADR-0022). The committed ruleset ships with RFC 5737/3849 documentation
# IPs as placeholders; this script replaces them with the REAL per-region egress
# IPs, validates with `nft -c`, and installs/applies the result. Re-run after
# any `fly ips allocate-egress`.
#
# Egress IPs are taken from (in order):
#   1. CLI args:    apply-origin-firewall.sh 1.2.3.4 5.6.7.8 2a09:0:1::1
#   2. else `fly ips list --json` (Type=egress), if the fly CLI is on PATH.
#
# Options:
#   --check     render + validate only; do not install or apply (safe anywhere)
#   --print     print the rendered ruleset to stdout and exit
#   -h, --help  this help
#
# Applying requires root (writes /etc/nftables.d and loads the ruleset).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TEMPLATE="${SCRIPT_DIR}/katpool-stratum.nft"
DEST=/etc/nftables.d/katpool-stratum.nft

mode=apply
ips=()

die() {
    echo "apply-origin-firewall: $*" >&2
    exit 1
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --check) mode=check; shift ;;
        --print) mode=print; shift ;;
        -h | --help)
            sed -n '2,20p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        -*) die "unknown option: $1 (use --help)" ;;
        *) ips+=("$1"); shift ;;
    esac
done

[[ -f "${TEMPLATE}" ]] || die "template not found: ${TEMPLATE}"

# ----- Resolve egress IPs --------------------------------------------------
if [[ ${#ips[@]} -eq 0 ]]; then
    command -v fly >/dev/null 2>&1 ||
        die "no IPs given and fly CLI not found; pass egress IPs as args or run \`fly ips list\`"
    command -v python3 >/dev/null 2>&1 || die "python3 required to parse \`fly ips list --json\`"
    echo "==> collecting egress IPs from \`fly ips list --json\`"
    mapfile -t ips < <(
        fly ips list --json |
            python3 -c 'import json,sys
for e in json.load(sys.stdin):
    if str(e.get("Type","")).lower()=="egress" and e.get("Address"):
        print(e["Address"])'
    )
    [[ ${#ips[@]} -gt 0 ]] ||
        die "no egress IPs found via fly; allocate them (\`fly ips allocate-egress -r <region>\`) or pass them as args"
fi

# Split into v4 / v6 (v6 contains a colon).
v4=()
v6=()
for ip in "${ips[@]}"; do
    if [[ "${ip}" == *:* ]]; then v6+=("${ip}"); else v4+=("${ip}"); fi
done
[[ ${#v4[@]} -gt 0 || ${#v6[@]} -gt 0 ]] || die "no usable IPs after parsing"

join() {
    local IFS=,
    echo "$*"
}
v4_csv="$(join "${v4[@]:-}")"
v6_csv="$(join "${v6[@]:-}")"

echo "    fly egress v4: ${v4_csv:-<none>}"
echo "    fly egress v6: ${v6_csv:-<none>}"

# ----- Render: replace each sentinel-delimited elements line ---------------
# In each managed block, drop the old `elements = { ... }` line and, if we have
# IPs for that family, emit a fresh one; an empty family becomes an empty set
# (valid nft — the referencing rule simply never matches).
rendered="$(
    awk -v v4="${v4_csv}" -v v6="${v6_csv}" '
        /# >>> fly_egress_v4/ { print; skip="v4"; next }
        /# <<< fly_egress_v4/ {
            if (v4 != "") print "        elements = { " v4 " }";
            skip=""; print; next
        }
        /# >>> fly_egress_v6/ { print; skip="v6"; next }
        /# <<< fly_egress_v6/ {
            if (v6 != "") print "        elements = { " v6 " }";
            skip=""; print; next
        }
        skip != "" { next }   # swallow the old elements line(s) in the block
        { print }
    ' "${TEMPLATE}"
)"

if [[ "${mode}" == "print" ]]; then
    printf '%s\n' "${rendered}"
    exit 0
fi

# ----- Validate ------------------------------------------------------------
tmp="$(mktemp)"
trap 'rm -f "${tmp}"' EXIT
printf '%s\n' "${rendered}" > "${tmp}"

command -v nft >/dev/null 2>&1 || die "nft (nftables) not found on PATH"
echo "==> validating ruleset (nft -c)"
nft -c -f "${tmp}" || die "ruleset failed validation; not applying"
echo "    ok"

if [[ "${mode}" == "check" ]]; then
    echo "==> --check: validated only, not applied"
    exit 0
fi

# ----- Install + apply -----------------------------------------------------
[[ ${EUID} -eq 0 ]] || die "applying requires root (writes ${DEST} and loads nft)"
install -d -m 0755 "$(dirname "${DEST}")"
install -m 0644 "${tmp}" "${DEST}"
echo "    installed -> ${DEST}"
nft -f "${DEST}"
echo "==> applied. Persist across reboots by including ${DEST} from the host's"
echo "    nftables config (e.g. an \`include\` in /etc/nftables.conf), and re-run"
echo "    this script after any \`fly ips allocate-egress\` change."
