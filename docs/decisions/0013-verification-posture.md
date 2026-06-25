---
status: accepted
date: 2026-05-26
deciders: argonmining
---

# ADR-0013: Verification posture

## Context and Problem Statement

"Enterprise-grade" can mean a lot of things. We need a written
record of *what* we verify, *how* we verify it, what we
deliberately don't verify (yet), and *why* — both to keep the
agent honest as it ships new code, and to give a reviewer a
clear yardstick.

The system is a pool that handles miners' money. Bugs in the
allocation, payout, or schema layers are not equivalent to bugs
in (say) the rendering layer of a SaaS dashboard: a wrong number
in `share_allocation` is a real-world loss for a real miner, and
the legacy stack's 2024 NACHO double-pay incident proves the
class of failure is operational, not hypothetical.

## Decision

The katpool rebuild commits to **seven verification layers**.
Each layer's purpose, mechanism, and explicit non-goals are
spelled out below. PRs that would weaken any layer require an
amendment to this ADR.

### Layer 1 — Compiler-side static guarantees

- `#![forbid(unsafe_code)]` workspace-wide. No exceptions.
- `#![deny(...)]` for every clippy lint that catches a real bug
  class: `unwrap_used`, `expect_used`, `panic`, `indexing_slicing`,
  `float_arithmetic`, `print_stdout`, `print_stderr`,
  `integer_division`, `todo`, `unimplemented`, `dbg_macro`.
  Relaxations live in `#[cfg(test)]` only, scoped to a single
  module or test crate, never workspace-wide.
- `#[non_exhaustive]` on every domain enum that crosses a crate
  boundary; consumers use `TryFrom` (not infallible `From`) so the
  build fails on upstream enum drift before runtime sees it.
- Newtype wrappers (`WalletId`, `BlockId`, `CorrelationId`,
  `BlockHash`, `WalletAddress`, ...) make wrong-type-at-call-site
  bugs unrepresentable.

### Layer 2 — Schema-side runtime guarantees

- Foreign keys on every relationship. Cascading deletes only
  where the parent's identity is meaningless without the child;
  `ON DELETE RESTRICT` everywhere else.
- `CHECK` constraints encode every invariant the schema can:
  `share_allocation_balance` (`gross = pool_fee + nacho_accrual +
  net_payout`), `payout_lifecycle_order`, `block_lifecycle_order`,
  `share_window_range`, etc. The CHECK is the last line of defence
  if the Rust code drifts.
- `UNIQUE` constraints on every natural-key idempotency surface:
  `block.hash`, `payout (cycle_id, wallet_id)`,
  `payout_cycle.idempotency_key`, `krc20_pending_transfer.payout_id`,
  `share_window (wallet_id, daa_start, daa_end)`.
- Postgres enums for every state machine, with the variant labels
  byte-for-byte matched by `sqlx::Type` declarations on the Rust
  side.

### Layer 3 — Integration tests against real Postgres

Every repo function and every cross-aggregate operation is
exercised via testcontainers (not mocks). The accountant + db
+ importer suites currently total 80+ tests, every one running
against an ephemeral Postgres container.

Mocks are banned in DB tests. They produce green-test cheerfulness
that doesn't survive contact with the actual driver, the actual
query planner, or the actual constraint engine.

### Layer 4 — Property tests for money math

`accountant/tests/allocation_properties.rs` exercises
`FeeConfig::compute_allocation` over thousands of randomly-chosen
`(gross, topline_bps, tier)` triples per CI run, asserting:

- the balance equation (`gross == pool_fee + nacho_accrual +
  net_payout`),
- non-negativity of every component,
- tier monotonicity (elite ≥ standard in NACHO accrual; net KAS
  payout identical across tiers),
- topline monotonicity (higher topline ⇒ lower net),
- audit-trail-field faithfulness (the `applied_*` fields always
  match the inputs that produced them),
- boundary cases (zero gross, zero topline, max gross).

Property tests are the appropriate verification layer for any
function that's both pure and money-sensitive — they catch off-by-
one errors a human test-writer would not think to enumerate.

Every PR that adds new money math MUST extend this suite.

### Layer 5 — Replay-determinism tests

`accountant/tests/replay_determinism.rs` feeds an identical event
stream to two independent consumers (independent Postgres
instances, fresh schemas) and asserts the two resulting databases
are byte-equal on the content the consumer wrote. Catches:

- non-determinism via wallclock leaks into row identity,
- hidden ordering dependencies in the event handler,
- any unobservable randomness introduced by future refactors.

This is the single strongest test we have for the implicit
contract "given the same input stream, the accountant produces
the same DB state every time".

### Layer 6 — Enum-parity round-trips

`crates/katpool-db/tests/enum_parity.rs` round-trips every
variant of every `sqlx::Type` enum through a temporary postgres
table that declares the column with the matching `type_name`. A
postgres-enum variant in the wrong on-disk position silently
shifts every old row's value — this test catches that class of
bug before migration. Exhaustiveness-guard `match`es in the same
file fail the build when a Rust variant is added without
extending the test loop.

### Layer 7 — Supply-chain & code-hygiene gates

CI runs five non-test gates on every PR:

1. **`cargo fmt`** — formatting consistency.
2. **`cargo clippy --workspace --all-targets -- -D warnings`** —
   every lint above plus the standard pedantic / nursery selection.
3. **`cargo doc --no-deps -- -D warnings`** — rustdoc links and
   `missing_docs` enforced on public surfaces.
4. **`cargo deny check`** — advisories (RustSec), bans, licences,
   sources (`unknown-registry = deny`, `unknown-git = deny`).
5. **`cargo machete`** — unused-dependency detection. Scaffold
   crates declare their reserved-for-phase-X dep set via
   `[package.metadata.cargo-machete] ignored = [...]`; the
   metadata block carries a comment naming the phase that
   activates each dep.
6. **`typos`** — typo detection with a kaspa-specific
   `_typos.toml` whitelist.

A separate non-blocking job runs `cargo tarpaulin` against the
workspace to produce coverage artefacts. Coverage is informational
only — gating on a number would distort incentives.

## Explicitly out of scope (today)

These are deliberate omissions, not oversights:

- **Mutation testing** (`cargo-mutants`). Slow; would inflate CI
  cost without changing the failure modes our gates already catch.
  Revisit pre-cutover (Phase 7 hardening).
- **Concurrency stress tests.** The accountant's consumer is
  single-tasked by design; M3 adds the allocation engine on a
  separate task. Stress testing the cross-task model is a Phase 7
  task tied to the load-test suite.
- **Failure-injection harness.** `katpool-fault-injection` is a
  scaffold crate awaiting Phase 9. Pre-cutover load testing wires
  it in.
- **End-to-end production replay.** The legacy importer's scale
  test runs at 1:50 of production scale on CI; the full-scale
  rehearsal happens at T-24h per the cutover importer (evidence under
  `cutover-evidence/`), not on every PR.
- **Compile-time-checked sqlx queries** (`query!` macro). Requires
  a live `DATABASE_URL` or a committed `.sqlx/` offline cache;
  the testcontainer integration tests already give us
  fail-at-test-run for every query the codebase ships, which is
  good enough until repo-layer churn rises.

## Consequences

### Positive

- Every PR runs through the seven layers above. A reviewer can
  assume that landed code has passed each gate without
  re-verifying.
- The verification posture is reviewable in one document —
  changes to it require a discussion, not a quiet PR.
- Property tests on money math give us a categorically stronger
  guarantee than human-curated unit tests do, for the part of the
  system where bugs translate most directly into real-world loss.

### Negative

- CI time on every PR is now in the ~5-10 minute range. The
  testcontainer-based integration suite dominates; we accept this
  trade because the alternative (mocks) is materially weaker.
- The seven-layer set adds friction to "quick fix" PRs. Acceptable:
  no PR in this project is small enough to skip them.

## Re-evaluation triggers

This ADR is re-opened if:

- We ever consider relaxing `forbid(unsafe_code)`.
- A bug class ships that none of layers 1–7 would have caught.
- The CI gate set materially changes (a new tool added, an
  existing tool deprecated).
- Pre-cutover (Phase 7) review surfaces gaps the cutover playbook
  needs filled.

The Phase 7 close-out PR must include a re-confirmation that this
ADR still represents the project's posture, signed off in the PR
body.
