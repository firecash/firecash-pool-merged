#!/usr/bin/env bash
# Rollback rehearsal (Phase 9/10 gate — cutover-plan.md "rollback rehearsed").
# Proves the unified runtime can be rolled back to the previous binary that
# scripts/deploy.sh preserves (${deploy_dir}/katpool.bak-<ts>).
#
#   --check   (default) non-disruptive: confirm a valid backup exists and print
#             the exact rollback command. Safe on a live host.
#   --execute roll back for real: save the current binary for roll-forward,
#             restore the latest backup, restart, and verify /ready. Disruptive —
#             rehearse on a non-prod VPS or a maintenance window. Prints the
#             roll-forward command on success.
set -euo pipefail

NETWORK="${1:-}"
MODE="${2:---check}"
[ -n "$NETWORK" ] || { echo "usage: $0 <network> [--check|--execute]" >&2; exit 1; }

DEPLOY_DIR="/root/katpool-${NETWORK}"
SERVICE="katpool-${NETWORK}"
BIN="${DEPLOY_DIR}/katpool"
READY_URL="${KATPOOL_ROLLBACK_READY_URL:-http://127.0.0.1:18080/ready}"

latest_bak="$(ls -1t "${BIN}".bak-* 2>/dev/null | head -n1 || true)"
[ -n "$latest_bak" ] || { echo "error: no rollback backup found (${BIN}.bak-*); deploy at least twice first" >&2; exit 1; }

echo "service:        $SERVICE"
echo "current binary: $BIN"
echo "rollback to:    $latest_bak"
file "$latest_bak" | grep -q 'ELF' || { echo "error: backup is not an ELF executable" >&2; exit 1; }
echo "backup validates as an ELF executable: OK"

if [ "$MODE" = "--check" ]; then
    echo "rollback command: cp '$latest_bak' '$BIN' && systemctl restart $SERVICE"
    echo "(check-only — pass --execute to actually roll back)"
    exit 0
fi
[ "$MODE" = "--execute" ] || { echo "error: unknown mode '$MODE' (--check|--execute)" >&2; exit 1; }

fwd="${BIN}.rollforward-$(date -u +%Y%m%dT%H%M%SZ)"
cp -p "$BIN" "$fwd"
echo "saved current binary for roll-forward: $fwd"
cp -p "$latest_bak" "$BIN"
systemctl restart "$SERVICE"
echo "rolled back; waiting for /ready ..."
ok=0
for _ in $(seq 1 30); do
    if curl -fsS -o /dev/null --max-time 2 "$READY_URL"; then ok=1; break; fi
    sleep 2
done
if [ "$ok" = 1 ]; then
    echo "ROLLBACK OK: $SERVICE is ready on the previous binary"
else
    echo "ROLLBACK WARN: /ready not green within 60s — investigate" >&2
fi
echo "roll forward when finished: cp '$fwd' '$BIN' && systemctl restart $SERVICE"
