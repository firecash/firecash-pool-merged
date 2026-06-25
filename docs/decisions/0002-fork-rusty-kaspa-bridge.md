---
status: accepted
date: 2026-05-25
deciders: argonmining
---

# ADR-0002: Fork the rusty-kaspa v1.1.0 in-house stratum bridge

## Context and Problem Statement

The new pool needs a stratum server that accepts ASIC connections,
validates shares, fetches block templates from kaspad, and submits
found blocks back. The legacy pool implemented stratum from scratch
in Bun/TypeScript on top of WASM kaspa bindings, which produced the
silent-failure modes and protocol drift we are rebuilding to avoid.

Three credible candidates exist in May 2026:

1. **`onemorebsmith/kaspa-stratum-bridge`** — Go, historically the
   community-default. Last meaningful commit: 2024-02-22. Two
   protocol generations behind. Effectively unmaintained.
2. **rusty-kaspa v1.1.0 `bridge/`** — Rust, in-tree, BETA, includes
   the modern protocol with multi-port, vardiff (shares-per-min +
   pow2_clamp), miner-type auto-detection (IceRiver / Bitmain /
   BzMiner / Goldshell), in-process kaspad mode, Prometheus metrics,
   web dashboard. Active maintenance by the rusty-kaspa team.
3. **Build native from rusty-kaspa crates** — use
   `kaspa-grpc-client`, `kaspa-pow`, `kaspa-txscript` as libraries
   and re-implement stratum from scratch.

We need per-miner/wallet share accounting and PROP allocation,
which the bridge does NOT provide. That layer is ours regardless of
which option we pick.

## Decision Drivers

- Time-to-working-mainnet — we are replacing a live pool
- Maintainability — minimal divergence from upstream rusty-kaspa
- Protocol compliance — must accept all common ASIC miner variants
- Observability — Prometheus + structured share accounting
- Risk surface — BETA status is acceptable if testable; abandoned
  code is not

## Considered Options

1. Fork the rusty-kaspa v1.1.0 `bridge/` into our monorepo via
   `git subtree`, add intrusive hooks for share/block events to be
   consumed by our accountant.
2. Vendor the bridge via git submodule, extend via a public trait
   callback.
3. Build a native Rust stratum from scratch using rusty-kaspa
   crates as libraries.
4. Use `onemorebsmith/kaspa-stratum-bridge` (Go). Rejected on
   maintenance grounds.

## Decision Outcome

**Chosen option: 1 (fork via subtree + intrusive hooks).** This is
the lowest-risk path to a working pool while keeping us on the
official rusty-kaspa code path.

We add exactly two intrusive patches to the upstream code:

- A `tokio::sync::broadcast::Sender<PoolEvent>` injected into
  `share_handler.rs` that publishes `ShareCredited`,
  `ShareRejected`, `BlockFound`, and `BlockAccepted` events for
  our accountant to consume.
- Anti-abuse hooks (per-IP rate limit, per-IP connection cap,
  address-allowlist/denylist) layered as middleware ahead of the
  share handler.

We do **not** modify the protocol parser, the PoW validator, the
vardiff logic, or the miner-type detection. These remain identical
to upstream so future upstream improvements merge cleanly.

### Consequences

- Positive: we inherit the maintained protocol implementation;
  vardiff, miner-type detection, and metrics work day one.
- Positive: in-process kaspad mode removes a network hop and an IPC
  dependency.
- Positive: keeps us on the canonical rusty-kaspa code path.
- Negative: BETA status means upstream may change semantics. Mitigated
  by pinning to commit `e97070f` (v1.1.0) and tracking upstream
  divergence in [`bridge/UPSTREAM.md`](../../bridge/UPSTREAM.md).
- Negative: forking creates merge work. Mitigated by keeping our
  patches tightly scoped (two changes only) and rebasing
  infrequently (only when we want a specific upstream fix).

### Confirmation

- `bridge/UPSTREAM.md` records the upstream commit we vendored.
- The two intrusive patches are the only `+` lines in the diff
  against upstream when measured by `git diff e97070f -- bridge/`.
- Integration test in Phase 1 spawns the bridge in-process and
  asserts events fire on the broadcast channel.

## Pros and Cons of the Options

### Option 1: Fork the rusty-kaspa bridge

- Good: maintained upstream; clean merge path
- Good: shared mining logic + metrics + dashboard out of the box
- Good: in-process kaspad mode available
- Bad: BETA; we have to track upstream changes
- Bad: requires intrusive patches for our event hooks

### Option 2: Submodule + trait-callback extension

- Good: cleaner upstream sync (no patches in our tree)
- Bad: bridge does not currently expose stable trait extension
  points for share events; we'd be PRing them to upstream and
  blocking on review
- Bad: more design overhead than #1 for similar end-state

### Option 3: Native from rusty-kaspa crates

- Good: highest control
- Bad: ~5× the engineering effort
- Bad: have to reimplement vardiff, multi-miner-type protocol
  variants, dashboard
- Bad: not better in any way we can name

### Option 4: onemorebsmith Go bridge

- Bad: abandoned (last commit 2024-02-22)
- Bad: two protocol generations behind
- Rejected.

## Amendment (2026-06-01)

The vendored `bridge/` source remains the `v1.1.0` snapshot, but the
linked `kaspa-*`/`kaspad` **dependency** tag has advanced to `tn10-toc3`
to track the testnet-10 Toccata hardfork (a `v1.1.0` crate pin against a
toc3 node rejected every found block with `BadMerkleRoot`). Coupling and
the bump procedure for the node/crate/toolchain pins are now governed by
[ADR-0017](0017-kaspa-version-pinning.md) and
[Runbook 20](../runbooks/20-kaspa-version-bump.md). Details:
[`bridge/UPSTREAM.md`](../../bridge/UPSTREAM.md).

## More Information

- Upstream PR introducing the bridge: kaspanet/rusty-kaspa#793
- Bridge docs: `bridge/docs/README.md` in upstream
- Companion ADR: [0001 (Rust-first)](0001-rust-first.md)
- Version coupling: [ADR-0017](0017-kaspa-version-pinning.md)
