#!/usr/bin/env bash
# Fast CI parity (~2–5 min locally with cache). Run before every push.
# Mirrors the gating jobs in .github/workflows/ci.yml except test/coverage.
set -euo pipefail

cd "$(dirname "$0")/.."

export CARGO_TERM_COLOR=always
export CARGO_INCREMENTAL=0
export RUSTFLAGS="-D warnings"
export RUST_BACKTRACE=short

echo "==> cargo fmt --all --check"
cargo fmt --all --check

echo "==> cargo clippy --workspace --all-targets --locked"
cargo clippy --workspace --all-targets --locked -- -D warnings

echo "==> cargo doc --workspace --no-deps --locked"
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked

if command -v cargo-machete >/dev/null 2>&1; then
  echo "==> cargo machete"
  cargo machete --skip-target-dir
else
  echo "==> cargo machete (skipped — install with: cargo install cargo-machete)"
fi

if command -v typos >/dev/null 2>&1; then
  echo "==> typos"
  typos
else
  echo "==> typos (skipped — install from https://github.com/crate-ci/typos)"
fi

echo "ci-fast: OK"
