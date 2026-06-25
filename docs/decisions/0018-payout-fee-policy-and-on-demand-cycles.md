---
status: accepted
date: 2026-06-02
deciders: argonmining
---

# ADR-0018: KAS payout fee policy, exact-fee finalization, cadence, and on-demand cycles

## Context and Problem Statement

The first live KAS payout cycle on the `tn10-toc3` soak never reached the
chain. Each broadcast was rejected by kaspad's mempool with
`RejectInsufficientFee` (e.g. *"transaction has 126400 fees which is under the
required amount of 203600 for compute mass 2036"*), and the treasury showed no
outbound transactions. Two defects combined:

1. **No fee was reserved.** The planner built the treasury change output as
   `change = input_sum − payout_sum`, leaving an implicit **zero fee**. kaspad
   relays nothing below the minimum relay fee.
2. **Mass was under-estimated even once a fee was reserved.** The offline
   planner's transaction shape diverged from the signed transaction kaspad
   actually validates, so the reserved fee was still too low. Two divergences
   were found and fixed (signed P2PK signature-script length; per-input
   `sig_op_count`), but chasing field-by-field parity across crates is fragile:
   any future mass-rule change (a new output type, a transaction-version bump,
   storage-mass on many small outputs) would silently re-open the gap.

Separately, the operator needs a clear answer to two policy questions that move
money and were previously implicit:

- **Cadence**: how often does the pool pay, and can an operator pay on demand?
- **Fee adaptivity**: does the fee track network congestion, and does it stay
  correct as transaction mass grows (many recipients, multiple batches)?

## Decision Drivers

* Value math must be integer-deterministic and mirror consensus exactly
  (ADR-0013 verification posture; kaspad mempool rules are the ground truth).
* The reserved fee must equal what kaspad's mempool charges for the **exact
  bytes** it validates — not an estimate that can drift from consensus.
* The fee must adapt to congestion and remain correct for any transaction mass
  (compute / storage / transient), at any recipient count.
* On-demand operation must not introduce a double-pay surface.
* Minimal, production-grade change; no new external dependencies.

## Decision

### 1. Adaptive node fee-estimate, floored at the relay minimum

A new `FeeRate` policy (`katpool-storagemass::fee`) sizes the fee as
`feerate × effective_mass`, floored at kaspad's minimum relay fee:

- `feerate_sompi_per_gram` is pulled live from the node's `get_fee_estimate`
  **priority bucket** (`RpcFeerateBucket.feerate`) via a new
  `KaspadClient::fee_estimate_sompi_per_gram`. A fee-estimate RPC failure is
  **non-fatal**: it falls back to feerate 0, i.e. the relay-minimum floor, so
  payouts still go out during a node hiccup.
- `effective_mass = max(compute, storage, transient)` — the same quantity
  kaspad uses to order the block template — so the fee/mass ratio clears the
  threshold on **every** mass dimension, including storage mass for many small
  outputs.
- The relay floor mirrors kaspad's `minimum_required_transaction_relay_fee`
  verbatim (`compute_mass × MIN_RELAY_TX_FEE_SOMPI_PER_KG / 1000`, floored at
  `MIN_RELAY_TX_FEE_SOMPI_PER_KG = 100_000` sompi/kg) and the dust rule mirrors
  `is_transaction_output_dust`. Both are reproduced (not imported — the mempool
  crate is not a dependency) from rusty-kaspa `tn10-toc3`.

The planner reserves this fee out of change and **folds a dust (or zero) change
output into the fee** (kaspad rejects dust outputs), re-measuring the no-change
shape so the recorded mass matches what is signed.

### 2. Exact fee sized from the *signed* transaction (divergence-proof)

The authoritative fee is computed in `payout-kas::execute` from the **signed
transaction's own mass** — the exact bytes kaspad validates — not from the
planner's reconstructed shape. `sign_batch_with_exact_fee` signs once
(in-memory, no external effect), measures the populated signed transaction,
recomputes `feerate × effective_mass` floored at the relay minimum, folds the
difference back into treasury change (dropping dust into the fee), and re-signs
only if the change moved. This makes the fee **structurally incapable of
diverging** from mempool policy regardless of future mass-rule changes.

The planner's shape (signed-P2PK script length `SIGNED_P2PK_SIG_SCRIPT_LEN =
66`, per-input `sig_op_count = 1`) is still corrected so batch *packing* against
the block-mass limit stays accurate; but correctness of the *fee* no longer
depends on that parity.

Submit failures are now surfaced loudly (per-batch `ERROR` with the kaspad
rejection message, plus an engine-tick `ERROR` listing all `submit_errors`) so a
stuck cycle can never be silent again.

### 3. Cadence is DAA-windowed; 6h on tn10; operator on-demand via CLI

Payout cycles are bucketed by **virtual DAA score**, not wall-clock
(`cycle_window`, ADR-era M4): the window `[start, end)` of width
`cycle_span_daa` is the cycle's identity, so every tick inside a bucket resumes
the same cycle and is safe across multiple instances (no clock skew). Because
DAA advances at the network block rate, the span is **block-rate-specific**: at
tn10's ~10 BPS, 6h ≈ `216_000` DAA (`KATPOOL_PAYOUT_CYCLE_SPAN_DAA`). The KAS
payout threshold is operator-tunable; tn10 runs **10 KAS**
(`KATPOOL_PAYOUT_THRESHOLD_SOMPI`), NACHO **10 KAS-worth**
(`KATPOOL_KRC20_MIN_PENDING_SOMPI`).

**Mainnet uses the identical defaults.** Mainnet has run 10 BPS since the
Crescendo hardfork (mainnet v1.0.0, 2025-05-05), so the same `216_000` DAA span
yields the same 6h cadence and the same 10 KAS thresholds. These values are now
the **built-in binary defaults** (`216_000` DAA span, `1_000_000_000` sompi for
both the KAS threshold and the NACHO `min_pending`), so a deployment that does
not override them via env inherits the decided policy rather than a stale
1-BPS-era cadence. The env files set them explicitly for documentation.

On-demand payouts are served by a new CLI subcommand,
`katpool payout run-now [--dry-run]`, which drives the **current** window's
cycle exactly as one daemon tick would (plan → broadcast → confirm →
reconcile), under the shared `payout-kas:kas-leader` advisory lock so only one
driver acts at a time. It deliberately does **not** open a second ad-hoc cycle
mid-window: KAS eligibility nets only `confirmed` payouts (a deliberate,
tested invariant — *"submitted but not confirmed — still fully payable"*), so a
parallel off-schedule cycle would double-pay in-flight balances. **Mid-window
ad-hoc top-ups are explicitly out of scope** (operator decision): `run-now`
always drives the current window's single cycle. Balances that accrue
mid-window are paid by the next window (or sooner if the operator lowers the
span); they are never lost.

## Decision Outcome

**Chosen: node fee-estimate + exact-fee-from-signed-transaction.** It is
production-grade (adapts to congestion), divergence-proof (the fee is derived
from the bytes kaspad validates), and crash-safe (signing has no external
effect; the recorded txid matches the broadcast transaction). The on-demand
trigger reuses the engine's existing tick under the same leader lock, adding no
new double-pay surface.

### Consequences

- Positive: payouts clear the mempool on the first try and adapt to congestion.
- Positive: the fee cannot silently drift from consensus as mass rules evolve.
- Positive: rejections are now loud; a stuck cycle is observable immediately.
- Positive: operators can pay on demand without waiting for the window.
- Negative: the 6h cadence is expressed in DAA, so the span must be recomputed
  for any network whose block rate differs from 10 BPS. Mitigation: documented
  in `ops/env` and the binary's module docs. tn10 and mainnet both run 10 BPS,
  so both share the `216_000` default; only a differently-paced network needs a
  recompute.
- Negative: `run-now` cannot pay balances that accrued mid-window (one cycle per
  window) — accepted by design (mid-window top-ups are out of scope).
  Mitigation: lower the span for a faster cadence.

### Confirmation

- First live KAS payout confirmed on-chain: txid
  `15de9cfb663956017e42b0b83c959d8e7e855cc969ad0c169d78964ccf4d574f`,
  `is_accepted = true`, mass `2036`, `sig_op_count = 1` — paying 17,395.16 KAS
  to the miner with change to treasury.
- Unit tests in `katpool-storagemass` (`fee`, `plan_batches`) pin the relay
  floor, effective-mass selection, dust folding, and zero-fee shape parity;
  `payout-kas` lifecycle tests exercise the exact-fee path; `katpool` arg-parser
  tests cover the subcommand. Full workspace `clippy`/`fmt`/`test` green.

## Pros and Cons of the Options

### Option 1: Static/hardcoded fee

- Good: trivial.
- Bad: under-pays in congestion (rejected) or over-pays otherwise; not
  enterprise-grade; needs manual retuning.

### Option 2: Planner-estimated fee only (field-by-field mass parity)

- Good: no second signing pass.
- Bad: fragile — already produced two divergences; any future mass-rule change
  silently re-opens the under-fee bug.

### Option 3 (chosen): Node fee-estimate + exact fee from the signed transaction

- Good: adaptive, divergence-proof, crash-safe, no new dependency.
- Bad: an extra in-memory sign+measure pass per batch (negligible cost; no
  external effect).

## More Information

- Related: ADR-0012 (fee model / tiers), ADR-0016 (KRC-20 conversion &
  floor-price), ADR-0017 (kaspa version pinning, `tn10-toc3`).
- Mirrored consensus rules: rusty-kaspa
  `mining/src/mempool/check_transaction_standard.rs`,
  `mining/src/mempool/config.rs` (tag `tn10-toc3`).
- Resolved: mainnet uses the same cadence/threshold as tn10 (both 10 BPS); these
  are now the built-in binary defaults (operator decision, 2026-06-01).
- Out of scope (operator decision): mid-window ad-hoc top-ups / netting in-flight
  (non-failed) payouts.
