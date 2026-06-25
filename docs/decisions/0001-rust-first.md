---
status: accepted
date: 2026-05-25
deciders: argonmining
---

# ADR-0001: Use Rust as the primary language for the new pool

## Context and Problem Statement

The legacy pool is a mixed Bun (TypeScript) + Go + Postgres stack that
talks to `rusty-kaspa` via WASM bindings. In production we have hit
several classes of failure that trace directly to the language and
runtime mix:

- Recurring `Unreachable code should not be executed` panics from the
  WASM `kaspa.js` bindings during normal payout cycles.
- A silent block-submission failure path because the `submit_block`
  catch returned `undefined` and Bun stack-traces did not flag it.
- Bun-specific quirks (canary upgrades pinned in the Dockerfile) and
  Puppeteer-headless-Chrome scraping for a price oracle.
- Cross-repo coupling between four separately-versioned Bun/Go services,
  each with its own divergent fork lineage (`argonmining` vs.
  `Nacho-the-Kat`).

We must pick a language for the rebuild that gives us strong types,
deterministic builds, and native integration with `rusty-kaspa` (the
source-of-truth implementation of the Kaspa node, which is itself in
Rust).

## Decision Drivers

- Eliminate WASM-boundary failure modes
- Reproducible builds, signed releases
- Single-binary deployment fits the operational budget
- Tight integration with `rusty-kaspa` Rust crates
  (`kaspa-grpc-client`, `kaspa-consensus-core`, `kaspa-txscript`,
  `kaspa-wallet-core`) without WASM intermediation
- Strong typing of money math (no floats; explicit `Sompi` newtype)
- Memory-safety guarantees relevant to treasury-key handling
- Hireability / community: Rust has a healthy crypto ecosystem in 2026

## Considered Options

1. **Rust-first across the whole stack.**
2. **Hybrid: Rust for new core, keep legacy TS for KRC-20.** Carried
   the Bun/WASM/Puppeteer dependency forward in a smaller scope.
3. **Go.** Compiled, single-binary, fast, but no native rusty-kaspa
   integration; we'd be back to the WASM or a re-implementation in Go.
4. **Continue Bun/TypeScript.** No-op. Rejected because the legacy
   failure modes are language-level.

## Decision Outcome

**Chosen option: 1 (Rust-first).** It is the only option that gives
us native integration with rusty-kaspa, eliminates WASM-related
failure modes, and produces a single signed binary suitable for our
custody and ops constraints.

The one known cost of Rust-first is the lack of a native Rust KRC-20
SDK in May 2026. We accept the engineering effort to build native
KRC-20 commit/reveal ourselves (covered in Phase 5); the KRC-20
protocol envelope is small and well-specified.

### Consequences

- Positive: deterministic builds with `Cargo.lock`; signed releases
  via cosign keyless; strong type safety on money math via
  `katpool-domain::Sompi`; single-binary deploy; native gRPC to
  embedded kaspad.
- Negative: we own the native KRC-20 implementation (≈ 1 week of
  focused Phase 5 work). Mitigation: byte-equivalence tests vs.
  kasplex/go-krc20d for a fixed set of test vectors.
- Negative: smaller pool of contributors familiar with Kaspa-specific
  Rust crates compared to Bun/TS. Mitigation: thorough onboarding
  doc, ADR + runbook discipline.

### Confirmation

- `cargo deny` passes
- `cargo audit` passes (or equivalent via `cargo deny check
  advisories`)
- Release pipeline produces a static musl binary signed with cosign
- All money math goes through `katpool-domain::Sompi`; clippy lint
  `clippy::float_arithmetic = "deny"` prevents float drift

## Pros and Cons of the Options

### Option 1: Rust-first

- Good: native rusty-kaspa integration, single binary, deterministic,
  memory-safe, no WASM, no GC
- Good: workspace + cargo-deny gives a clean supply-chain story
- Bad: native KRC-20 implementation required
- Bad: Rust learning curve for future contributors unfamiliar

### Option 2: Hybrid (Rust core + TS KRC-20)

- Good: reuse working KRC-20 code from legacy pool
- Bad: keeps Bun+WASM in the runtime; reintroduces the failure modes
  this rebuild aims to eliminate
- Bad: cross-language IPC needed (and that's the failure-mode class
  we hit with Redis-stale-IP)

### Option 3: Go

- Good: also single-binary; mature ecosystem
- Bad: no native rusty-kaspa integration; we'd reimplement consensus
  primitives or use WASM. Defeats the purpose.

### Option 4: Continue Bun/TS

- Good: zero migration cost
- Bad: doesn't address any of the legacy failure modes
- Rejected.

## More Information

- Native Rust KRC-20 feasibility evidenced by the kasplex envelope spec
  and rusty-kaspa's `kaspa-txscript` already shipping `ScriptBuilder`,
  `Opcodes`, and the `addresses` / `consensus-core` primitives we need.
- Companion ADRs: [0002 (fork bridge)](0002-fork-rusty-kaspa-bridge.md),
  [0006 (postgres pinning)](0006-postgres-17-pinned.md).
