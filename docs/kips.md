# KIP-9 and KIP-13 Implementation Notes

This document is the engineering reference for how we implement the
Kaspa Improvement Proposals that govern transaction validity. Anyone
touching `katpool-storagemass`, `payout-kas`, or `payout-krc20` should
read this first.

Authoritative sources:

- KIP-9 (Storage Mass): <https://github.com/kaspanet/kips/blob/master/kip-0009.md>
- KIP-13 (Transient Storage Mass): <https://github.com/kaspanet/kips/blob/master/kip-0013.md>
- KIP-14 (Crescendo hardfork bundle): <https://github.com/kaspanet/kips/blob/master/kip-0014.md>
- rusty-kaspa reference implementation: `consensus/core/src/mass.rs`
- Storage mass guide (Aspectron docs):
  <https://kaspa.aspectron.org/transactions/fees/storage-mass.html>

## 1. Why this matters to us

The May 1 2026 production NACHO payout failure was caused by
constructing transactions that the network rejected with
`Storage mass exceeds maximum`. KIP-9 (and KIP-13, added in the
Crescendo hardfork) define what makes a transaction valid for
inclusion in a block.

The legacy `katpool-payment` had no mass-aware planning — it just
batched 35 outputs per KAS payout transaction and hoped. When the
on-chain UTXO distribution changed, transactions started failing
silently. The new pool computes mass before signing and rejects any
plan that would not fit.

## 2. The three masses

A Kaspa transaction has three independent "masses". A block can
contain transactions only if the sum of each mass, independently, is
≤ the block mass limit (500,000 grams).

| Mass | Computed by | Bounds |
|---|---|---|
| `compute_mass` | Per-transaction; reflects compute cost (sig ops, script ops) | Function of inputs and script complexity |
| `storage_mass` | Per-transaction; reflects persistent UTXO-set growth | Function of input/output values and counts (KIP-9 formula) |
| `transient_storage_mass` | Per-transaction; reflects block-byte size | `serialized_size * 4` (KIP-13) |

For mempool / fee-rate purposes, a transaction's effective mass is

```text
mass = max(compute_mass, storage_mass, transient_storage_mass)
```

Validity per block requires each of the three to fit independently.

## 3. The KIP-9 storage_mass formula

Let `H` denote a function with the property that the storage mass
captures the dust-prevention intent: small outputs cost a lot;
combining UTXOs is cheap.

Empirically (per the Aspectron guide):

- **Outputs > 100 KAS each**: storage mass is effectively zero per
  output; the count is unconstrained.
- **Outputs > 10 KAS each**: ~100 outputs hit the 100k-mass mark.
- **Outputs > 1 KAS each**: ~10 outputs hit the limit.
- **Output < 0.019 KAS**: rejected entirely (the absolute floor).
- **Output count ≤ input count**: combining or 1:1 transfers are
  essentially free (mass-wise) regardless of values.
- **Output count > input count, with small outputs**: mass
  explodes rapidly. This is the "fanout" case the formula penalises.

The exact formula and its constants are encoded in
`crates/katpool-storagemass/src/lib.rs` (Phase 4). Every release of
that crate is property-tested with `proptest` against the rusty-kaspa
reference implementation's outputs for random input/output sets.

## 4. KIP-13 transient_storage_mass

Activated in the Crescendo hardfork (mainnet 2025-05-05). For us,
this is the simpler of the three:

```text
transient_storage_mass = serialized_size(tx) * 4
```

`serialized_size` is the wire-format size in bytes. We compute this
by actually serialising the candidate transaction; there is no
formula to estimate it.

## 5. Practical implications for payout planning

### 5.1 KAS batch payouts

For a daily KAS payout to N miners:

- Each miner is one output. Combine UTXOs as inputs to reach the
  total value needed.
- **Rule of thumb**: number of outputs in a single tx ≤ 10 × number
  of inputs.
- Avoid outputs below 1 KAS unless the rest of the transaction has
  many large inputs to dilute the dust mass.
- Use a single change output (or none, if outputs sum exactly).

The `katpool-storagemass::plan_batches(eligible, available_utxos)`
function returns a `Vec<Tx>` where every element passes the
three-mass check. If a recipient's payout would individually create a
sub-floor output (< 0.019 KAS), it is held until next cycle.

### 5.2 KRC-20 reveal transactions

KRC-20 reveal transactions are inherently small (one input from the
commit, one or two outputs: recipient + optional change). They almost
never hit storage-mass limits but routinely hit `transient_storage_mass`
because the redeem-script-and-data path bloats serialised size.

Approach:

- Use exactly one recipient per reveal tx (keeps the envelope small).
- Use the same mass-aware planner — the answer is usually "one tx per
  recipient" but the planner returns a precise verdict.

The KRC-20 planner lives in `payout-krc20::plan` (`plan_commit_reveal`).
Unlike the KAS planner — which evaluates *unsigned* shapes because KAS
payouts are storage-mass-dominated — the KRC-20 planner sizes the
**signature scripts to their signed length** before evaluating, because the
reveal's `transient_storage_mass` is driven by the redeem-script-and-data
push that only appears once the input is "signed". A standard Schnorr push
is 66 bytes (rusty-kaspa `wallet::tx::mass::SIGNATURE_SIZE`); the reveal
additionally carries the canonical push of the full redeem script. The
planner builds the commit (treasury inputs → P2SH commit output + change)
and the minimal 1-input/1-output reveal, then asserts every mass fits
`max_block_mass` independently.

Note the planner also surfaces KIP-9 anti-dust: a commit *change* output
that clears the economic floor can still exceed storage mass when funded
by a much larger input. The planner reports this as a mass failure rather
than emitting an unminable transaction; choosing mass-appropriate funding
UTXOs is the execute/maintain layers' job (§5.3–§5.4), not the planner's.

### 5.4 Plan vs execute (treasury UTXO lifecycle)

Two layers — do not conflate them:

| Layer | Crate | Responsibility |
|---|---|---|
| **Plan** | `katpool-storagemass` | Given recipients + a starting UTXO snapshot, produce mass-valid batches. Re-injects each batch's **change** as a *planning-only* virtual UTXO (`PLANNING_VIRTUAL_TXID_HEX`) so one large treasury coin can fund many txs in the plan. Virtual coins are never signed or broadcast. |
| **Execute** | `payout-kas` (M4.6+) | Before **each** sign/submit: fetch the live treasury UTXO set from kaspad, map planned inputs to real outpoints (replace virtual change with confirmed change from the prior tx), re-check mass, then sign. Abort the cycle on mismatch — never guess. |
| **Maintain** | `payout-kas` background job | When the treasury accumulates many small UTXOs (§5.3), run `sweep_to_self` / compounding so the input side stays mass-efficient. |

A cycle therefore: **plan once** (dry-run + idempotency rows) → **execute sequentially** with live UTXO refresh per batch → optional consolidation between cycles.

### 5.3 UTXO maintenance

When the treasury's UTXO set drifts toward many small dust outputs
(common after long stretches of mining-receive activity), the planner
returns ever-smaller batches because the input side becomes
mass-constrained. The runbook
[`07-storage-mass-rejection-burst.md`](runbooks/07-storage-mass-rejection-burst.md)
covers the manual `sweep_to_self` operation that consolidates UTXOs
into fewer, larger ones. Phase 4 includes a scheduled background job
that runs this consolidation automatically when the UTXO count
exceeds a threshold.

## 6. Testing

`crates/katpool-storagemass` is tested at three levels:

1. **Unit**: hand-rolled tx cases from the KIPs and the Aspectron
   guide; each case has an explicit "should fit" or "should fail"
   assertion.
2. **Property** (`proptest`): random (`n_inputs, n_outputs,
   input_values, output_values`) sets; the property is that the mass
   we compute matches what rusty-kaspa's `consensus::core::mass`
   computes for the same transaction.
3. **Fuzz** (`cargo-fuzz`): byte-level fuzzing of the input encoder
   used by `transient_storage_mass`; any panic or non-deterministic
   result is a failure.

Replay test: the May 1 production NACHO failure scenario is captured
as a regression test in `tests/replay/may_01_storage_mass.rs`. The
test takes the exact UTXO set and recipient list from that day and
confirms the new planner produces a set of batches that the network
would accept.

## 7. References

- KIP-9: <https://github.com/kaspanet/kips/blob/master/kip-0009.md>
- KIP-13: <https://github.com/kaspanet/kips/blob/master/kip-0013.md>
- KIP-14: <https://github.com/kaspanet/kips/blob/master/kip-0014.md>
- rusty-kaspa storage mass implementation: `consensus/core/src/mass.rs`
- Aspectron storage-mass guide:
  <https://kaspa.aspectron.org/transactions/fees/storage-mass.html>
- Kaspa ecosystem updates (incl. KIP-9 worked examples):
  <https://github.com/kaspa-ng/kaspa-ecosystem-updates>
