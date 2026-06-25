---
status: accepted
date: 2026-05-25
deciders: argonmining
---

# ADR-0006: Pin PostgreSQL to the `17` major

## Context and Problem Statement

The legacy pool uses `postgres` as the Docker image tag — the
floating `latest`. The current production data directory was
written by PostgreSQL 17. On 2026-04-22 we narrowly avoided a
production outage when a planned `docker compose pull` would have
upgraded the image to `postgres:18.3` (the floating `latest` at the
time), which would have refused to start against the 17-version
data directory — at best — and possibly silently re-initialised on
the version-scoped `PGDATA` path introduced in PostgreSQL 18.

We need to pin the major.

## Decision Drivers

- Reproducible deploys — a pull must not change what's running
- Avoid catastrophic accidental major-version upgrades of the DB
- Allow safe minor (patch) updates for security fixes
- Keep the same on-disk format the production data directory was
  initialised with

## Considered Options

1. Pin `postgres:17` (floating minor under major 17, e.g. 17.6,
   17.7 …)
2. Pin `postgres:17.6` (exact minor)
3. Pin by digest (`postgres@sha256:...`)
4. Leave `postgres:latest` (status quo). Rejected on grounds of
   the April 2026 near-miss.

## Decision Outcome

**Chosen option: 1, plus digest pinning per release.** We declare
`postgres:17` as the canonical tag in [`ops/docker/compose.prod.yml`](../../ops/docker/compose.prod.yml)
for clarity, but bind to a specific digest at deploy time. The
release workflow snapshots the digest of the current `postgres:17`
into the deploy manifest. This gives us:

- No surprise major-version pull (17 is stable until we deliberately
  upgrade to 18)
- Digest-pinned at deploy so a re-deploy is reproducible
- Easy refresh: bump the digest after CI runs the upgrade-test
  workflow against a copy of `postgres_data`

Major upgrade to 18 is a separate planned migration with its own
ADR + runbook, requiring `pg_upgrade` and a maintenance window.

### Consequences

- Positive: no accidental major upgrade
- Positive: minor security patches are easy (refresh the pinned
  digest in a PR; CI tests the migration roundtrip)
- Positive: documented upgrade path
- Negative: requires periodic digest refresh hygiene. Mitigated
  via a Dependabot-style scheduled CI workflow that opens a PR
  proposing the latest 17.x digest

### Confirmation

- `ops/docker/compose.prod.yml` references `postgres:17@sha256:...`
- A CI workflow (`security.yml` or a dedicated one) runs `trivy`
  against the pinned digest and alerts on Critical/High
- Migration roundtrip test runs against the pinned image in CI

## Pros and Cons of the Options

### Option 1: Pin `postgres:17` + digest

- Good: hits all drivers
- Bad: requires periodic hygiene to bump the digest

### Option 2: Pin `postgres:17.6` exactly

- Good: ultra-explicit
- Bad: every patch release is a manual config bump in the compose
  file; redundant with digest pinning

### Option 3: Pin by digest only

- Good: most explicit possible
- Bad: harder for an operator to read at a glance which major they
  have
- Used in combination with #1

### Option 4: `postgres:latest`

- Bad: caused the near-miss in April 2026
- Rejected

## More Information

- PostgreSQL 18 PGDATA change announcement (Docker Hub overview):
  the `PGDATA` env var moved to a version-specific path in v18
- Companion ADR: [0007 (pgBackRest)](0007-pgbackrest-wal-archiving.md)
