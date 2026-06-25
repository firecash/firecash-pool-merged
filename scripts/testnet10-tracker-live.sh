#!/usr/bin/env bash
# Phase 3 M3c — live exercise of the maturity tracker against the
# operator's testnet-10 kaspad-tn10.
#
# Stands up a throwaway Docker Postgres, runs the new schema's
# migrations, seeds a known testnet block into the DB
# (whichever the operator picks), runs the
# `accountant-tracker-runner` binary in the background, and
# tails its logs while the tracker observes the seeded block
# transition through `submitted_to_node` → `confirmed_blue` →
# `matured` (or `orphaned`).
#
# Required environment:
#   KASPAD_GRPC_URL          — operator's testnet kaspad gRPC URL
#                              (e.g. grpc://127.0.0.1:16210)
#   KATPOOL_POOL_ADDRESS     — kaspa-testnet address whose
#                              coinbase outputs count as pool
#                              revenue (= the address that mined
#                              the seeded block)
#   KATPOOL_SEED_BLOCK_HASH  — 64-char hex hash of the testnet
#                              block to seed; pick one whose
#                              blue-score depth ≥ 100 below the
#                              current sink so it matures during
#                              the test
#   KATPOOL_SEED_DAA_SCORE   — DAA score of the seeded block
#
# Optional:
#   KATPOOL_RUN_FOR_SECS     — how long to keep the tracker
#                              running before tearing down
#                              (default: 120)
#   KATPOOL_INSTANCE_ID      — instance label in logs and metrics
#                              (default: tracker-live)
#   KATPOOL_OUTPUT_DIR       — artefact dir
#                              (default: ./tracker-evidence/<UTC>)
#
# Outputs (written into KATPOOL_OUTPUT_DIR):
#   tracker.log     — runner stderr (tracing events)
#   db-final.txt    — final block-table row + audit-log entries
#                     for the seeded block
#   manifest.json   — git rev, binary path/sha256, env summary,
#                     timestamps, exit code
#
# Exit code:
#   0  tracker observed the seeded block reach `matured`
#   2  tracker exited with an error
#   3  observation window elapsed without the block maturing
#      (operator decides whether this is a kaspad issue or a
#      depth-budget issue)

set -euo pipefail

# -------- arg parsing ----------------------------------------------
for arg in "$@"; do
  case "$arg" in
    --help|-h)
      sed -n '1,55p' "$0"
      exit 0
      ;;
    *)
      echo "Unknown argument: $arg" >&2
      exit 1
      ;;
  esac
done

require_var() {
  local name="$1"
  if [[ -z "${!name:-}" ]]; then
    echo "FATAL: $name is required (see docs/runbooks/15-testnet10-tracker-live.md)" >&2
    exit 1
  fi
}
require_var KASPAD_GRPC_URL
require_var KATPOOL_POOL_ADDRESS
require_var KATPOOL_SEED_BLOCK_HASH
require_var KATPOOL_SEED_DAA_SCORE

RUN_FOR_SECS=${KATPOOL_RUN_FOR_SECS:-120}
INSTANCE_ID=${KATPOOL_INSTANCE_ID:-tracker-live}
STAMP=$(date -u +%Y-%m-%dT%H-%M-%SZ)
OUTDIR=${KATPOOL_OUTPUT_DIR:-./tracker-evidence/${STAMP}-${INSTANCE_ID}}
mkdir -p "$OUTDIR"

for tool in docker psql jq sha256sum; do
  command -v "$tool" >/dev/null 2>&1 || {
    echo "FATAL: required tool $tool not on PATH" >&2; exit 1; }
done

REPO_ROOT=$(git -C "$(dirname "$0")/.." rev-parse --show-toplevel 2>/dev/null \
    || echo "$(dirname "$0")/..")
cd "$REPO_ROOT"

GIT_REV=$(git rev-parse HEAD 2>/dev/null || echo unknown)

# -------- Build + Docker Postgres ----------------------------------
echo "==> compiling accountant-tracker-runner (release)" >&2
cargo build --release --bin accountant-tracker-runner -p accountant >/dev/null

BIN=$REPO_ROOT/target/release/accountant-tracker-runner
BIN_SHA=$(sha256sum "$BIN" | awk '{print $1}')

PG_CONTAINER=katpool-tracker-pg-$$
PG_PORT=$(python3 -c 'import socket; s=socket.socket(); s.bind(("",0)); print(s.getsockname()[1]); s.close()')

cleanup() {
  echo "==> cleanup: removing Postgres container" >&2
  docker rm -f "$PG_CONTAINER" >/dev/null 2>&1 || true
}
trap cleanup EXIT

echo "==> starting Docker Postgres on port $PG_PORT" >&2
docker run --rm -d \
  --name "$PG_CONTAINER" \
  -e POSTGRES_PASSWORD=postgres \
  -e POSTGRES_DB=katpool_live \
  -p "$PG_PORT:5432" \
  postgres:17-alpine >/dev/null

# Wait for postgres to accept connections.
for i in {1..30}; do
  if docker exec "$PG_CONTAINER" pg_isready -U postgres -d katpool_live >/dev/null 2>&1; then
    break
  fi
  sleep 1
done

DB_URL="postgres://postgres:postgres@127.0.0.1:${PG_PORT}/katpool_live"

# Apply migrations via sqlx-cli if available, else cat the files.
echo "==> applying migrations" >&2
for f in $(ls crates/katpool-db/migrations/*.sql | sort); do
  PGPASSWORD=postgres psql -h 127.0.0.1 -p "$PG_PORT" -U postgres -d katpool_live -v ON_ERROR_STOP=1 -f "$f" >/dev/null
done

# -------- seed block + finder wallet/worker rows -------------------
echo "==> seeding block ${KATPOOL_SEED_BLOCK_HASH}" >&2
# Synthesise a finder wallet/worker for the seeded block. These
# don't need to correspond to anything real — the tracker doesn't
# care, it only cares about the block's status.
SEED_WALLET=$KATPOOL_POOL_ADDRESS
SEED_HASH_BYTES=$(printf '\\x%s' "$KATPOOL_SEED_BLOCK_HASH")
PGPASSWORD=postgres psql -h 127.0.0.1 -p "$PG_PORT" -U postgres -d katpool_live -v ON_ERROR_STOP=1 <<SQL >/dev/null
INSERT INTO wallet (address, network) VALUES ('${SEED_WALLET}', 'testnet-10')
ON CONFLICT (address) DO NOTHING;
INSERT INTO worker (wallet_id, name)
SELECT id, 'tracker-live' FROM wallet WHERE address = '${SEED_WALLET}'
ON CONFLICT (wallet_id, name) DO NOTHING;
INSERT INTO block
    (hash, finder_wallet_id, finder_worker_id, daa_score, nonce, correlation_id, status, submitted_at)
SELECT
    decode('${KATPOOL_SEED_BLOCK_HASH}', 'hex'),
    w.id, wk.id,
    ${KATPOOL_SEED_DAA_SCORE}, 0,
    gen_random_uuid(),
    'submitted_to_node',
    now()
  FROM wallet w
  JOIN worker wk ON wk.wallet_id = w.id
 WHERE w.address = '${SEED_WALLET}'
   AND wk.name = 'tracker-live'
ON CONFLICT (hash) DO NOTHING;
SQL

# -------- run the tracker ------------------------------------------
echo "==> running tracker for ${RUN_FOR_SECS}s" >&2
STARTED=$(date -u --iso-8601=seconds)

set +e
KATPOOL_DATABASE_URL="$DB_URL" \
KASPAD_GRPC_URL="$KASPAD_GRPC_URL" \
KATPOOL_POOL_ADDRESS="$KATPOOL_POOL_ADDRESS" \
KATPOOL_INSTANCE_ID="$INSTANCE_ID" \
KATPOOL_MATURITY_POLL_SECS=5 \
RUST_LOG=info,accountant=debug \
timeout "${RUN_FOR_SECS}" "$BIN" \
  2> "$OUTDIR/tracker.log"
RUNNER_EXIT=$?
set -e

# `timeout` returns 124 on SIGTERM; treat that as "we ran long
# enough" rather than failure.
[[ $RUNNER_EXIT -eq 124 ]] && RUNNER_EXIT=0

FINISHED=$(date -u --iso-8601=seconds)

# -------- capture final DB state -----------------------------------
PGPASSWORD=postgres psql -h 127.0.0.1 -p "$PG_PORT" -U postgres -d katpool_live -At -F $'\t' <<SQL > "$OUTDIR/db-final.txt"
SELECT 'block_row',
       encode(hash, 'hex'), status::text, blue_score, daa_score,
       submitted_at, confirmed_at, matured_at, miner_reward_sompi
  FROM block
 WHERE hash = decode('${KATPOOL_SEED_BLOCK_HASH}', 'hex');
SELECT '---audit-log---' AS sep;
SELECT id, occurred_at, actor, action, payload::text
  FROM audit_log
 WHERE subject_type = 'block'
 ORDER BY occurred_at ASC;
SELECT '---allocations---' AS sep;
SELECT count(*) AS allocation_count FROM share_allocation;
SQL

FINAL_STATUS=$(PGPASSWORD=postgres psql -h 127.0.0.1 -p "$PG_PORT" -U postgres -d katpool_live -At -c \
  "SELECT status::text FROM block WHERE hash = decode('${KATPOOL_SEED_BLOCK_HASH}', 'hex');")

# -------- manifest -------------------------------------------------
DISPOSITION="unknown"
case "$FINAL_STATUS" in
  matured)         DISPOSITION="matured" ;;
  orphaned)        DISPOSITION="orphaned" ;;
  confirmed_blue)  DISPOSITION="still_in_flight" ;;
  submitted_to_node) DISPOSITION="never_confirmed" ;;
  *) DISPOSITION="unexpected_status:${FINAL_STATUS}" ;;
esac

jq -n \
  --arg git "$GIT_REV" \
  --arg bin "$BIN" \
  --arg bin_sha "$BIN_SHA" \
  --arg started "$STARTED" \
  --arg finished "$FINISHED" \
  --arg kaspad "$KASPAD_GRPC_URL" \
  --arg pool_addr "$KATPOOL_POOL_ADDRESS" \
  --arg seed_hash "$KATPOOL_SEED_BLOCK_HASH" \
  --arg seed_daa "$KATPOOL_SEED_DAA_SCORE" \
  --argjson exit "$RUNNER_EXIT" \
  --arg final_status "$FINAL_STATUS" \
  --arg disposition "$DISPOSITION" \
  --argjson run_for_secs "$RUN_FOR_SECS" \
  '{
    schema: "accountant-tracker-live.manifest/v1",
    git_rev: $git,
    binary: { path: $bin, sha256: $bin_sha },
    timestamps: { started: $started, finished: $finished },
    config: {
      kaspad_url: $kaspad,
      pool_address: $pool_addr,
      seed_block_hash: $seed_hash,
      seed_daa_score: $seed_daa,
      run_for_secs: $run_for_secs
    },
    runner_exit_code: $exit,
    final_block_status: $final_status,
    disposition: $disposition
  }' \
  > "$OUTDIR/manifest.json"

echo "==> live test complete" >&2
echo "    final_block_status=$FINAL_STATUS  disposition=$DISPOSITION  runner_exit=$RUNNER_EXIT" >&2
echo "    artefacts in $OUTDIR" >&2
ls -la "$OUTDIR" >&2

case "$DISPOSITION" in
  matured)        exit 0 ;;
  still_in_flight|never_confirmed) exit 3 ;;
  *)              exit 2 ;;
esac
