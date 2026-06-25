---
status: accepted
date: 2026-05-26
revised: 2026-06-01
deciders: argonmining
---

# ADR-0014: Block maturity tracker architecture

## Revision history

- **2026-05-26** — original design (polling tracker; blue-score-depth
  maturity; reward extracted from the found block's own coinbase;
  `is_blue`/`is_chain_block` for confirmation).
- **2026-06-01** — **corrected against rusty-kaspa `tn10-toc3`
  consensus.** The original maturity and reward model was wrong on
  three counts (see [Correction](#correction-2026-06-01)). The tracker
  is now a **two-sweep** design: GHOSTDAG-colour block telemetry plus
  UTXO-anchored coinbase-reward allocation. Sections 2–5 below describe
  the corrected design; the superseded text is preserved in the
  Correction section for the record.

## Context and Problem Statement

Phase 3 M3 landed the [`AllocationEngine`], which converts a matured
reward into per-wallet `share_allocation` rows. It takes the reward
amount as an argument and assumes the caller has already decided the
reward is realised.

Something needs to *be* that caller. It must:

1. Notice when kaspad confirms a `submitted_to_node` block as **blue**
   (the pool earned its work) versus **red** / never-merged (no
   reward) — operator-facing lifecycle telemetry.
2. Notice when the pool has actually **been paid** a coinbase reward,
   for an exact sompi amount, and only once that reward is spendable
   (consensus coinbase-maturity reached).
3. Hand each matured reward to
   [`AllocationEngine::allocate_coinbase_reward`].

This ADR captures the architectural decisions for the
`MaturityTracker` that does all of the above.

## Correction (2026-06-01)

The original design matured a *found block* and allocated its reward by
parsing **that block's own coinbase**, gating maturity on a
**blue-score depth of 100** and treating `is_blue`/`is_chain_block` as
the reward condition. Validated against rusty-kaspa `tn10-toc3`, all
three are wrong:

1. **A block's own coinbase does not pay that block.** A block `B`'s
   coinbase pays the miners of the blocks `B` *merges*; `B`'s own
   reward is paid in the coinbase of a later chain block `C` that
   merges `B` as blue
   (`consensus/src/processes/coinbase.rs`). Parsing `B`'s coinbase
   attributes the wrong reward to the wrong work.
2. **Coinbase maturity is a DAA-score depth of 1000, not a blue-score
   depth of 100.** `BlockrateParams::coinbase_maturity = BPS(10) ×
   COINBASE_MATURITY_SECONDS(100) = 1000`. A UTXO is spendable when
   `virtual_daa_score ≥ utxo.block_daa_score + 1000`.
3. **Reward condition is GHOSTDAG colour, not chain membership.** A
   block earns its reward iff it is **blue** (in some chain block's
   blue mergeset), reported by `RpcApi::get_current_block_color`.
   `is_chain_block` (selected-parent-chain membership) is a far
   narrower set; using it strands almost every rewarded block in
   `submitted_to_node` forever (the original stall).

The corrected model takes the **coinbase UTXO set credited to the pool
address** as ground truth for realised reward — the only exact,
attribution-free source of "the pool was paid N sompi". Block lifecycle
becomes pure telemetry, decoupled from money.

## Decision

### 1. Polling, not subscription

Kaspa's gRPC API supports both poll and notification-stream patterns.
The tracker uses **polling** for three reasons:

- **Simpler restart semantics.** A polling loop has no in-flight
  notification queue to reconcile on restart. The state of the world is
  fully derivable from the DB plus kaspad's current DAG and UTXO set.
- **Bounded kaspad load.** Polling at a known cadence gives the
  operator a predictable RPS against kaspad. A notification stream that
  suddenly fans out can knock down a shared-VPS kaspad.
- **Trivial back-pressure.** The tracker processes at most
  `cfg.batch_size` block transitions and reward allocations per sweep.

Default cadence: 15s. Operator-tunable via `MaturityConfig`.

### 2. kaspad access behind a trait

The tracker depends on a [`KaspadClient`] trait, not on
`kaspa-grpc-client` directly. The corrected trait surface is exactly
three methods:

```text
get_virtual_daa_score()     -> u64                 // maturity gate
get_block_color(hash)       -> Blue | Red | NotYetMerged
get_pool_coinbase_utxos()   -> [CoinbaseUtxo]       // realised reward
```

`CoinbaseUtxo` carries `{transaction_id, index, amount_sompi,
block_daa_score}` — outpoint identity, exact value, and the accepting
chain block's DAA score (both the maturity gate and the PROP window key
off it).

**Why a trait, not a direct dependency:** the tracker's two-sweep logic
is the bulk of the code under review. Stubbing kaspad behind a trait
lets the test suite cover every transition and maturity path
deterministically against an in-memory fake (`FakeKaspad` in
`accountant/tests/maturity_tracker.rs`) without standing up a real
kaspad-tn10 instance. The real gRPC-backed `KaspadGrpcClient`
(`accountant/src/kaspad_grpc.rs`) maps these to
`get_block_dag_info`, `get_current_block_color`, and
`get_utxos_by_addresses` (requires kaspad `--utxoindex`).

### 3. Two independent sweeps per tick

Each sweep reads `virtual_daa_score` once, then runs two independent,
idempotent passes:

**A. Block-lifecycle telemetry.** For each `submitted_to_node` block
(oldest-first, FIFO — see below), ask `get_block_color`:

- `Blue` → `confirmed_blue` (the pool earned its work).
- `Red` → `orphaned` (merged red; no reward).
- `NotYetMerged` → stay `submitted_to_node`, **unless** the block is
  already past `coinbase_maturity` DAA depth, in which case age it out
  to `orphaned` (an honest block merges well before this; a block still
  unmerged this deep is lost, and without age-out it would sit forever).

This pass drives **no money**. Because every block resolves to a
terminal state, oldest-first selection (`ORDER BY found_at ASC`) drains
the backlog FIFO and cannot head-of-line block — fixing the original
"window churn" where `DESC LIMIT n` starved older blocks.

**B. Coinbase-reward allocation.** Scan
`get_pool_coinbase_utxos`. Each UTXO with `virtual_daa_score ≥
block_daa_score + coinbase_maturity` is recorded in `coinbase_reward`
(idempotent by outpoint) and the engine is invoked for any
not-yet-allocated reward. The UTXO set is ground truth, so DAG re-orgs
need no special handling: a reward that re-orgs out before maturity
simply never appears.

If the engine call errors mid-flight, its own transaction rolls back;
the tracker logs + counts but unwinds nothing else. The next sweep
retries — the engine's `allocated_at` gate makes re-runs no-ops.

### 4. Window-size policy

Each matured reward triggers a PROP allocation over a DAA window ending
at the UTXO's `block_daa_score` (the accepting chain block's DAA
score). `daa_start = block_daa_score − cfg.window_daa_span`. Default
span: 600 DAA scores.

**Why 600:** post-Crescendo BPS is 10 → 600 DAA ≈ 60 seconds, aligning
with the legacy pool's PPLNS convention without locking in a specific
share-count constant. Operator-tunable via `MaturityConfig`.

The window-span is **NOT** automatically tied to coinbase maturity.
They are independent parameters:

- `coinbase_maturity` controls *when* a reward is spendable (a
  consensus parameter; 1000 DAA on mainnet and tn10).
- `window_daa_span` controls *which shares* contribute to that reward's
  allocation (a fairness parameter set by the pool).

### 5. The coinbase UTXO is the unit of reward

The `coinbase_reward` table has one row per matured coinbase UTXO
credited to the pool address, anchored by the outpoint
`(transaction_id, index)` — naturally unique, giving exactly-once
allocation. `share_allocation` references `coinbase_reward_id`, not
`block_id`. The `block` table is pure lifecycle telemetry and plays no
part in allocation.

"Which UTXOs belong to the pool" is policy that lives entirely inside
the `KaspadClient` implementation (`get_utxos_by_addresses` over the
configured pool address(es), filtered to `is_coinbase`). The tracker
treats each `CoinbaseUtxo` as an opaque exact amount.

### 6. Per-item error isolation, whole-sweep error fail-fast

- **Per-item errors** (kaspad transient failure on `get_block_color`,
  a single allocation failing) are logged + counted in `SweepStats`
  and the sweep continues.
- **Whole-sweep errors** (kaspad transport down for
  `get_virtual_daa_score`, DB pool unavailable) abort the sweep with a
  `TrackerError`. `run_loop` catches and logs but doesn't kill the
  loop — the next tick retries.

### 7. `tokio::sync::watch` for shutdown

`run_loop` takes a `watch::Receiver<bool>`. Setting the channel to
`true` from the parent task causes the loop to exit cleanly at the next
select. Tested in `run_loop_exits_cleanly_on_shutdown_signal`.

`watch` is preferred over `oneshot` (the receiver outlives multiple
restarts) and over `CancellationToken` (we already depend on `tokio`).

## Consequences

### Positive

- Allocation is anchored on realised, exact, exactly-once reward — no
  attribution guesswork, and DAG re-orgs are handled by construction.
- Block lifecycle and money are decoupled: a telemetry bug cannot
  cause an incorrect payout, and an allocation bug cannot corrupt
  lifecycle state.
- The full two-sweep state machine has deterministic coverage against
  the in-memory fake (12 tracker tests, 9 allocation-engine tests).
- Oldest-first FIFO drain cannot head-of-line block.
- Operator controls (poll cadence, coinbase maturity, window span,
  batch size) are all on one config struct.

### Negative

- Requires kaspad `--utxoindex` for `get_utxos_by_addresses`. Verified
  enabled on kaspad-tn10; documented as a deploy precondition.
- 15-second polling cadence means worst-case 15s additional latency
  beyond consensus maturity. Acceptable for PROP allocation.

### Out of scope

- **Real-time push from kaspad** (a long-lived notification stream).
- **Multi-address / change-address policy** beyond the configured pool
  address set.

## Re-evaluation triggers

- A real kaspad call is needed that doesn't fit the three-method
  surface.
- Polling at 15s shows up as a hot signal on kaspad's load graphs.
- The window-span default needs to change after load testing.

[`AllocationEngine`]: ../../accountant/src/allocation.rs
[`AllocationEngine::allocate_coinbase_reward`]: ../../accountant/src/allocation.rs
[`KaspadClient`]: ../../accountant/src/maturity.rs
