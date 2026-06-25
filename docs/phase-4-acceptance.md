# Phase 4 acceptance evidence

Phase 4 (KAS payout engine) closes when every row below is GREEN
for a release-candidate commit. Phase 5 (NACHO/KRC-20 payout)
cannot start until this page is complete.

Prerequisites: Phase 3 complete (accountant + unified `katpool`
runtime). Schema payout tables and `katpool-db::repo::payout` landed
in Phase 2.

## Acceptance matrix

| # | Criterion | Verification | Status |
|---|---|---|---|
| 1 | `katpool-storagemass` computes compute, storage (KIP-9), and transient (KIP-13) masses matching `kaspa-consensus-core` for mainnet params. | Unit tests + `proptest` parity against `MassCalculator`; May-1 regression fixture when available. | GREEN — M4.1 |
| 2 | Mass-aware batch planner: given treasury UTXOs + recipient list, every planned tx satisfies independent mass ≤ `max_block_mass` (500_000 g). | Property tests + hand-rolled cases from `docs/kips.md` §5. | GREEN — M4.2 |
| 3 | Eligibility: wallets with `sum(net_payout_sompi) - sum(confirmed_kas_payouts) >= threshold` (default 5 KAS / 500_000_000 sompi). | Integration test against testcontainer Postgres. | GREEN — M4.3 |
| 4 | Payout cycle orchestration: create `payout_cycle`, insert `payout` rows, advance status machine; idempotent on retry. | Uses existing `repo::payout` + new planner; integration tests. | GREEN — M4.3 |
| 5 | Record idempotency **before** sign: no double-pay on mid-cycle restart. | `payout_cycle.idempotency_key` + per-recipient `UNIQUE (cycle_id, wallet_id)`; chaos test simulating crash after plan, before broadcast. | GREEN — M4.4 |
| 6 | `katpool-secrets`: load treasury key via age/sops contract; `Secret` type has no `Debug`; page `mlock` + `zeroize` on drop. | Unit tests + documented systemd `LoadCredentialEncrypted` path. | GREEN — M4.5 |
| 7 | Sign + submit KAS txs via kaspad gRPC; confirm and mark `payout` rows `confirmed`. | Deterministic: txscript-engine signature verification + mock-kaspad orchestration on testcontainer Postgres (sign → submit → accept → confirm → settle, idempotent re-run, dry-run). Live broadcast against tn10 funded treasury exercised in M4.8. | GREEN — M4.6 |
| 8 | `payout-kas` wired into `katpool` runtime (periodic loop + distributed lock); `KATPOOL_PAYOUT_DRY_RUN` for rehearsal. | Deterministic: `katpool-idempotency` advisory-lock mutual-exclusion test + `payout-kas::engine` multi-tick settlement, non-leader skip, and `run_loop` shutdown over testcontainer Postgres + mock kaspad. Live testnet-10 dry-run exercised in M4.8; safe-by-default (engine off + dry-run unless two flags flipped). | GREEN — M4.7 |
| 9 | Operator rehearsal: one full dry-run cycle produces reconcile JSON + audit log + manifest (mirrors Phase 2 importer pattern). | Phase 4 sign-off complete; live testnet-10 dry-run artefacts archived under `payout-evidence/`. The one-shot rehearsal tool was retired post-acceptance. | GREEN — M4.8 |
| 10 | `cargo deny check` clean on the locked `Cargo.lock`. | CI step. | GREEN — inherited from Phase 3 |

## Milestone map (PR-sized)

| Milestone | Delivers | Closes rows |
|---|---|---|
| **M4.1** | `katpool-storagemass` mass parity wrapper | 1 |
| **M4.2** | `plan_batches` greedy payout tx planner | 2 |
| **M4.3** | Eligibility query + cycle planning (DB only, no chain) | 3, 4 |
| **M4.4** | Restart-safe cycle state machine + audit hooks | 5 |
| **M4.5** | `katpool-secrets` treasury loader | 6 |
| **M4.6** | kaspad sign/submit/confirm adapter | 7 |
| **M4.7** | `payout-kas` engine + `katpool` wiring + dry-run | 8 |
| **M4.8** | Rehearsal script + runbook + acceptance evidence | 9 |

## Out of scope for Phase 4

- **Phase 5** — NACHO KRC-20 commit/reveal payouts.
- **Phase 6** — Public HTTP API.
- **Phase 7–10** — Production edge, shadow run, cutover.

## Sign-off

Phase 4 closes when:

1. Every row in the matrix is GREEN.
2. A testnet-10 dry-run payout cycle completes with `KATPOOL_PAYOUT_DRY_RUN=true`
   (planned rows + mass plan, no broadcast).
3. Operator has archived rehearsal evidence under `payout-evidence/`.
