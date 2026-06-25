---
status: accepted
date: 2026-05-25
deciders: argonmining
---

# ADR-0007: pgBackRest with WAL archiving to Backblaze B2

## Context and Problem Statement

The legacy pool backs up Postgres via `pg_dumpall` to Google Drive
on a twice-daily cron. That gives an RPO of up to 12 hours: a crash
at 11:55 with a 12:00 backup window loses 12 hours of share +
balance state. For a pool processing real-money transactions, that
is too lossy. Restore time has also never been measured — a
quarterly drill was previously proposed but is not actually
performed.

We need:

- RPO well under 1 minute
- RTO under 1 hour
- Automated verification (not "run a drill quarterly and hope")
- Encrypted at rest
- Storage cheap enough to be unremarkable

## Decision Drivers

- RPO under 1 minute → continuous WAL streaming, not periodic dumps
- RTO under 1 hour → fast restore from cheap object storage
- Operator cost discipline (operator constraint)
- Encryption at rest by default
- Restore must be automatically verified at a defined cadence

## Considered Options

1. **pgBackRest streaming WAL to Backblaze B2** (S3-compatible) +
   nightly full base + automated weekly DR validator
2. **WAL-G to B2.** Similar capability set.
3. **Continue `pg_dumpall` to Google Drive.** Status quo.
4. **Postgres logical replication to a hot standby** — adds a
   second DB host (cost + complexity).

## Decision Outcome

**Chosen option: 1 (pgBackRest + B2 + automated DR validator).**
pgBackRest is the most mature open-source physical-backup tool for
Postgres, has excellent documentation, supports the streaming-WAL +
incremental + full pattern, and integrates cleanly with
S3-compatible object stores.

Backblaze B2 is chosen over AWS S3 because:

- Egress to other clouds is free for the first 3× of stored bytes
  (Bandwidth Alliance)
- Storage is $6/TB/month vs. S3 standard ~$23/TB/month
- The Backblaze CLI / S3-compatible API is well-supported by
  pgBackRest

Automated weekly DR validation runs on Railway (separate failure
domain from B2 and the pool) and exercises the full restore-and-
reconcile path. See [`runbooks/10-automated-dr-validation.md`](../runbooks/10-automated-dr-validation.md).

### Consequences

- Positive: RPO drops from 12 h to < 1 min
- Positive: RTO under 1 h via point-in-time restore
- Positive: weekly validator means a real restore happens once a
  week, automatically — far stronger than a documented "we will
  drill quarterly"
- Positive: storage cost ~$1/month at our scale
- Negative: requires running the `pgbackrest` agent alongside the
  pool process (small, well-understood)
- Negative: ties us to B2; mitigated by S3-compatible API
  (provider-portable)

### Confirmation

- `ops/backup/pgbackrest.conf` is committed and applied via systemd
- pgBackRest `stanza-check` runs as a periodic systemd timer
- Weekly DR validator on Railway runs and reports green to the
  Grafana dashboard
- Alerts:
  - `BackupArchiveLag` — WAL archive lag > 5 min
  - `DRValidatorMissed` — no successful weekly run in > 10 days
  - `DRValidatorFailed` — most recent run reported a mismatch

## Pros and Cons of the Options

### Option 1: pgBackRest + B2 + automated validator

- Good: hits every driver
- Good: industry-standard tool with long track record
- Bad: requires running an agent; small ops surface

### Option 2: WAL-G + B2

- Good: also industry-standard
- Bad: comparable feature set; choice is largely preference
- Note: this is a close runner-up; if pgBackRest disappoints we'd
  pick WAL-G next.

### Option 3: pg_dumpall + Drive

- Bad: 12 h RPO; manual restore; no automated verification
- Rejected

### Option 4: Hot standby replication

- Good: near-zero RPO/RTO
- Bad: doubles DB compute cost
- Bad: doesn't replace off-site backups (we still need them)
- Out of scope for current pool size; revisit at scale

## More Information

- pgBackRest: <https://pgbackrest.org/>
- Backblaze B2 S3-compatible API: <https://www.backblaze.com/b2/docs/>
- Companion ADRs:
  [0006 (Postgres 17 pinning)](0006-postgres-17-pinned.md),
  [0009 (automated DR validation)](0009-automated-weekly-dr-validation.md)
