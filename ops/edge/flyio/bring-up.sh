#!/usr/bin/env bash
# Idempotent fly.io anycast stratum edge bring-up (ADR-0022). Wraps the manual
# steps in README.md so a re-run is safe (it skips anything already present).
# Requires flyctl, authenticated (`fly auth login`). The actual live bring-up is
# operator-gated — this only orchestrates the documented commands and prompts
# before each mutating step.
#
# Usage:
#   KATPOOL_ORIGIN_HOST=kas-origin.katpool.com ops/edge/flyio/bring-up.sh
#
# Env:
#   KATPOOL_ORIGIN_HOST   (required) DNS name resolving to the NetCup origin's
#                         REAL IP — never the public/anycast name (avoid a loop).
#   KATPOOL_EDGE_APP      fly app name              (default: katpool-edge)
#   KATPOOL_EDGE_REGIONS  space-separated fly codes (default: the 7 legacy regions)
#   ASSUME_YES=1          skip confirmation prompts (for CI/non-interactive runs)
set -euo pipefail

APP="${KATPOOL_EDGE_APP:-katpool-edge}"
ORIGIN_HOST="${KATPOOL_ORIGIN_HOST:-}"
REGIONS="${KATPOOL_EDGE_REGIONS:-sjc iad fra sin hkg gru syd}"
HERE="$(cd "$(dirname "$0")" && pwd)"

command -v fly >/dev/null 2>&1 || { echo "error: flyctl not found (https://fly.io/docs/flyctl/install/)" >&2; exit 1; }
fly auth whoami >/dev/null 2>&1 || { echo "error: not logged in — run 'fly auth login'" >&2; exit 1; }
[ -n "$ORIGIN_HOST" ] || { echo "error: set KATPOOL_ORIGIN_HOST (origin real-IP DNS name)" >&2; exit 1; }

confirm() {
    [ "${ASSUME_YES:-0}" = "1" ] && return 0
    read -r -p "$1 [y/N] " ans
    [ "$ans" = "y" ] || [ "$ans" = "Y" ]
}

echo "==> fly edge bring-up: app=$APP origin=$ORIGIN_HOST regions=[$REGIONS]"

# 1. App ---------------------------------------------------------------------
if fly apps list 2>/dev/null | grep -qw "$APP"; then
    echo "    app '$APP' already exists"
elif confirm "create fly app '$APP'?"; then
    fly apps create "$APP"
fi

# 2. Origin secret (idempotent — fly re-sets without error) ------------------
echo "    setting KATPOOL_ORIGIN_HOST secret"
fly secrets set "KATPOOL_ORIGIN_HOST=$ORIGIN_HOST" -a "$APP" >/dev/null

# 3. Dedicated anycast IPs (skip if already allocated) -----------------------
ips="$(fly ips list -a "$APP" 2>/dev/null || true)"
if echo "$ips" | grep -qiE '\bv4\b.*public'; then
    echo "    dedicated anycast IPv4 already allocated"
elif confirm "allocate a DEDICATED anycast IPv4 (required for raw TCP)?"; then
    fly ips allocate-v4 -a "$APP"
fi
echo "$ips" | grep -qiE '\bv6\b' || fly ips allocate-v6 -a "$APP" || true

# 4. Stable per-region egress IPs (for the origin nftables allowlist) --------
for r in $REGIONS; do
    if echo "$ips" | grep -qi "egress.*$r"; then
        echo "    egress IP for $r already allocated"
    else
        fly ips allocate-egress -r "$r" -a "$APP" || echo "    warn: egress allocate for $r failed (retry manually)"
    fi
done

# 5. Deploy + spread across regions ------------------------------------------
if confirm "deploy and scale to [$REGIONS]?"; then
    ( cd "$HERE" && fly deploy -a "$APP" )
    count="$(echo "$REGIONS" | wc -w | tr -d ' ')"
    fly scale count "$count" --region "$(echo "$REGIONS" | tr ' ' ',')" -a "$APP"
fi

# 6. Summary -----------------------------------------------------------------
echo "==> done. Current IPs (feed the egress IPs to the origin firewall):"
fly ips list -a "$APP"
cat <<EOF

Next steps (origin + DNS):
  1. On the NetCup origin, allowlist the egress IPs above:
       sudo ops/edge/flyio/nftables/apply-origin-firewall.sh <egress-ip> [<egress-ip> ...]
     and set KATPOOL_STRATUM_PROXY_PROTOCOL=true in the pool env.
  2. Point every *.katpool.com stratum hostname's A/AAAA at the anycast IP(s) above.
  3. Run the load + failover checks in README.md "Load + failover test".
EOF
