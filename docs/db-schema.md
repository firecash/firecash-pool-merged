# Database schema reference

Operator-facing reference for the katpool database. The design rationale
is in [ADR-0011](decisions/0011-db-schema-and-migrations.md); this file
is the "what tables exist, what columns mean, and how do I query them"
reference for anyone debugging a payout, reconciling a balance, or
investigating a per-rig issue.

The schema is owned by the `katpool-db` crate. Every change to it lands
as a new SQL migration under `crates/katpool-db/migrations/` and is
applied automatically by the binary on startup (see ADR-0011 for the
migration tooling rationale).

```
                    ┌──────────────────────────┐
                    │ wallet                   │
                    │ id, address, network     │
                    └─────────────┬────────────┘
                                  │ 1
                                  │
                                  ▼ N
                    ┌──────────────────────────┐
                    │ worker                   │
                    │ id, wallet_id, name      │
                    └─────────────┬────────────┘
                                  │ 1
                                  │
                                  ▼ N
            ┌─────────────────────┴─────────────────────┐
            │                                           │
            ▼ N                                         ▼ N
┌──────────────────────────┐              ┌──────────────────────────┐
│ connection_session       │              │ share                    │
│ id, worker_id, ip, ...   │◀── N         │ id, wallet_id,           │
└──────────────────────────┘     │        │   worker_id, session_id, │
                                 │        │   difficulty, daa_score  │
                                 └────────┤ correlation_id           │
                                          └──────────────────────────┘
                                                       │ aggregate
                                                       ▼
                                          ┌──────────────────────────┐
                                          │ share_window             │
                                          │ wallet_id, daa range,    │
                                          │   total_weight           │
                                          └──────────────────────────┘

                  ┌──────────────────────────┐
                  │ block                    │
                  │ id, hash, finder_*,      │
                  │   status, daa, reward    │
                  └─────────────┬────────────┘
                                │ 1
                                │
                                ▼ N
                  ┌──────────────────────────┐
                  │ share_allocation         │
                  │ block_id, wallet_id,     │
                  │   weight, fee, accrual,  │
                  │   net_payout_sompi       │
                  └──────────────────────────┘

                  ┌──────────────────────────┐
                  │ payout_cycle             │
                  │ id, kind, status,        │
                  │   idempotency_key        │
                  └─────────────┬────────────┘
                                │ 1
                                │
                                ▼ N
                  ┌──────────────────────────┐
                  │ payout                   │
                  │ cycle_id, wallet_id,     │◀── 1 ── krc20_pending_transfer
                  │   amount, status, tx     │
                  └──────────────────────────┘
```

## Tables

### `wallet`

One row per wallet ever seen. Address is the canonical bech32 form;
the CHECK constraint enforces network prefix + length so gross
malformations cannot land.

```sql
SELECT * FROM wallet WHERE address = 'kaspa:qz4...' LIMIT 1;
```

### `worker`

One row per `(wallet_id, name)`. Names follow the
`katpool_domain::WorkerName` charset; cascades on wallet delete.

### `connection_session`

One row per stratum TCP connection. `worker_id` is nullable because
the session is recorded at TCP-accept (before authorize).

### `share`

Every accepted share. Indexed on `(daa_score, wallet_id)` for PROP
allocation scans and on `(worker_id, credited_at DESC)` for per-rig
analytics. **High-volume table** — expect 10⁷–10⁹ rows over the
pool's lifetime.

```sql
-- PROP weight for a wallet in a DAA window
SELECT sum(difficulty) FROM share
 WHERE wallet_id = $1 AND daa_score >= $2 AND daa_score < $3;
```

### `share_window`

Pre-aggregated rollups of `share` for closed windows. The accountant
materialises a row per `(wallet_id, daa_start, daa_end)` after a
window finalises, so frequent payout queries don't scan the live
`share` table.

### `block`

Blocks we found. Status state machine: `found → submitted_to_node →
confirmed_blue → matured` (plus `orphaned` from any pre-mature
state). The lifecycle CHECK constraint enforces monotone timestamps.

```sql
-- Recent blocks the pool found, with finder identity
SELECT b.hash, w.address, b.status, b.found_at, b.miner_reward_sompi
  FROM block b
  JOIN wallet w ON w.id = b.finder_wallet_id
 ORDER BY b.found_at DESC LIMIT 20;
```

### `share_allocation`

PROP allocation of a block's matured reward among wallets that
contributed shares to its window. The `share_allocation_balance`
CHECK constraint enforces
`gross = pool_fee + nacho_accrual + net_payout` — every sompi
accounted for.

```sql
-- Total fee revenue from a block
SELECT sum(pool_fee_sompi) FROM share_allocation WHERE block_id = $1;
```

### `payout_cycle`

One row per KAS payout cycle or NACHO payout cycle. Identified by the
human-readable `idempotency_key` (`kas-<daa_start>-<daa_end>` /
`krc20-<daa_start>-<daa_end>`). Status state machine:
`planned → broadcasting → partially_settled → settled` (or `failed`).

### `payout`

Individual recipient payout under a cycle. `UNIQUE (cycle_id, wallet_id)`
is the idempotency guard — re-broadcast attempts are `INSERT ON
CONFLICT DO NOTHING` no-ops, never duplicates.

```sql
-- Open payouts that need a rebroadcast
SELECT p.*, w.address
  FROM payout p
  JOIN wallet w ON w.id = p.wallet_id
 WHERE p.status IN ('planned', 'submitted')
   AND p.planned_at < now() - interval '1 hour';
```

### `nacho_rebate_accrual`

Running NACHO rebate balance per wallet.
`pending = accrued_sompi - paid_sompi`; the
`nacho_rebate_paid_le_accrued` CHECK enforces non-negative pending.

### `krc20_pending_transfer`

Replaces legacy `pending_krc20_transfers`. State machine: `pending →
commit_submitted → reveal_submitted → completed` (or `failed`).
Cascades on payout delete.

### `treasury_snapshot`

Periodic hot-wallet snapshot row. The treasury custody runbook (runbook
11) populates these on every key rotation; the auditor (Phase 8) reads
them for the running balance ledger.

### `audit_log`

Append-only audit trail. Insert-only by convention (no `UPDATE`,
no `DELETE`). The `JSONB` payload carries action-specific detail.

```sql
-- Every action on a specific payout
SELECT * FROM audit_log
 WHERE subject_type = 'payout' AND subject_id = $1
 ORDER BY occurred_at;
```

### `pool_meta`

Single-row key/value store for runtime constants. Used by the
accountant for "last DAA processed" markers, by the API for the
last warmup timestamp, etc.

## Enum types

| Type | Variants |
|---|---|
| `block_status` | `found`, `submitted_to_node`, `confirmed_blue`, `matured`, `orphaned` |
| `payout_kind` | `kas`, `krc20_nacho` |
| `payout_cycle_status` | `planned`, `broadcasting`, `partially_settled`, `settled`, `failed` |
| `payout_status` | `planned`, `submitted`, `accepted`, `confirmed`, `failed` |
| `krc20_transfer_status` | `pending`, `commit_submitted`, `reveal_submitted`, `completed`, `failed` |

Adding a variant requires a new migration; never edit an existing
enum in place.

## Migration tooling

Migrations live under `crates/katpool-db/migrations/` named
`<YYYYMMDDHHMMSS>_<name>.sql`. The `sqlx::migrate!` macro embeds them
into the binary at build time.

To apply migrations against a database manually:

```bash
DATABASE_URL=postgres://kat:kat@localhost:5432/katpool \
  sqlx migrate run --source crates/katpool-db/migrations
```

Production applies migrations automatically on startup via
`katpool_db::migrate::run(&pool)` — see ADR-0011 for the fail-closed
contract.

Down-migrations are intentionally **not** authored. Rollback is via
pgBackRest snapshot restore (see runbook 04).
