#!/usr/bin/env bash
# Phase 1 close-out smoke against testnet-10.
#
# Drives a 60s CPU-miner load against a running bridge instance and
# reports the Phase 1 acceptance counts:
#
#   - Bridge cold-boot time      ≤ 5 s
#   - ks_valid_share_counter     ≥ 100
#   - ks_blocks_mined            ≥ 1
#
# Pre-requisites (the script verifies and exits non-zero on missing):
#   - The bridge binary `katpool-stratum-bridge` is on PATH or pointed
#     at by $KATPOOL_BRIDGE_BIN.
#   - A kaspad-testnet-10 endpoint is reachable. Set
#     $KASPAD_TESTNET10_GRPC (default: 127.0.0.1:16210).
#   - A CPU stratum miner. We default to the upstream rusty-kaspa
#     `kaspad-miner` if installed, else $KATPOOL_TESTNET10_MINER must
#     point at a stratum-capable miner binary.
#   - `curl` and `jq` on PATH for metric scraping.
#
# Outputs:
#   - JSON line on stdout summarising the result
#   - Non-zero exit code if any acceptance criterion fails

set -euo pipefail

# ---------- Configuration -------------------------------------------
BRIDGE_BIN=${KATPOOL_BRIDGE_BIN:-katpool-stratum-bridge}
BRIDGE_CONFIG=${KATPOOL_BRIDGE_CONFIG:-./config.yaml}
KASPAD=${KASPAD_TESTNET10_GRPC:-127.0.0.1:16210}
STRATUM_HOST=${KATPOOL_TESTNET10_STRATUM_HOST:-127.0.0.1}
STRATUM_PORT=${KATPOOL_TESTNET10_STRATUM_PORT:-5555}
PROM_PORT=${KATPOOL_TESTNET10_PROM_PORT:-2114}
MINER_BIN=${KATPOOL_TESTNET10_MINER:-kaspa-miner}
MINER_WALLET=${KATPOOL_TESTNET10_WALLET:-}
DURATION_SECS=${KATPOOL_TESTNET10_DURATION:-60}
EXPECT_MIN_SHARES=${KATPOOL_TESTNET10_MIN_SHARES:-100}
EXPECT_MIN_BLOCKS=${KATPOOL_TESTNET10_MIN_BLOCKS:-1}
BOOT_TIME_BUDGET_SECS=${KATPOOL_TESTNET10_BOOT_BUDGET:-5}

# ---------- Sanity checks -------------------------------------------
for binary in curl jq "${BRIDGE_BIN}" "${MINER_BIN}"; do
    if ! command -v "${binary}" >/dev/null 2>&1; then
        echo "missing dependency: ${binary}" >&2
        exit 2
    fi
done

if [[ -z "${MINER_WALLET}" ]]; then
    echo "set KATPOOL_TESTNET10_WALLET to a kaspatest: address" >&2
    exit 2
fi

# ---------- 0. Pre-flight: confirm kaspad-tn10 is at tip ------------
# Running the smoke against a kaspad still in IBD produces opaque
# "block template not available" errors; the bridge boots, the miner
# connects, but no jobs ever appear. We discriminate the post-IBD
# state by tailing the most-recent kaspad-tn10 journal lines for the
# "Accepted N blocks ... via relay" pattern, which fires only when
# kaspad is following the live tip via P2P relay (during IBD the
# log line is "Processed N blocks and N headers", with no "via
# relay" suffix). We only run this check when the configured
# `${KASPAD}` is local (`127.0.0.1:16210`) — when targeting an
# operator-supplied external node we trust the operator's own
# sync verification.
case "${KASPAD}" in
    127.0.0.1:16210|localhost:16210|::1:16210)
        if command -v systemctl >/dev/null && systemctl is-active katpool-kaspad-tn10 >/dev/null 2>&1; then
            echo "[0/4] verifying local katpool-kaspad-tn10 is at live tip..."
            recent=$(journalctl -u katpool-kaspad-tn10 --no-pager -n 200 2>/dev/null | grep -c 'Accepted [0-9]\+ blocks .* via relay' || true)
            if [[ "${recent}" -lt 1 ]]; then
                echo "kaspad-tn10 has no recent 'Accepted ... via relay' line in the last 200 journal entries." >&2
                echo "This usually means IBD is still in progress. Wait for sync then re-run." >&2
                echo "  watch:  journalctl -fu katpool-kaspad-tn10" >&2
                exit 1
            fi
            echo "    kaspad-tn10 is at tip (${recent} recent relay-block lines)"
        fi
        ;;
esac

# ---------- 1. Cold-boot the bridge ---------------------------------
echo "[1/4] cold-booting bridge against ${KASPAD}..."
log_file=$(mktemp /tmp/katpool-smoke-XXXXXX.log)
trap 'kill ${bridge_pid:-0} 2>/dev/null || true; rm -f "${log_file}"' EXIT

t_start=$(date +%s%N)
"${BRIDGE_BIN}" \
    --config "${BRIDGE_CONFIG}" \
    --node-mode external \
    -- \
    >> "${log_file}" 2>&1 &
bridge_pid=$!

# Wait for the stratum port to start accepting connections.
booted=false
for _ in $(seq 1 50); do
    if (echo > "/dev/tcp/${STRATUM_HOST}/${STRATUM_PORT}") 2>/dev/null; then
        booted=true
        break
    fi
    sleep 0.1
done
t_boot_end=$(date +%s%N)
boot_secs=$(awk "BEGIN { printf \"%.2f\", (${t_boot_end} - ${t_start}) / 1e9 }")

if [[ "${booted}" != "true" ]]; then
    echo "bridge did not accept stratum within boot budget (${BOOT_TIME_BUDGET_SECS}s)" >&2
    tail -20 "${log_file}" >&2
    exit 1
fi

echo "    boot wall time: ${boot_secs}s (budget ${BOOT_TIME_BUDGET_SECS}s)"

# ---------- 2. Snapshot baseline metric counters --------------------
echo "[2/4] snapshotting baseline metrics from :${PROM_PORT}/metrics..."
baseline=$(curl -fsS "http://127.0.0.1:${PROM_PORT}/metrics" \
    | awk '/^ks_valid_share_counter\{/ { shares += $NF } /^ks_blocks_mined\{/ { blocks += $NF } END { print shares+0, blocks+0 }')
baseline_shares=$(echo "${baseline}" | awk '{print $1}')
baseline_blocks=$(echo "${baseline}" | awk '{print $2}')
echo "    baseline: shares=${baseline_shares}, blocks=${baseline_blocks}"

# ---------- 3. Run the CPU miner for $DURATION_SECS seconds ---------
echo "[3/4] running CPU miner for ${DURATION_SECS}s..."
miner_log=$(mktemp /tmp/katpool-miner-XXXXXX.log)
trap 'kill ${bridge_pid:-0} ${miner_pid:-0} 2>/dev/null || true; rm -f "${log_file}" "${miner_log}"' EXIT

"${MINER_BIN}" \
    --kaspad-address "${KASPAD}" \
    --stratum-address "stratum+tcp://${STRATUM_HOST}:${STRATUM_PORT}" \
    --mining-address "${MINER_WALLET}" \
    >> "${miner_log}" 2>&1 &
miner_pid=$!

sleep "${DURATION_SECS}"

kill "${miner_pid}" 2>/dev/null || true
wait "${miner_pid}" 2>/dev/null || true

# ---------- 4. Snapshot final metrics and report --------------------
echo "[4/4] reading final metrics..."
final=$(curl -fsS "http://127.0.0.1:${PROM_PORT}/metrics" \
    | awk '/^ks_valid_share_counter\{/ { shares += $NF } /^ks_blocks_mined\{/ { blocks += $NF } END { print shares+0, blocks+0 }')
final_shares=$(echo "${final}" | awk '{print $1}')
final_blocks=$(echo "${final}" | awk '{print $2}')

delta_shares=$(awk "BEGIN { printf \"%.0f\", ${final_shares} - ${baseline_shares} }")
delta_blocks=$(awk "BEGIN { printf \"%.0f\", ${final_blocks} - ${baseline_blocks} }")

# ---------- 5. Pass / fail ------------------------------------------
boot_ok=$(awk "BEGIN { if (${boot_secs} <= ${BOOT_TIME_BUDGET_SECS}) print 1; else print 0 }")
shares_ok=$([[ "${delta_shares}" -ge "${EXPECT_MIN_SHARES}" ]] && echo 1 || echo 0)
blocks_ok=$([[ "${delta_blocks}" -ge "${EXPECT_MIN_BLOCKS}" ]] && echo 1 || echo 0)

pass="true"
if [[ "${boot_ok}" -eq 0 ]] || [[ "${shares_ok}" -eq 0 ]] || [[ "${blocks_ok}" -eq 0 ]]; then
    pass="false"
fi

jq -n \
    --arg ts "$(date -u +%FT%TZ)" \
    --argjson boot_secs "${boot_secs}" \
    --argjson boot_budget "${BOOT_TIME_BUDGET_SECS}" \
    --argjson delta_shares "${delta_shares}" \
    --argjson delta_blocks "${delta_blocks}" \
    --argjson expect_min_shares "${EXPECT_MIN_SHARES}" \
    --argjson expect_min_blocks "${EXPECT_MIN_BLOCKS}" \
    --arg pass "${pass}" \
    '{
        ts: $ts,
        result: ($pass == "true"),
        boot: { secs: $boot_secs, budget: $boot_budget, ok: ($boot_secs <= $boot_budget) },
        shares: { observed: $delta_shares, required: $expect_min_shares, ok: ($delta_shares >= $expect_min_shares) },
        blocks: { observed: $delta_blocks, required: $expect_min_blocks, ok: ($delta_blocks >= $expect_min_blocks) }
    }'

[[ "${pass}" == "true" ]]
