#!/usr/bin/env bash
# On-call paging dry-run (Phase 9): verify a page actually reaches the on-call
# phone. This script exercises the publicly-reachable **last mile** — it
# publishes a clearly-marked TEST page to the ntfy topic with `urgent` priority.
# Confirm it arrives on the phone.
#
# The full path (Alertmanager -> ntfy-alertmanager bridge -> ntfy) is exercised
# separately by firing a synthetic alert into Alertmanager, which is NOT public;
# see docs/runbooks/21-resilience-drills.md for the in-Railway amtool/curl snippet.
set -euo pipefail

NTFY_URL="${KATPOOL_ONCALL_NTFY_URL:?set ntfy base URL, e.g. https://ntfy-stage-testnet-10.up.railway.app}"
NTFY_TOPIC="${KATPOOL_ONCALL_NTFY_TOPIC:?set the alerts topic, e.g. katpool-tn10-alerts}"
NTFY_TOKEN="${KATPOOL_ONCALL_NTFY_TOKEN:?set the ntfy access token (tk_...)}"

echo "Publishing a TEST page to ${NTFY_URL}/${NTFY_TOPIC} (urgent priority)..."
curl -fsS \
    -H "Authorization: Bearer ${NTFY_TOKEN}" \
    -H "Title: katpool on-call paging test" \
    -H "Priority: urgent" \
    -H "Tags: warning,test_tube" \
    -d "On-call paging dry-run $(date -u +%FT%TZ): if you see this, the ntfy last mile works. NOT a real incident." \
    "${NTFY_URL}/${NTFY_TOPIC}" >/dev/null
echo "Published. Confirm the on-call phone received the urgent notification."
