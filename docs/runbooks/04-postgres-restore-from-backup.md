# Runbook 04 — Postgres restore from backup

## When to use this runbook

- The primary postgres database is corrupted, unrecoverable, or
  the VPS is gone
- A logical-data incident requires point-in-time recovery
- The automated DR validator failed and you need to investigate
  with a real interactive restore

## Prerequisites

- B2 credentials available (in `ops/secrets/secrets.sops.yaml`)
- `pgbackrest` installed locally or on the target VPS
- Target Postgres 17 binary available
- Empty target data directory

## RTO target

< 1 hour from incident-acknowledge to pool-back-online for most
scenarios.

## Procedure

### 1. Determine the recovery target

Decide whether you need:

- **Latest possible** (`--type=immediate`) — fastest, restores to
  the end of the WAL stream
- **Specific point in time** (`--type=time --target='<UTC>'`) —
  use when recovering from a logical-data incident at a known
  moment

### 2. Prepare the restore environment

```bash
# On the target host (could be the same VPS, a fresh VPS, or
# Railway ephemeral container — for cold restore we use a fresh
# VPS).

# Stop any running postgres
systemctl stop postgresql || true
# Clear the data directory (or move it aside)
mv /var/lib/postgresql/17/main /var/lib/postgresql/17/main.old.$(date +%s)
mkdir -p /var/lib/postgresql/17/main
chown -R postgres:postgres /var/lib/postgresql/17/main
```

### 3. Run the restore

```bash
# As the postgres user
sudo -u postgres pgbackrest \
  --stanza=katpool \
  --type=immediate \
  --target-action=promote \
  restore
```

For point-in-time:

```bash
sudo -u postgres pgbackrest \
  --stanza=katpool \
  --type=time \
  --target="2026-05-25 14:00:00+00" \
  --target-action=promote \
  restore
```

### 4. Start postgres

```bash
systemctl start postgresql
# Tail the log; you should see "database system is ready to accept
# connections" within a minute or two.
journalctl -u postgresql -n 50 --no-pager
```

### 5. Verify

```bash
# Connect and run reconciliation queries
psql -U katpool -d katpool -c "SELECT MAX(timestamp) FROM block_details;"
psql -U katpool -d katpool -c "SELECT SUM(balance) FROM miners_balance;"
psql -U katpool -d katpool -c "SELECT COUNT(*) FROM payments;"

# Compare these to the dashboard's last-known-good values.
```

### 6. Bring the pool back online

If the restore is on the same VPS:

```bash
systemctl start katpool
```

Watch logs; confirm shares are being credited and the canary
miner gets paid.

If the restore is on a fresh VPS, follow the deploy procedure in
[09](09-deploy-and-rollback.md) to point DNS at the new host.

## After the incident

- File a postmortem regardless of duration
- If the restore exposed a backup or WAL-archive issue, re-run the
  DR validator manually to confirm the fix
- If any miner balance moved during the recovery window, document
  the reconciliation
