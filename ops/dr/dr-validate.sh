#!/usr/bin/env bash
# Automated DR validation (ADR-0009 / Runbook 10). Proves the backup -> restore
# loop actually works: dumps the source database, restores it into a throwaway
# scratch database, and reconciles the restored copy (schema completeness +
# referential sanity + non-empty core tables). Publishes dr_validator_* to
# VictoriaMetrics via vmauth so `DRValidatorMissed` / `DRValidatorFailed` alert.
#
# Read-only against the source (pg_dump uses a consistent snapshot); writes only
# to the scratch database, which it resets each run. Schedule weekly (systemd
# timer or Railway cron). Exit 0 = pass, non-zero = fail.
#
# Env:
#   KATPOOL_DR_SOURCE_URL    (required) source DB (a read replica is ideal)
#   KATPOOL_DR_SCRATCH_URL   (required) throwaway DB to restore into (its public
#                            schema is DROPped + recreated every run)
#   KATPOOL_DR_VMAUTH_URL / _USER / _PASSWORD  (optional) publish metrics
#   KATPOOL_DR_NETWORK       metric label (default: unknown)
set -euo pipefail

SRC_URL="${KATPOOL_DR_SOURCE_URL:?set KATPOOL_DR_SOURCE_URL}"
SCRATCH_URL="${KATPOOL_DR_SCRATCH_URL:?set KATPOOL_DR_SCRATCH_URL (a throwaway DB)}"
VMAUTH_URL="${KATPOOL_DR_VMAUTH_URL:-}"
VMAUTH_USER="${KATPOOL_DR_VMAUTH_USER:-}"
VMAUTH_PASSWORD="${KATPOOL_DR_VMAUTH_PASSWORD:-}"
NETWORK="${KATPOOL_DR_NETWORK:-unknown}"
# Core tables that must be present and non-empty in any healthy restore.
CORE_TABLES="${KATPOOL_DR_CORE_TABLES:-wallet share share_allocation block payout_cycle}"

log() { printf '%s %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$*"; }

push() { # name value
    [ -n "$VMAUTH_URL" ] || return 0
    printf '%s{network="%s"} %s\n' "$1" "$NETWORK" "$2" \
        | curl -fsS -u "${VMAUTH_USER}:${VMAUTH_PASSWORD}" --data-binary @- \
            "${VMAUTH_URL}/api/v1/import/prometheus" >/dev/null 2>&1 \
        || log "WARN: failed to push metric $1"
}

fail() {
    log "DR VALIDATION FAILED: $*"
    push dr_validator_success 0
    exit 1
}

start=$(date +%s)
dump="$(mktemp "${TMPDIR:-/tmp}/katpool-dr.XXXXXX.dump")"
trap 'rm -f "$dump"' EXIT

log "DR validation start (network=${NETWORK})"

# 1. Backup — consistent custom-format dump of the source.
pg_dump --format=custom --no-owner --no-privileges --file="$dump" "$SRC_URL" \
    || fail "pg_dump of source failed"
log "dumped source ($(du -h "$dump" | cut -f1))"

# 2. Restore — reset the scratch schema, then restore into it.
psql "$SCRATCH_URL" -v ON_ERROR_STOP=1 -q \
    -c "DROP SCHEMA IF EXISTS public CASCADE; CREATE SCHEMA public;" \
    || fail "could not reset scratch schema"
pg_restore --no-owner --no-privileges --exit-on-error --dbname="$SCRATCH_URL" "$dump" \
    || fail "pg_restore into scratch failed"
log "restored into scratch"

# 3. Reconcile — schema completeness (every source table present in restored)
#    + core tables non-empty + a referential-integrity sanity join. Exact
#    live-source row equality is intentionally NOT asserted: the source keeps
#    writing after the snapshot, so equality is impossible by construction.
src_tables="$(psql "$SRC_URL" -tAc "select table_name from information_schema.tables where table_schema='public' and table_type='BASE TABLE' order by 1")"
dst_tables="$(psql "$SCRATCH_URL" -tAc "select table_name from information_schema.tables where table_schema='public' and table_type='BASE TABLE' order by 1")"
missing="$(comm -23 <(echo "$src_tables") <(echo "$dst_tables"))"
[ -z "$missing" ] || fail "tables missing from restore: $(echo "$missing" | tr '\n' ' ')"
log "schema complete ($(echo "$src_tables" | wc -w | tr -d ' ') tables restored)"

for t in $CORE_TABLES; do
    n="$(psql "$SCRATCH_URL" -tAc "select count(*) from public.\"$t\"" 2>/dev/null || echo 0)"
    [ "${n:-0}" -gt 0 ] || fail "core table '$t' is empty in the restore"
done
log "core tables non-empty"

# Referential sanity: a share's wallet must exist in the restored copy.
orphans="$(psql "$SCRATCH_URL" -tAc \
    "select count(*) from public.share s left join public.wallet w on w.id = s.wallet_id where w.id is null" 2>/dev/null || echo "ERR")"
[ "$orphans" = "0" ] || fail "referential integrity broken in restore (orphan shares: $orphans)"
log "referential integrity intact"

dur=$(( $(date +%s) - start ))
log "DR VALIDATION OK (${dur}s)"
push dr_validator_success 1
push dr_validator_duration_seconds "$dur"
push dr_validator_last_success_timestamp_seconds "$(date +%s)"
