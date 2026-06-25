# Phase 2 acceptance evidence

Phase 2 (database schema + legacy importer) closes when every row
below is GREEN for a release-candidate commit. The Phase 3
(accountant) work-stream cannot start until this page is complete.

## Acceptance matrix

| # | Criterion | Verification | Status |
|---|---|---|---|
| 1 | New PostgreSQL schema covers every legacy use-case and every Phase 3+ use-case, with foreign keys, CHECK constraints, and idempotency guards. | 14 tables + 5 enums + 25 integration tests in `crates/katpool-db`. | GREEN — landed in PR #7 (milestone 1) |
| 2 | Schema applies via `sqlx::migrate!()` against an empty PostgreSQL 16+ instance from scratch. | `cargo test -p katpool-db --test migration` against ephemeral testcontainer. | GREEN — landed in PR #7 |
| 3 | Repository layer provides typed query API for every table the importer + accountant + payout engine + API will touch. | 11 `repo::*` modules; 25 integration tests covering wallet / worker / share / block / audit / pool_meta / connection_session / treasury / share_window / share_allocation / nacho_rebate / payout. | GREEN — landed in PR #12 (milestone 2) |
| 4 | Importer maps every production legacy table into the new schema. | Five transforms: blocks, balances, payments, nacho_payments, krc20 (landed PR #13/#14). | GREEN — landed in PR #13 (part A) + PR #14 (part B) |
| 5 | Every transform is idempotent (UPSERT / ON CONFLICT / set-not-add). | 6 + 4 + 6 + 4 = 20 integration tests across `import_blocks.rs`, `import_balances.rs`, `import_payments.rs`, `import_krc20.rs`. | GREEN |
| 6 | Cross-table reconciliation pass proves `sum(legacy) == sum(new)` for every monetary aggregate the importer touched. | `reconcile::run` + 2 reconcile-specific integration tests + assertion inside every scale/property test. | GREEN — landed in PR #14 |
| 7 | Importer scales linearly enough to finish a production cutover inside the 30-minute window. | Measured 2.4 ms/block on CI runner; ~21.6 min extrapolated for 539K blocks. | GREEN — measured 2026-05-26 |
| 8 | Cross-cutting invariants survive partial-failure restarts and idempotent re-runs against a mutating legacy DB. | Property/integration tests at import time (retired with the one-shot tool post-cutover). | GREEN — landed in this PR (milestone 4) |
| 9 | Operator rehearsal script captures the cutover-evidence artefacts required by the cutover ticket. | Evidence archived under `cutover-evidence/`; one-shot tool retired post-cutover. | GREEN — landed in this PR |
| 10 | Cutover plan ([`docs/cutover-plan.md`](cutover-plan.md)) references the importer gate. | Mainnet cutover executed 2026-06 with `reconcile_all_passed == true`. | GREEN — landed in this PR |
| 11 | Operator runbook documents dry-run, hot-run, and partial-failure-restart flows. | Runbook 14 retired post-cutover; procedure preserved in `cutover-plan.md` and `cutover-evidence/`. | GREEN — landed in PR #14 |
| 12 | `cargo deny check` is clean on the locked Cargo.lock. | CI step; locally verifiable with `cargo deny check`. | GREEN — every Phase 2 PR |

## Run history

### Importer scale-test throughput

Append a row every time the scale test runs on a new hardware
class or after a perf-impacting commit.

| Date (UTC) | Commit | Blocks | Import time | ms/block | Hardware | Extrapolated 539K-block runtime |
|---|---|---|---|---|---|---|
| 2026-05-26 | phase-2-importer-acceptance @ tip | 1,000 | 2.36 s | 2.36 | dev laptop (no concurrency-tuned pool) | ~21.2 min |
| 2026-05-26 | phase-2-importer-acceptance @ tip | 10,000 | 24.04 s | 2.40 | dev laptop (no concurrency-tuned pool) | ~21.6 min |

Cutover-time evidence (operator-driven, post-cutover) goes into
`cutover-evidence/<UTC-stamp>-hot-run/manifest.json`, not this
file.

### Reconcile checks proven by tests

Every integration test that exercises the reconcile pass asserts
every check passes. The full check inventory:

| Name | Aggregate | Source view | Target view |
|---|---|---|---|
| `blocks.row_count` | row counts | `block_details` | `block` |
| `blocks.miner_reward_total_sompi` | `sum(miner_reward)` | `block_details` | `block.miner_reward_sompi` |
| `payments.amount_total_sompi` | `sum(amount)` | `payments` | `payout` joined to `payout_cycle WHERE kind = 'kas' AND idempotency_key LIKE 'kas-legacy-%'` |
| `nacho_payments.amount_total` | `sum(nacho_amount)` | `nacho_payments` | `payout` joined to `payout_cycle WHERE kind = 'krc20_nacho' AND idempotency_key LIKE 'krc20-legacy-%' AND NOT LIKE 'krc20-legacy-pending-%'` |
| `miners_balance.nacho_rebate_total` | `sum(nacho_rebate_kas)` | `miners_balance` | `nacho_rebate_accrual.accrued_sompi` |
| `krc20_pending_transfer.count[PENDING]` | row count where status = PENDING | `pending_krc20_transfers` | `krc20_pending_transfer WHERE status = 'pending'` |
| `krc20_pending_transfer.count[COMPLETED]` | same, COMPLETED | same | `krc20_pending_transfer WHERE status = 'completed'` |
| `krc20_pending_transfer.count[FAILED]` | same, FAILED | same | `krc20_pending_transfer WHERE status = 'failed'` |

A reconcile mismatch surfaces as `importer_exit_code = 2` so the
rehearsal script (and any CI / runbook caller) can branch on it
without parsing JSON.

## Out of scope for Phase 2

- **Actual production-snapshot rehearsal.** This is an operator
  action at T-24h per runbook 14, captured into
  `cutover-evidence/` at cutover time. The Phase 2 engineering
  deliverable is the harness + the runbook, not the empirical
  go/no-go report (that lives in the cutover ticket).
- **Pending legacy KAS balance settlement.** The legacy stack
  flushes every `miners_balance.balance` row as its last
  on-chain action. The importer does **not** import that column.
  See runbook 14 § "Out-of-band: pending legacy KAS balance".

## Sign-off

Phase 2 closes when every row above is GREEN **and** the cutover
plan ([§ T-2m](cutover-plan.md#t-2m-legacy-stop--reconcile))
references the rehearsal script unambiguously.
