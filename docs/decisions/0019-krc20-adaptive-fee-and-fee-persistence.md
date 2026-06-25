---
status: accepted
date: 2026-06-02
deciders: argonmining
---

# ADR-0019: KRC-20 commit/reveal adaptive fees, frozen for crash-safe determinism

## Context and Problem Statement

ADR-0018 fixed the KAS payout engine's `RejectInsufficientFee` defect by
sizing fees adaptively from the node fee-estimate (floored at the relay
minimum). The KRC-20 (NACHO) payout engine carried the **same defect**: its
commit and reveal transactions reserved a flat `0.0001 KAS` fee. A real
commit needs ≈ `0.002 KAS` (compute mass ≈ 2047) and a real reveal ≈
`0.0018 KAS` (compute mass ≈ 1821) — so the fixed fee was ~18–20× below the
mempool minimum and **both transactions would be rejected** the moment the
engine went live.

KRC-20 differs from KAS in a way that makes a straight copy of the KAS
"exact-fee-from-signed-transaction" approach unsafe. A NACHO transfer is a
**two-phase commit/reveal** (ADR-0015):

1. the **commit** funds a P2SH output (`commit_amount`) plus treasury change;
2. the **reveal** spends that one P2SH output and returns
   `commit_amount − reveal_fee` to the treasury.

For crash-safety the engine records each transaction's id **before**
broadcast (`record-before-broadcast`, M5.4). Both ids are sensitive to the
fees: the commit id depends on its change (`= inputs − commit_amount −
commit_fee`), and the reveal id depends on its return (`= commit_amount −
reveal_fee`). If a crash-resume re-quoted a **different** live fee-rate, it
would rebuild a **different** commit/reveal and the re-derived id would no
longer match the recorded one — surfacing as a (correct, but un-actionable)
`CommitDrift` / `RevealDrift` and stalling the transfer.

## Decision Drivers

* Long-term reliability across any network condition (operator decision: match
  the KAS approach rather than a static relay-minimum policy).
* Value math integer-deterministic and mirrored from consensus (ADR-0018).
* A recorded-before-broadcast txid must be **reproducible** for the life of the
  transfer, across restarts and re-quotes.
* Minimal, production-grade change; no new external dependency; reuse the
  `katpool-storagemass::FeeRate` policy and `KaspadClient::fee_estimate_*`
  introduced in ADR-0018.

## Decision

### 1. Adaptive fees, sized from each transaction's exact mass

The planner (`payout-krc20::plan`) sizes both fees with the ADR-0018
`FeeRate` policy (`feerate × effective_mass`, floored at the relay minimum
`compute_mass × 100_000 / 1000`). The reveal is sized first (its mass is
driven by the redeem-script-and-data push in the signature script); the
commit is then funded mass-aware, reserving its fee out of treasury change and
**folding a dust/zero change output into the fee** — identical input-selection
and dust logic to the KAS `plan_batches` (`is_change_dust`), so the two engines
stay consistent.

The live `feerate_sompi_per_gram` is pulled once per settle pass from
`KaspadClient::fee_estimate_sompi_per_gram`; an RPC failure is **non-fatal**
and falls back to feerate 0 (the relay-minimum floor), exactly as in
`payout-kas`.

### 2. Fees are *frozen* onto the transfer row at first execution

A new migration adds nullable `commit_fee_sompi` / `reveal_fee_sompi`
(`BIGINT CHECK (… >= 0)`) to `krc20_pending_transfer`. The first time a
`pending` transfer is executed, the adaptively-resolved fees are persisted in
the **same transaction** that records the commit hash and marks it
`commit_submitted` — i.e. before the commit hits the wire. A `Krc20FeePolicy`
enum captures the two regimes:

- `Adaptive(FeeRate)` — used only on the first plan of a fresh transfer;
- `Frozen { commit_fee_sompi, reveal_fee_sompi }` — used by **every** later
  reconstruction (reveal build, commit drift-check/re-broadcast, reveal
  re-broadcast), loaded from the row.

`record_krc20_fees` writes once (guarded `… WHERE commit_fee_sompi IS NULL`),
so the first executor to record wins and no later reconstruction can overwrite
the frozen value. This makes the recorded commit/reveal ids **bit-identical**
across restarts regardless of how the node fee-rate moves afterwards.

The fixed-fee configuration is removed end-to-end: `Krc20FeeConfig`, the
`KATPOOL_KRC20_COMMIT_FEE_SOMPI` / `…_REVEAL_FEE_SOMPI` env knobs, and the
rehearsal's fee flags are gone — fees are no longer operator-tunable, by
design.

### 3. Testnet-10 go-live success criterion

NACHO does not exist on testnet-10, so the Kasplex indexer will always report
`insufficient balance` there. On tn10 the success criterion is therefore
**Kaspa L1 acceptance and deterministic transaction shape** (commit + reveal
accepted by the mempool; envelope/script bytes pinned by golden tests) — the
Kasplex result is ignored. On **mainnet** everything (including Kasplex
crediting) must verify with no exceptions.

## Decision Outcome

**Chosen: adaptive node fee-estimate + per-transfer fee persistence (Option B).**
It gives the KAS engine's congestion-adaptive reliability while preserving the
record-before-broadcast determinism the two-phase flow requires. The frozen
fee is the minimal state needed to reproduce both txids; persisting it in the
commit-submit transaction keeps the write crash-atomic.

### Consequences

- Positive: commit/reveal clear the mempool on the first try and adapt to
  congestion, with no fixed-fee retuning.
- Positive: crash-resume reproduces the exact recorded txids; `CommitDrift` /
  `RevealDrift` now fire only on genuine UTXO drift, never on fee re-quoting.
- Positive: KRC-20 and KAS share one `FeeRate` / dust implementation.
- Negative: a schema migration (two nullable columns) — additive and
  data-preserving (NULL until first execution), so safe to roll forward.
- Negative: a transfer cannot be re-priced once frozen. Accepted: the frozen
  fee already cleared the relay minimum at plan time; if it ever became
  un-relayable the transfer would be re-planned as a new cycle entry, not
  re-priced in place.

### Confirmation

- `payout-krc20` unit tests pin: fees meet/exceed the relay minimum and dwarf
  the legacy 10 000-sompi fee; a `Frozen` replay of an `Adaptive` plan
  reproduces identical inputs, change, fees, scripts, and reveal return
  (determinism); dust change folds into the fee.
- `payout-krc20` orchestration tests (testcontainer Postgres + mock kaspad)
  exercise the freeze-then-replay path through
  `pending → commit_submitted → reveal_submitted → completed`, including
  crash-before-broadcast re-broadcast and UTXO-drift refusal.
- Full workspace `clippy` / `fmt` / `test` green; tn10 on-chain verification of
  commit/reveal acceptance and shape pending deploy.

## Pros and Cons of the Options

### Option A: Relay-minimum policy (deterministic, no persistence)

- Good: no schema change; fee is a pure function of mass, so trivially
  reproducible on resume.
- Bad: never pays above the relay floor, so it under-pays in congestion — the
  opposite of the ADR-0018 posture the operator chose for KAS.

### Option B (chosen): Node fee-estimate + persist fees

- Good: congestion-adaptive *and* resume-deterministic; matches KAS.
- Bad: requires a migration and a freeze-on-first-execution write.

### Option C: Report only

- Bad: leaves a known rejection defect in the live path.

## More Information

- Builds on ADR-0018 (KAS fee policy, `FeeRate`, `fee_estimate_sompi_per_gram`).
- Related: ADR-0015 (inscription envelope), ADR-0016 (KAS→NACHO conversion &
  floor price), ADR-0017 (`tn10-toc3` version pinning).
- Mirrored consensus rules: rusty-kaspa
  `mining/src/mempool/check_transaction_standard.rs`,
  `mining/src/mempool/config.rs` (tag `tn10-toc3`).
