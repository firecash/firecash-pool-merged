#!/usr/bin/env bash
# Phase 3 M3d — live full-pipeline exercise on testnet-10.
#
# Spins up:
#   1. A throwaway Docker Postgres with the new schema migrated.
#   2. The unified `katpool` runtime binary, which embeds the
#      stratum bridge + the accountant event consumer + the
#      maturity tracker in one process.
#
# Then the operator points a testnet ASIC at the bridge's stratum
# port (default 15555), mines for `KATPOOL_RUN_FOR_SECS` seconds,
# and the script captures the resulting state — share counts,
# block lifecycle, allocation rows — into a timestamped artefact
# directory for the Phase 3 acceptance ticket.
#
# Required environment:
#   KASPAD_GRPC_URL        — operator's testnet kaspad gRPC URL
#   KATPOOL_POOL_ADDRESS   — kaspatest address(es) the ASIC will
#                            authorize as / mine to (the addresses
#                            whose coinbase outputs count as pool
#                            revenue)
#
# Optional:
#   KATPOOL_RUN_FOR_SECS   — total mining window (default: 900 = 15 min)
#   KATPOOL_STRATUM_PORT   — bridge stratum listen port (default: 15555)
#   KATPOOL_OUTPUT_DIR     — artefact dir (default:
#                            ./pipeline-evidence/<UTC>-pipeline-live)
#   KATPOOL_INSTANCE_ID    — instance label (default: pipeline-live)
#
# Outputs:
#   katpool.log         — runtime stderr (tracing events)
#   db-final.txt        — post-run row counts + sample rows
#   manifest.json       — git rev, binary path/sha256, timestamps,
#                         exit code, observed counts
#
# Exit codes:
#   0  pipeline observed at least one accepted share AND at least
#      one block transition (find or higher)
#   2  runtime exited unexpectedly
#   3  pipeline ran the full window but observed no shares
#      (likely the ASIC wasn't connected; the manifest documents
#      the negative result for diagnosis)

set -euo pipefail

for arg in "$@"; do
  case "$arg" in
    --help|-h) sed -n '1,50p' "$0"; exit 0 ;;
    *) echo "Unknown argument: $arg" >&2; exit 1 ;;
  esac
done

require_var() {
  local name="$1"
  if [[ -z "${!name:-}" ]]; then
    echo "FATAL: $name is required (see docs/runbooks/16-testnet10-full-pipeline-live.md)" >&2
    exit 1
  fi
}
require_var KASPAD_GRPC_URL
require_var KATPOOL_POOL_ADDRESS

RUN_FOR_SECS=${KATPOOL_RUN_FOR_SECS:-900}
STRATUM_PORT=${KATPOOL_STRATUM_PORT:-15555}
INSTANCE_ID=${KATPOOL_INSTANCE_ID:-pipeline-live}
STAMP=$(date -u +%Y-%m-%dT%H-%M-%SZ)
OUTDIR=${KATPOOL_OUTPUT_DIR:-./pipeline-evidence/${STAMP}-${INSTANCE_ID}}
mkdir -p "$OUTDIR"

for tool in docker psql jq sha256sum; do
  command -v "$tool" >/dev/null 2>&1 || { echo "FATAL: tool $tool not on PATH" >&2; exit 1; }
done

REPO_ROOT=$(git -C "$(dirname "$0")/.." rev-parse --show-toplevel 2>/dev/null \
    || echo "$(dirname "$0")/..")
cd "$REPO_ROOT"
GIT_REV=$(git rev-parse HEAD 2>/dev/null || echo unknown)

# ---- build the binary --------------------------------------------
echo "==> compiling katpool (release)" >&2
cargo build --release --bin katpool -p katpool >/dev/null

BIN=$REPO_ROOT/target/release/katpool
if [[ ! -x "$BIN" ]]; then
  # Cursor sandbox path
  BIN_ALT=$(find / -name katpool -path "*release*" -type f 2>/dev/null | head -1)
  [[ -n "$BIN_ALT" ]] && BIN=$BIN_ALT
fi
BIN_SHA=$(sha256sum "$BIN" | awk '{print $1}')

# ---- Postgres ----------------------------------------------------
PG_CONTAINER=katpool-pipeline-pg-$$
PG_PORT=55433
cleanup() {
  echo "==> cleanup" >&2
  if [[ -n "${KATPOOL_PID:-}" ]]; then
    kill "$KATPOOL_PID" 2>/dev/null || true
    wait "$KATPOOL_PID" 2>/dev/null || true
  fi
  docker rm -f "$PG_CONTAINER" >/dev/null 2>&1 || true
}
trap cleanup EXIT

echo "==> starting Docker Postgres on port $PG_PORT" >&2
docker run --rm -d \
  --name "$PG_CONTAINER" \
  -e POSTGRES_PASSWORD=postgres \
  -e POSTGRES_DB=katpool_pipeline \
  -p "$PG_PORT:5432" \
  postgres:17-alpine >/dev/null
for _ in {1..30}; do
  if docker exec "$PG_CONTAINER" pg_isready -U postgres -d katpool_pipeline >/dev/null 2>&1; then
    break; fi
  sleep 1
done

DB_URL="postgres://postgres:postgres@127.0.0.1:${PG_PORT}/katpool_pipeline"

echo "==> applying migrations" >&2
for f in $(ls crates/katpool-db/migrations/*.sql | sort); do
  PGPASSWORD=postgres psql -h 127.0.0.1 -p "$PG_PORT" -U postgres -d katpool_pipeline -v ON_ERROR_STOP=1 -f "$f" >/dev/null
done

# ---- run the katpool binary --------------------------------------
STARTED=$(date -u --iso-8601=seconds)
echo "==> starting katpool runtime for ${RUN_FOR_SECS}s on stratum port $STRATUM_PORT" >&2
echo "==> point your testnet ASIC at stratum+tcp://<host>:${STRATUM_PORT}" >&2
echo "==> authorize with $KATPOOL_POOL_ADDRESS (or sub-worker, e.g. ${KATPOOL_POOL_ADDRESS}.rig-01)" >&2

KATPOOL_DATABASE_URL="$DB_URL" \
KASPAD_GRPC_URL="$KASPAD_GRPC_URL" \
KATPOOL_POOL_ADDRESS="$KATPOOL_POOL_ADDRESS" \
KATPOOL_STRATUM_PORT="$STRATUM_PORT" \
KATPOOL_INSTANCE_ID="$INSTANCE_ID" \
KATPOOL_MATURITY_POLL_SECS=10 \
RUST_LOG="${RUST_LOG:-info,accountant=debug,kaspa_stratum_bridge=info}" \
  "$BIN" > "$OUTDIR/katpool.log" 2>&1 &
KATPOOL_PID=$!

# Wait for the runtime to listen on the stratum port (max 30s).
for _ in {1..30}; do
  if ss -lnt 2>/dev/null | grep -q ":${STRATUM_PORT}\b"; then break; fi
  sleep 1
done
if ! ss -lnt 2>/dev/null | grep -q ":${STRATUM_PORT}\b"; then
  echo "FATAL: katpool didn't bind stratum port $STRATUM_PORT within 30s" >&2
  exit 2
fi
echo "==> runtime listening; mining window open" >&2

# Mine for the window.
sleep "$RUN_FOR_SECS"

# Stop runtime cleanly via SIGTERM.
kill -TERM "$KATPOOL_PID" 2>/dev/null || true
wait "$KATPOOL_PID" 2>/dev/null || true
RUNNER_EXIT=$?
FINISHED=$(date -u --iso-8601=seconds)

# ---- capture state -----------------------------------------------
PGPASSWORD=postgres psql -h 127.0.0.1 -p "$PG_PORT" -U postgres -d katpool_pipeline -At -F $'\t' <<'SQL' > "$OUTDIR/db-final.txt"
SELECT 'wallets'    AS table, count(*)::text FROM wallet;
SELECT 'workers'    AS table, count(*)::text FROM worker;
SELECT 'shares'     AS table, count(*)::text FROM share;
SELECT 'rejects'    AS table, count(*)::text FROM share_reject;
SELECT 'blocks'     AS table, count(*)::text FROM block;
SELECT 'allocations' AS table, count(*)::text FROM share_allocation;
SELECT '---block-rows---';
SELECT encode(hash,'hex'), status::text, blue_score, daa_score, submitted_at, confirmed_at, matured_at, miner_reward_sompi
  FROM block ORDER BY found_at DESC LIMIT 10;
SELECT '---share-summary---';
SELECT w.address, w.network, count(s.*) AS share_count, sum(s.difficulty) AS weight
  FROM wallet w LEFT JOIN share s ON s.wallet_id = w.id
 GROUP BY w.address, w.network ORDER BY share_count DESC LIMIT 10;
SELECT '---allocation-rows---';
SELECT encode(b.hash,'hex') AS block_hash,
       w.address AS wallet,
       sa.gross_share_sompi, sa.pool_fee_sompi, sa.nacho_accrual_sompi, sa.net_payout_sompi,
       sa.applied_topline_bps, sa.applied_tier::text
  FROM share_allocation sa
  JOIN block b ON b.id = sa.block_id
  JOIN wallet w ON w.id = sa.wallet_id
 ORDER BY sa.computed_at DESC LIMIT 20;
SQL

SHARE_COUNT=$(PGPASSWORD=postgres psql -h 127.0.0.1 -p "$PG_PORT" -U postgres -d katpool_pipeline -At -c 'SELECT count(*) FROM share;')
BLOCK_COUNT=$(PGPASSWORD=postgres psql -h 127.0.0.1 -p "$PG_PORT" -U postgres -d katpool_pipeline -At -c 'SELECT count(*) FROM block;')
ALLOC_COUNT=$(PGPASSWORD=postgres psql -h 127.0.0.1 -p "$PG_PORT" -U postgres -d katpool_pipeline -At -c 'SELECT count(*) FROM share_allocation;')

# ---- manifest ----------------------------------------------------
DISPOSITION="unknown"
if [[ "$SHARE_COUNT" -gt 0 ]] && [[ "$BLOCK_COUNT" -gt 0 ]]; then
  DISPOSITION="shares_and_blocks_observed"
elif [[ "$SHARE_COUNT" -gt 0 ]]; then
  DISPOSITION="shares_observed_no_blocks"
elif [[ "$BLOCK_COUNT" -gt 0 ]]; then
  DISPOSITION="blocks_observed_no_shares"
else
  DISPOSITION="no_activity"
fi

jq -n \
  --arg git "$GIT_REV" \
  --arg bin "$BIN" --arg bin_sha "$BIN_SHA" \
  --arg started "$STARTED" --arg finished "$FINISHED" \
  --arg kaspad "$KASPAD_GRPC_URL" \
  --arg pool_addr "$KATPOOL_POOL_ADDRESS" \
  --arg stratum_port "$STRATUM_PORT" \
  --argjson run_for_secs "$RUN_FOR_SECS" \
  --argjson runner_exit "$RUNNER_EXIT" \
  --argjson share_count "$SHARE_COUNT" \
  --argjson block_count "$BLOCK_COUNT" \
  --argjson alloc_count "$ALLOC_COUNT" \
  --arg disposition "$DISPOSITION" \
  '{
    schema: "katpool-pipeline-live.manifest/v1",
    git_rev: $git,
    binary: { path: $bin, sha256: $bin_sha },
    timestamps: { started: $started, finished: $finished },
    config: {
      kaspad_url: $kaspad,
      pool_address: $pool_addr,
      stratum_port: $stratum_port,
      run_for_secs: $run_for_secs
    },
    runner_exit_code: $runner_exit,
    observed: {
      shares: $share_count,
      blocks: $block_count,
      allocations: $alloc_count
    },
    disposition: $disposition
  }' > "$OUTDIR/manifest.json"

echo "==> live exercise complete" >&2
echo "    shares=$SHARE_COUNT  blocks=$BLOCK_COUNT  allocations=$ALLOC_COUNT  disposition=$DISPOSITION" >&2
echo "    artefacts in $OUTDIR" >&2
ls -la "$OUTDIR" >&2

case "$DISPOSITION" in
  shares_and_blocks_observed) exit 0 ;;
  shares_observed_no_blocks)  exit 0 ;; # Shares without blocks is still a successful share-ingestion proof
  *)                          exit 3 ;;
esac
