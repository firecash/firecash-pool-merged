---
status: accepted
date: 2026-05-25
deciders: argonmining
---

# ADR-0009: Automated weekly DR validation instead of manual quarterly drills

## Context and Problem Statement

A manually-scheduled quarterly DR drill (spin up a throwaway VPS,
restore the latest backup, reconcile balances, sign off) was the
original plan. The operator pushed back: it's a recurring calendar
item that easily slips, and the only way to know the backups
actually work is to run a real restore. We need something that
either runs itself or doesn't run.

Manual drills, in practice, are how organisations discover their
backups have been silently broken for 90+ days when an actual
disaster hits. Automation is the answer.

## Decision Drivers

- Backup validity must be continuously verified, not aspirationally
  documented
- Zero recurring manual ops where avoidable (operator preference)
- Use the same Railway platform as the rest of our non-pool
  services (consistency)
- Real restore, not just a backup-file-exists check
- Alertable mismatch detection so silent corruption surfaces fast

## Considered Options

1. **Automated weekly Railway cron job** that pulls the latest base
   + WAL, restores to an ephemeral Postgres, runs reconciliation
   queries, alerts on mismatch.
2. **Manual quarterly drill.** Original plan; rejected for the
   reasons above.
3. **Backup-existence checks only** (file in B2 in the last N
   hours). Doesn't catch silent corruption.
4. **Continuous logical-replication standby** as a poor-man's
   restore drill. Doesn't exercise the actual restore path; also
   doubles DB cost.

## Decision Outcome

**Chosen option: 1 (automated weekly Railway cron).** Service at
[`ops/railway/dr-validator/`](../../ops/railway/dr-validator/) is a
single Rust binary that:

1. Pulls the latest `pgBackRest` base + WAL files from Backblaze B2
2. Spins up an ephemeral Postgres 17 container on Railway's
   compute (~5 minute lifetime)
3. Runs `pgbackrest restore --type=time` to ~T-5min (proves
   point-in-time recovery)
4. Executes reconciliation queries:
   - `SELECT SUM(balance) FROM miners_balance` matches the
     reference (within ±0 sompi)
   - `SELECT COUNT(*) FROM block_details` exceeds a moving
     threshold
   - `SELECT MAX(timestamp) FROM block_details` is within 1 h of
     expected
   - 10-row hash spot-check on `block_details`
   - `REINDEX DATABASE` succeeds (catches structural corruption)
5. Posts result to Alertmanager / Grafana dashboard
6. Tears the container down

Two alerts cover the failure modes:

- `DRValidatorMissed` (no successful weekly run in > 10 days)
- `DRValidatorFailed` (most recent run reported a mismatch)

### Consequences

- Positive: a real restore happens once a week, automatically
- Positive: catches silent backup misconfiguration within 7 days
- Positive: no recurring manual calendar item
- Positive: provides a dashboard "last successful DR drill: T-N
  hours" indicator that's perpetually relevant
- Negative: ~$2–5/month additional Railway compute
- Negative: requires implementing the validator binary
  (single-purpose, ~few hundred lines)

### Confirmation

- The DR validator binary exists in `ops/railway/dr-validator/`
- Railway cron is configured to run weekly (`0 4 * * 0`)
- A controlled test (run a known-bad backup through the validator)
  confirms `DRValidatorFailed` fires correctly
- Pre-cutover acceptance: 4 consecutive successful weekly cycles
  visible in the dashboard

## Pros and Cons of the Options

### Option 1: Automated weekly Railway cron

- Good: every objective met
- Good: tiny ongoing cost; consistent with our existing Railway
  footprint
- Bad: requires the operator to fix it if it breaks (but that's
  signalled by an alert, not by silence)

### Option 2: Manual quarterly drill

- Good: zero implementation effort
- Bad: known to be skipped in practice; catches problems at most
  every 90 days; calendar-driven not failure-driven
- Rejected

### Option 3: Backup-existence-only check

- Good: trivial
- Bad: doesn't catch silent backup corruption — the failure mode
  that matters most
- Rejected

### Option 4: Hot-standby replication

- Bad: doesn't exercise the restore path
- Bad: doubles DB cost
- Out of scope; this is a redundancy strategy, not a backup
  strategy

## More Information

- pgBackRest restore: <https://pgbackrest.org/user-guide-rhel.html#quickstart/perform-restore>
- Companion ADRs:
  [0007 (pgBackRest)](0007-pgbackrest-wal-archiving.md),
  [0004 (self-host observability)](0004-self-host-observability.md)
- Companion runbook: [`runbooks/10-automated-dr-validation.md`](../runbooks/10-automated-dr-validation.md)
