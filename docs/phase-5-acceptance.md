# Phase 5 acceptance evidence

Phase 5 (NACHO / KRC-20 rebate payout engine) closes when every row below
is GREEN for a release-candidate commit. It reuses the Phase 4 payout
scaffolding (`payout_cycle` / `payout` rows, `katpool-idempotency`
advisory lock, the `PayoutEngine` loop shape, `katpool-secrets` treasury
custody, and the `katpool-storagemass` planner).

Prerequisites: Phase 4 complete (KAS payout engine + treasury custody).
The `krc20_pending_transfer` table and `krc20_transfer_status` enum landed
in Phase 2 (`docs/db-schema.md`).

## Acceptance matrix

| # | Criterion | Verification | Status |
|---|---|---|---|
| 1 | KRC-20 inscription envelope: build the kasplex commit redeem script, P2SH commit address, and reveal signature script, byte-compatible with the kasplex-accepted production transfer. | Deterministic, chain-free unit tests pinning the exact envelope bytes, canonical compact JSON field order, testnet-10 P2SH derivation, hash-binds-payload, and `<sig><pushed redeem>` reveal script. Format decision recorded in [ADR-0015](decisions/0015-krc20-inscription-envelope.md). | GREEN — M5.1 |
| 2 | NACHO eligibility + rebate amount: pending balance per wallet (`nacho_rebate::list_pending`); floor-price quote from `api.kaspa.com` behind a mockable trait with a fail-closed circuit breaker; exact integer KAS-sompi→NACHO conversion at the quoted price. **No payout-time multiplier** — the tier rebate (33%/100%) is already baked into `accrued_sompi` at allocation (ADR-0012); architecture.md §4.4's "3× at payout" is superseded by [ADR-0016](decisions/0016-krc20-payout-conversion-and-floor-price.md). | Pure unit + property tests for the conversion and exact fixed-point floor-price parser; deterministic time-injected circuit-breaker transitions; HTTP client against a wiremock server (200/parse, non-200, malformed). | GREEN — M5.2 |
| 3 | Mass-aware commit/reveal planner: one recipient per reveal; every planned commit and reveal tx satisfies independent mass ≤ `max_block_mass` incl. `transient_storage_mass`. The reveal is evaluated with **signed-length** signature scripts so the redeem-script push is counted (`SIGNATURE_SIZE = 66`); the planner also surfaces KIP-9 anti-dust on near-floor change. | Deterministic, chain-free tests via `katpool-storagemass` (`docs/kips.md` §5.2): both txs fit independently, reveal transient mass exceeds 4×redeem-script length, funding/dust/sub-floor/storage-mass verdicts. | GREEN — M5.3 |
| 4 | Sign + submit commit/reveal via kaspad gRPC; drive `krc20_pending_transfer` (`pending → commit_submitted → reveal_submitted → completed` / `failed`); record intent **before** broadcast; no double-pay on mid-cycle restart. | Deterministic txscript-engine verification + mock-kaspad orchestration on testcontainer Postgres; crash-before-broadcast chaos test. | GREEN — M5.4a (signer: commit standard, reveal manual P2SH-redeem path, both engine-verified, deterministic txids) + M5.4b (executor state machine reusing the Phase 4 `KaspadClient`/confirm policy: record-before-broadcast, idempotent re-broadcast, UTXO-drift refusal, full-lifecycle/crash/dry-run tests) |
| 5 | `payout-krc20` wired into `katpool` runtime (periodic loop + distributed lock, reusing the Phase 4 engine shape); dry-run flag for rehearsal; safe-by-default. | Advisory-lock mutual-exclusion + multi-tick settlement / non-leader skip / shutdown tests over testcontainer Postgres + mock kaspad. | GREEN — M5.5a (cycle state machine: plan/resume/credit/fail/reconcile; eligibility nets out non-terminal payouts so no cross-cycle double-select; exactly-once crediting + refund-on-failure) + M5.5b (single-leader `Krc20PayoutEngine` loop — plan → settle → credit → reconcile per DAA window, distinct advisory-lock namespace, dry-run records/broadcasts/credits nothing; wired into `katpool` opt-in via `KATPOOL_KRC20_PAYOUT_*`. Verified by multi-tick settlement, non-leader skip, and clean shutdown tests over testcontainer Postgres + address-keyed mock kaspad + fixed floor price). |
| 6 | Operator rehearsal: one full dry-run NACHO cycle produces reconcile JSON + audit log + manifest (mirrors the Phase 4 KAS rehearsal). | Phase 5 sign-off complete; live testnet-10 dry-run artefacts archived under `payout-evidence/`. The one-shot rehearsal tool was retired post-acceptance. | GREEN — M5.6 |
| 7 | `cargo deny check` clean on the locked `Cargo.lock`. | CI step. | GREEN — inherited |

## Milestone map (PR-sized)

| Milestone | Delivers | Closes rows |
|---|---|---|
| **M5.1** | kasplex inscription envelope + P2SH + reveal-script primitives (pure) | 1 |
| **M5.2** | NACHO eligibility + floor-price quote + full-rebate logic | 2 |
| **M5.3** | Mass-aware commit/reveal planner | 3 |
| **M5.4a** | commit/reveal signer (standard commit + manual P2SH reveal, engine-verified) | 4 (partial) |
| **M5.4b** | executor state machine + kaspad submit/confirm + crash-before-broadcast | 4 |
| **M5.5a** | KRC-20 cycle state machine (plan/resume/credit/fail/reconcile, DB-only) | 5 (partial) |
| **M5.5b** | single-leader engine loop + `katpool` wiring + dry-run | 5 |
| **M5.6** | Rehearsal tool + runbook + acceptance evidence | 6 |

## Out of scope for Phase 5

- **Phase 6** — Public HTTP API.
- **Phase 7–10** — Production edge, shadow run, cutover.

## Sign-off

Phase 5 closes when:

1. Every row in the matrix is GREEN.
2. A testnet-10 dry-run NACHO cycle completes (planned commit/reveal pairs +
   mass plan, no broadcast).
3. A live testnet-10 commit/reveal pair is accepted by Kaspa L1 with
   deterministic, golden-pinned envelope/script shape. Per
   [ADR-0019](decisions/0019-krc20-adaptive-fee-and-fee-persistence.md) §3,
   Kasplex crediting **cannot** be confirmed on testnet-10 — NACHO does not
   exist there, so the indexer always reports `insufficient balance`. The
   Kasplex-credit empirical confirmation of ADR-0015 is therefore deferred to
   the **mainnet cutover** (`docs/cutover-plan.md`, T+1h first live NACHO
   cycle), where it must verify with no exceptions.
4. Operator has archived rehearsal evidence under `payout-evidence/`.

## Live testnet-10 go-live (2026-06-02)

KRC-20 payouts were taken out of dry-run on `tn10-phase5`. Commit + reveal
pairs were accepted by the tn10 mempool with byte-exact envelope/script shape
(verified via `api-tn10.kaspa.org`), satisfying the ADR-0019 §3 success
criterion. Go-live drove out two defects that were fixed before sign-off:

- **Adaptive fees + per-transfer fee freeze** — the original flat `0.0001 KAS`
  commit/reveal fee was ~18–20× below the relay minimum and would have been
  rejected on first broadcast. Resolved in
  [ADR-0019](decisions/0019-krc20-adaptive-fee-and-fee-persistence.md).
- **Sweep-coherent UTXO chaining** — sibling commits in one settle sweep
  selected the same confirmed treasury UTXO and double-spent each other.
  Resolved in
  [ADR-0020](decisions/0020-krc20-sweep-coherent-utxo-chaining.md).

Evidence archived under `payout-evidence/2026-06-02T07-12-20Z-tn10-krc20-golive`.
