---
status: accepted
date: 2026-06-01
deciders: argonmining
---

# ADR-0017: Couple the kaspad node, kaspa-* crates, and Rust toolchain under one version bump

## Context and Problem Statement

The pool runs a Kaspa full node (`kaspad`) and links the rusty-kaspa
`kaspa-*` crates as a stratum bridge and consensus library (block
template handling, transaction serialization, sighash, txscript
verification, mass calculation). Block submission round-trips a block
through the `kaspa-rpc-core` types and is hashed/serialized by
`kaspa-consensus-core`.

These two surfaces — the running node binary and our linked crates —
**must speak the same consensus/wire version**. When they drift, the
failure is silent and severe: during the testnet-10 Toccata hardfork
(`tn10-toc3`, `1.2.1-toc.3`) the node enforced a new transaction-hashing
and SMT/seqcommit format while our crates were still pinned to `v1.1.0`.
The `v1.1.0` client silently dropped the new transaction fields
(`covenant`, `covenant_id`, the `TxInputMass` shape) during the RPC
round-trip, so every block the pool found was rejected with
`RuleError::BadMerkleRoot` — zero confirmed blocks, zero rewards, zero
NACHO rebates. See `docs/phase-1-acceptance.md` (kaspad-tn10 upgrade
incident).

A third pin is coupled to these: the **Rust toolchain**. rusty-kaspa
declares a workspace `rust-version`, and building their crates below it
fails (e.g. `tn10-toc3` pulls `wide 1.4.0` which requires Rust 1.89).

Before this ADR the version was spread across nine+ edit sites with no
single procedure, making a coordinated bump error-prone — exactly the
class of mistake that produced the `BadMerkleRoot` outage.

## Decision Drivers

* A node/crate version skew is a silent, production-breaking failure
  mode; it must be impossible to bump one without the others.
* We will bump this repeatedly: future testnet hardforks and the
  eventual mainnet cutover.
* The bump must be auditable (deterministic, reviewable diff) and fast
  to execute under incident pressure.
* Keep the surface minimal — no new build-time machinery.

## Considered Options

1. **Status quo** — hand-edit each site per bump, rely on memory.
2. **A single helper script + a documented coupled-pin checklist**,
   with the functional crate pins as the script's one source of truth.
3. **A build.rs / xtask that derives all pins from one constant.**

## Decision Outcome

**Chosen option: 2 (helper script + checklist).** The painful,
error-prone part — rewriting the rusty-kaspa git `tag` on every
`kaspa-*`/`kaspad` entry in the workspace `[dependencies]` — is
automated by [`scripts/set-kaspa-version.sh`](../../scripts/set-kaspa-version.sh),
which is the single source of truth for the functional pin. The script
also prints the coupled non-crate pins it deliberately does **not**
touch, so the operator updates them in one pass. Option 3 was rejected
as over-engineering: the non-crate pins (node SHA256, upstream MSRV)
cannot be derived from the tag without network calls, and a build.rs
would add opaque machinery to a security-sensitive build.

### The coupled pins (all move together, every bump)

| # | Location | Pin |
|---|----------|-----|
| 1 | `Cargo.toml` `[workspace.dependencies]` | rusty-kaspa git `tag` on all `kaspa-*` + `kaspad` (set by the script) |
| 2 | `rust-toolchain.toml` | `channel` = upstream `rust-version` for the tag |
| 3 | `Cargo.toml` `[workspace.package]` | `rust-version` = same |
| 4 | `clippy.toml` | `msrv` = same |
| 5 | `.github/workflows/ci.yml` | `toolchain:` on all jobs = same |
| 6 | `bridge/fuzz/Cargo.toml` | `rust-version` = same (separate workspace) |
| 7 | `ops/kaspad/install-kaspad-*.sh` | `TN*_RELEASE_TAG` + binary `SHA256` |
| 8 | `deny.toml` | re-run `cargo deny check`; reconcile advisories/licenses/sources for the new subgraph |

### Consequences

- Positive: a node/crate skew now requires ignoring an explicit
  checklist rather than forgetting one of nine scattered edits.
- Positive: the bump diff is deterministic and reviewable; the script
  is idempotent.
- Negative: the toolchain and node SHA256 are still manual. Mitigation:
  the script prints them as a checklist; CI's `toolchain` pins and the
  `cargo deny` sources/licenses checks fail loudly if a pin is missed.
- Negative: the vendored `bridge/` source stays at its original
  `v1.1.0` snapshot while its `kaspa-*` deps advance (it compiles and
  submits blocks correctly against `tn10-toc3` because serialization and
  hashing live in the crates, not the bridge source). Re-vendoring the
  bridge source is tracked separately in `bridge/UPSTREAM.md`.

### Confirmation

- `cargo deny check` (CI job `deny`) enforces the git-source allowlist
  and license set for the pinned subgraph.
- The six CI `toolchain` pins fail the build if the workspace
  `rust-version` exceeds them.
- Live confirmation that node and crates agree: blocks the pool finds
  confirm blue (no `BadMerkleRoot`), `share_allocation` rows accrue, and
  `nacho_rebate_accrual` increments — captured during the post-bump
  testnet-10 soak (Runbooks 18 & 19).

## Pros and Cons of the Options

### Option 1: Status quo (manual, no procedure)

- Good: no new files.
- Bad: produced the `BadMerkleRoot` outage; not auditable; slow under
  incident pressure.

### Option 2: Script + checklist

- Good: one command for the high-fan-out crate pins; explicit checklist
  for the rest; deterministic, reviewable, fast.
- Bad: non-crate pins remain manual (mitigated by CI gates).

### Option 3: build.rs / xtask deriving all pins

- Good: single literal source of truth.
- Bad: opaque build machinery in a security-sensitive build; cannot
  derive node SHA256 / upstream MSRV without network; high effort for
  an infrequent operation.

## More Information

- Procedure: [Runbook 20 — kaspa version bump](../runbooks/20-kaspa-version-bump.md)
- Tooling: [`scripts/set-kaspa-version.sh`](../../scripts/set-kaspa-version.sh)
- Provenance and re-vendor posture: [`bridge/UPSTREAM.md`](../../bridge/UPSTREAM.md)
- Fork rationale: [ADR-0002](0002-fork-rusty-kaspa-bridge.md)
- Toolchain pin rationale: [ADR-0001](0001-rust-first.md)
- Incident: `docs/phase-1-acceptance.md` (kaspad-tn10 → tn10-toc3 upgrade)
