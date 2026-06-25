---
status: accepted
date: 2026-05-26
deciders: argonmining
---

# ADR-0011: Database schema, sqlx-managed migrations, and legacy import strategy

## Context and Problem Statement

The legacy `katpool_mainnet` database has seven tables and 1.2 GiB of
production data accumulated over the pool's lifetime. The schema has
worked operationally but has structural issues that block the
reliability properties Phase 0 committed to:

- **No foreign keys anywhere.** `block_details.miner_id` references
  nothing; `miners_balance.wallet` is free-form text; `payments.wallet_address`
  is a `text[]` array of wallet strings rather than a join table.
- **Wallet identity is text-duplicated across every row.** A wallet
  with 50 k shares has the same 80-character bech32 string written
  50 k times.
- **Hashes stored as `varchar(255)`.** Block hashes are exactly 32
  bytes; storing them as 64-character hex in a 255-wide varchar
  multiplies index and table size by 4–8×.
- **`numeric` for sompi balances with no precision/scale.** Sompi is
  integer (kaspa supply caps at ~2.1 × 10¹⁵ sompi, well under `bigint`).
- **No shares table at all.** Only running balances. PROP allocation
  has no auditable per-share trail; share-difficulty fairness disputes
  are unprovable.
- **No idempotency keys on payouts.** Root cause of the legacy
  NACHO double-pay incident (see runbook 02). Every payout retry can
  re-execute.
- **Timestamps are `timestamp without time zone`.** DST/UTC ambiguity
  during cross-region triage.
- **No block lifecycle.** "Found" and "matured" collapse to a single
  row; orphans are invisible.

These aren't fatal for the legacy pool's daily operation but they make
the next four years of changes (every accountant tweak, payout
campaign, audit response) cost 10× what they should.

## Decision

The new database is a clean schema with thirteen tables, full FK
integrity, typed enums for state machines, and explicit idempotency on
every external-effecting row. Owned by the `katpool-db` crate.

### Schema layout

The migration is one SQL file checked in at
`crates/katpool-db/migrations/<timestamp>_bootstrap.sql`. Subsequent
schema changes get their own timestamp-prefixed migration; sqlx tracks
applied versions in its built-in `_sqlx_migrations` table.

Tables (with a one-line purpose each):

| Table | Purpose |
|---|---|
| `wallet` | Wallet identity (canonical bech32 + network); 1 row per wallet ever seen |
| `worker` | Rig identity within a wallet; 1 row per `(wallet, worker_name)` |
| `connection_session` | Per-stratum-connection record; populated from `PoolEvent::*` |
| `share` | Every accepted share; (daa_score, wallet_id) indexed for PROP queries |
| `share_window` | Pre-aggregated PROP rollups for closed windows |
| `block` | Blocks we found, full lifecycle state machine |
| `share_allocation` | PROP share-of-block per wallet per block (the gross/fee/accrual breakdown) |
| `payout_cycle` | KAS or NACHO payout cycle; idempotency key per cycle |
| `payout` | Individual recipient payout; idempotency key per `(cycle_id, wallet_id)` |
| `nacho_rebate_accrual` | Running NACHO rebate balance per wallet |
| `krc20_pending_transfer` | Replaces legacy `pending_krc20_transfers` with FK + status enum |
| `treasury_snapshot` | Periodic hot-wallet snapshots for audit & reconcile |
| `audit_log` | Append-only audit trail for operator and automated actions |

Plus one helper: `pool_meta` (single-row key/value for runtime
constants, e.g. last DAA the accountant processed).

### Type choices

| Concept | Storage | Rationale |
|---|---|---|
| Block hash | `BYTEA` with `CHECK octet_length = 32` | 4× smaller than hex-string; equality + index identical perf |
| Sompi amount | `BIGINT` | Max 2.1 × 10¹⁵ sompi total supply fits in `bigint`; no `numeric` overhead |
| Wallet address | `TEXT` with regex `CHECK` | Variable length 50–80 chars per network; CHECK rejects gross format violations |
| Share difficulty | `DOUBLE PRECISION` | Matches the wire format the bridge already operates on |
| DAA score | `BIGINT` | Native u64 from kaspa-consensus; bigint signed range still covers > 9 × 10¹⁸ |
| Timestamps | `TIMESTAMPTZ` everywhere | Always UTC on disk; no naive timestamps period |
| Synthetic PK | `BIGSERIAL` | Sequential indexes outperform UUID v4 for hot tables; UUID kept only for `correlation_id` (already a UUID v4 from the bridge event bus) |
| State machine | Postgres `ENUM` type | Bounded vocabulary, no string typos; migration adds variants explicitly |

### Foreign keys & cascade rules

Every non-static table has at least one FK. Cascade choices:

- `worker(wallet_id)` → `wallet(id)` `ON DELETE CASCADE` (orphan workers
  cannot survive their wallet)
- `share(wallet_id, worker_id, session_id)` → `RESTRICT` /
  `ON DELETE SET NULL` for session (sessions get pruned; shares stay
  for accounting)
- `block(finder_wallet_id, finder_worker_id)` → `RESTRICT` (block rows
  outlive their finders for audit)
- `share_allocation(block_id)` → `ON DELETE CASCADE` (allocations only
  exist for live blocks; if a block is purged so are its allocations)
- `payout(cycle_id)` → `RESTRICT` (cannot delete a cycle that has
  payouts under it)
- `krc20_pending_transfer(payout_id)` → `ON DELETE CASCADE`

### Idempotency

Two-tier:

1. **Per-cycle** key (`payout_cycle.idempotency_key`) is human-readable
   (`kas-<daa_start>-<daa_end>` or `krc20-<daa_start>-<daa_end>`).
   Lets operators reason about cycles by name and prevents
   duplicate-cycle creation if a partial commit gets retried.
2. **Per-recipient** key (`UNIQUE (cycle_id, wallet_id)` on `payout`).
   Makes payout broadcast retries strictly safe — re-running the same
   broadcast produces an `INSERT ... ON CONFLICT DO NOTHING` no-op
   rather than a duplicate transaction.

The application layer (`payout-kas`, `payout-krc20`) is responsible
for actually generating + checking these keys before broadcasting; the
schema enforces correctness if they do.

### Audit trail

`audit_log` is append-only by convention (no `UPDATE`/`DELETE` from
the application layer; revoked via `GRANT` if ever needed). Every
operator-initiated mutation (`payout.broadcast`, `cycle.cancel`,
`treasury.rotate`, etc.) inserts a row. The payload is `JSONB` so the
schema doesn't have to evolve with every new action; queries by
`action` or `subject_*` use the indexes provided.

The `correlation_id` column connects `audit_log` entries to the
PoolEvent (and downstream payout) that triggered them, completing the
trace from "miner submitted share" → "pool paid out" → "operator
audited".

### Migration & legacy-data strategy

Bootstrap migration creates the empty schema. Phase 2 milestone 3
(separate PR) ships a standalone `katpool-import-legacy` binary that:

1. Reads from the legacy `katpool_mainnet` database (read-only role).
2. Transforms each table into the new shape:
   - `block_details` → `block` + `wallet` + `worker` rows
   - `miners_balance` → `nacho_rebate_accrual` (the `nacho_rebate_kas`
     column) plus pending KAS balance
   - `payments` → `payout_cycle` + `payout` (one cycle per unique
     `transaction_hash`)
   - `nacho_payments` → same as `payments` but `kind = 'krc20_nacho'`
   - `pending_krc20_transfers` → `krc20_pending_transfer`
3. Idempotent: every UPSERT keys off a natural identifier, so the
   importer is safe to re-run.
4. Reconciliation report on completion: sum-of-balances pre/post,
   row counts per table, anomaly list.

The new pool runs **alongside** the legacy stack against an empty
new database during Phase 2–6 development. Phase 7's cutover plan
runs the importer at the cutover moment so the new pool starts with
a balance snapshot identical to the legacy pool's state.

### Migration tooling

sqlx's built-in migrator. Files in `crates/katpool-db/migrations/`
follow the `<timestamp>_<name>.sql` convention. Two execution paths:

- **CI / test:** `sqlx::migrate!` macro embeds migrations at compile
  time; tests use `#[sqlx::test]` to spin up an ephemeral postgres
  via testcontainers and replay every migration into it.
- **Production:** the `katpool` binary calls `katpool_db::migrate()`
  on startup before serving any request; refuses to start if a
  migration fails or the database schema is ahead of what the binary
  knows about.

Down-migrations (`*.down.sql`) are intentionally **not** authored.
sqlx supports them; we don't. Rollback strategy is "restore the
preceding pgBackRest snapshot" (see runbook 04). Hand-written down
migrations are a notorious source of "looked right, lost data"
incidents — pgBackRest gives us a known-good byte-perfect rollback
that we test weekly via the DR validator (ADR-0009).

## Consequences

- The `katpool-db` crate now has a real public surface: pool builder,
  migration runner, typed `DbError`. Service crates depend on it via
  the workspace path dep already wired in `Cargo.toml`.
- Every Phase 3+ schema change is one new migration file. No more
  ad-hoc `ALTER TABLE` in psql.
- The legacy database is **read-only** as far as the new pool is
  concerned. Any change to the legacy schema is a Phase 7 cutover
  concern, not a Phase 2 concern.
- `cargo-deny check sources` will see the postgres binary protocol
  but no new git deps from this work.
- Testcontainers + Docker required in CI. The CI workflow's `test`
  job already runs on `ubuntu-latest` which has docker available.
- pgBackRest scope statement (ADR-0007) explicitly covers the new
  database from the moment it has data; testnet databases use
  ephemeral storage and skip backups.

## Considered alternatives

- **Migrate the legacy schema in place.** Rejected: the FK story
  cannot be retrofitted without re-typing every column, and the
  legacy `text[]` payment-recipients schema cannot be normalised
  without breaking row identity. Cleaner to start fresh and import.
- **Diesel instead of sqlx.** Rejected: Diesel's compile-time schema
  derivation is great but locks us into a fixed schema-at-compile-time
  view that fights us when running against a database that's
  newer-than-binary (a guard-rail we want during rolling deploys).
  sqlx's `query!` macro gives compile-time SQL validation against a
  live `DATABASE_URL` at build, which is what we want for the
  type-safety win without the schema-rigidity tax.
- **Partitioning the `share` table by DAA range now.** Deferred:
  postgres handles 100M-row tables fine with the indexes we ship;
  partitioning becomes a Phase 9 capacity-test action item if hot-
  query latency degrades. Documented as such in `docs/capacity-plan.md`.

## Status

Accepted on 2026-05-26 alongside the first PR that lays the schema
and `katpool-db` crate wiring (Phase 2 milestone 1).
