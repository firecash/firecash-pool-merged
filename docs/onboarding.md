# Onboarding

Getting a working development environment for katpool. Aimed at a new
contributor with comfortable Linux/Rust experience and no prior
context on this codebase.

If anything in this document is wrong or out of date, that is itself
the bug — please open a PR.

## 1. Prerequisites

| Tool | Version | Why |
|---|---|---|
| `rustup` | latest | Toolchain manager. The repo pins Rust 1.88 via [`rust-toolchain.toml`](../rust-toolchain.toml). |
| `git` | 2.30+ | Standard git ops; SSH commit signing requires Git 2.34+. |
| `protoc` (protobuf-compiler) | 3.21+ | The bridge's transitive deps include `kaspa-grpc-core` which compiles `.proto` files via `prost-build` at build time. Without `protoc`, `cargo check -p kaspa-stratum-bridge` fails. Install via `apt-get install protobuf-compiler` (Debian) or `brew install protobuf` (macOS). |
| `clang` + `libclang-dev` | 14+ | The bridge transitively depends on `librocksdb-sys`, which builds the RocksDB C++ library and binds via clang. Install via `apt-get install build-essential clang libclang-dev libstdc++-12-dev`. |
| `docker` | 20.10+ | Ephemeral PostgreSQL for integration tests via testcontainers; local kaspad testnet-10. |
| `psql` (client) | 16+ | Manual DB inspection during development. |
| `cargo-deny` | 0.18+ | Supply-chain gates locally. `cargo install --locked cargo-deny --version '^0.18'` |
| `cargo-tarpaulin` | latest | Coverage. `cargo install --locked cargo-tarpaulin` |
| `cargo-fuzz` | latest | Fuzz harnesses (used in `katpool-storagemass`, `bridge`, KRC-20). `cargo install --locked cargo-fuzz` |
| `cargo-nextest` | latest (optional) | Faster test runner. `cargo install --locked cargo-nextest` |
| `gh` | 2.40+ | GitHub CLI for PRs and branch protection inspection. |
| `sops` | 3.9+ | Decrypts the dev-environment secrets. Install per <https://github.com/getsops/sops>. |
| `age` | 1.2+ | Sops backend. Install per <https://github.com/FiloSottile/age>. |

You do **not** need:

- Bun, Node.js, or any JavaScript runtime
- The WASM kaspa SDK
- Puppeteer / Chrome
- A production treasury key (development uses a generated dev key
  with zero on-chain funds)

## 2. Clone and verify

```bash
git clone https://github.com/Nacho-the-Kat/katpool.git
cd katpool

# The pinned toolchain installs automatically on first cargo invocation;
# you can also pre-install:
rustup show

# Verify the gates that CI runs:
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check
```

If all four pass, you have a working environment.

To run the Phase 0 stub binary:

```bash
cargo run --release --bin katpool
```

It prints linked-crate versions and exits — that's expected for
Phase 0. Real wiring lands across Phases 1–6.

## 3. Pre-commit hooks

We use `cargo-husky` to wire the same gates as Git hooks. They run
automatically on first `cargo build`. To verify:

```bash
ls .git/hooks/pre-commit
```

The hook script runs `cargo fmt --all --check` and `cargo clippy
--workspace --all-targets -- -D warnings` before allowing a commit.
Bypass only via `--no-verify` and only when you can immediately
explain why.

## 4. Working with the bridge fork

The `bridge/` directory is a vendored fork of `rusty-kaspa` v1.1.0's
`bridge/` subdirectory, brought in via `git subtree` so the upstream
history is preserved.

To inspect upstream history:

```bash
git log --grep="bridge" bridge/
```

To pull a future upstream update (rare — we intentionally keep this
divergence tight):

```bash
git subtree pull \
  --prefix=bridge \
  https://github.com/kaspanet/rusty-kaspa.git \
  master --squash
```

Document the pull in [`bridge/UPSTREAM.md`](../bridge/UPSTREAM.md).

## 5. Local PostgreSQL

Integration tests spin up a fresh ephemeral Postgres 17 via
`testcontainers`. For interactive development:

```bash
docker run --rm -d \
  --name katpool-dev-db \
  -e POSTGRES_PASSWORD=dev \
  -e POSTGRES_DB=katpool \
  -p 5432:5432 \
  postgres:17

# Migrations applied via sqlx-cli (see crates/katpool-db README once
# Phase 2 lands).
```

## 6. Local kaspad (testnet-10)

Phase 1+ work uses testnet-10. Easiest way to run one:

```bash
docker run --rm -d \
  --name kaspad-tn10 \
  -p 16210:16210 \
  -p 17210:17210 \
  supertypo/rusty-kaspad:v1.1.0 \
  kaspad --testnet --netsuffix=10 --utxoindex \
    --rpclisten=0.0.0.0:16210 --rpclisten-borsh=0.0.0.0:17210
```

A faucet for testnet-10 is at <https://faucet-tn10.kaspanet.io/>.

## 7. Secrets for development

Production secrets (treasury key, B2 credentials, Railway token, etc.)
live in `ops/secrets/secrets.sops.yaml` encrypted with age. **Never
decrypt to disk.**

For development, generate your own age key:

```bash
age-keygen -o ~/.age/katpool-dev.key
# Print the public recipient and add it as a recipient in ops/secrets/.sops.yaml
age-keygen -y ~/.age/katpool-dev.key
```

The production age recipient is held off-machine by the operator.
You will not be able to decrypt production secrets without explicit
recipient addition by the operator.

## 8. Project structure orientation

```text
crates/      Shared library crates (used by everything else)
bridge/      Forked stratum bridge (Phase 1)
accountant/  Event subscriber, PROP allocation (Phase 3)
payout-kas/  Daily KAS payouts (Phase 4)
payout-krc20/ NACHO rebate engine (Phase 5)
api/         Read-only HTTP API (Phase 6)
katpool/     Main wiring binary
ops/         Production deployment configs (Phase 7+)
migration/   DB migrations + legacy import (Phase 2)
docs/        This directory: architecture, threat model, ADRs, runbooks
tests/       Integration / replay / fuzz / chaos / load harnesses
```

Each crate has a focused `lib.rs` doc comment describing its role and
its phase. Read that first when entering a new crate.

## 9. Where to ask questions

- Codebase questions: file a GitHub Discussion or a draft PR with a
  question label. We answer in the open so the answer benefits the
  next contributor.
- Operations questions: see [runbooks](runbooks/).
- Architectural questions: check [decisions](decisions/) first; if
  no ADR covers it, open a "ADR-XXXX: <topic>" draft PR and we'll
  discuss there.
- Security findings: see [`SECURITY.md`](../SECURITY.md). **Do not**
  open a public issue.
