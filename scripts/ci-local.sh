#!/usr/bin/env bash
# Full local CI parity before opening a PR (~15–25 min with cache).
set -euo pipefail

cd "$(dirname "$0")/.."

./scripts/ci-fast.sh

echo "==> cargo test --workspace --locked --no-fail-fast"
cargo test --workspace --locked --no-fail-fast

if command -v cargo-deny >/dev/null 2>&1; then
  echo "==> cargo deny check"
  cargo deny check --all-features
else
  echo "==> cargo deny (skipped — install with: cargo install cargo-deny)"
fi

echo "ci-local: OK"
