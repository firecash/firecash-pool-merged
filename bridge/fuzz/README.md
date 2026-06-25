# bridge/fuzz — stratum parser fuzz harness

cargo-fuzz / libFuzzer harness for
`kaspa_stratum_bridge::jsonrpc_event::unmarshal_event`. Phase 1
acceptance: 1M+ iterations with zero panics.

## Why a standalone non-workspace crate

cargo-fuzz pulls `libfuzzer-sys`, which is nightly-only. Keeping this
crate outside the main workspace lets the rest of the repo build on
stable Rust 1.88 while still allowing operators to run the fuzzer on
nightly.

The crate is explicitly excluded from the parent workspace via
`/Cargo.toml` `workspace.exclude = ["bridge/fuzz"]` and re-declares an
empty `[workspace]` block of its own.

`bridge/fuzz/Cargo.lock` is checked in and intentionally tracks the
parent workspace's lockfile — without that adoption, the resolver
picks `wasm-bindgen` versions that are incompatible with
`workflow-core` 0.18.0. Whenever the parent lockfile is regenerated,
copy it forward:

```bash
cp /Cargo.lock /bridge/fuzz/Cargo.lock
```

## Prerequisites

```bash
rustup toolchain install nightly --profile minimal --component rust-src
cargo install cargo-fuzz --locked
sudo apt-get install -y build-essential clang libclang-dev libstdc++-12-dev protobuf-compiler
```

## Build

```bash
cd bridge/fuzz
cargo +nightly fuzz build stratum_parser
```

First build is slow (rocksdb compile, ~7 min cold). Incremental
rebuilds are seconds.

## Run

Smoke run (300k iterations, ~5 s):

```bash
cargo +nightly fuzz run stratum_parser -- -runs=300000 -max_total_time=10
```

Phase 1 acceptance run (1M+ iterations):

```bash
cargo +nightly fuzz run stratum_parser -- -runs=1500000 -max_total_time=60
```

Long unattended run (CI / scheduled job):

```bash
cargo +nightly fuzz run stratum_parser -- -max_total_time=3600
```

## Acceptance evidence

| Date | Branch | Iterations | Wall time | Panics |
|---|---|---|---|---|
| 2026-05-25 | phase-1-anti-abuse | 1,500,000 | 23 s | 0 |

Update this table on every PR that touches `bridge/src/jsonrpc_event.rs`
or its transitive serde-deserialiser surface. A panicking input file
auto-saved to `bridge/fuzz/artifacts/` is grounds for a blocking PR
revert + a follow-up regression test case checked into
`bridge/fuzz/corpus/stratum_parser/`.
