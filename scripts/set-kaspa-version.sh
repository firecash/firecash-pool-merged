#!/usr/bin/env bash
# Single-command bump of the pinned rusty-kaspa version across the workspace.
#
# The Kaspa node binary and our Rust client crates MUST speak the same
# consensus/wire version (a skew silently corrupts block submission, e.g.
# Toccata's transaction-hashing change -> RuleError::BadMerkleRoot). This
# script rewrites the one functional source of truth — the rusty-kaspa git
# `tag` on every `kaspa-*`/`kaspad` entry in the workspace [dependencies] —
# and reports the remaining coupled pins that must move with it.
#
# Usage:
#   scripts/set-kaspa-version.sh <git-tag> [commit-sha]
#
# Example:
#   scripts/set-kaspa-version.sh tn10-toc3 1015a62359e0d06e0b3b3b7f7d06bc1bd4bf0c1b
#
# After running, regenerate the lockfile (cargo update / build) and update
# the coupled pins printed at the end. See docs/decisions/0017-kaspa-version-pinning.md.

set -euo pipefail

TAG="${1:?usage: set-kaspa-version.sh <git-tag> [commit-sha]}"
SHA="${2:-}"

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MANIFEST="${ROOT}/Cargo.toml"
KASPA_REPO_RE='github\.com/kaspanet/rusty-kaspa'

if [[ ! -f "${MANIFEST}" ]]; then
    echo "error: workspace manifest not found at ${MANIFEST}" >&2
    exit 1
fi

# Rewrite tag = "..." on every rusty-kaspa git dependency line. The address
# uses the '\#...#' delimiter form so the slashes in the repo URL don't clash
# with sed's default '/' address delimiter. Anchoring on the canonical repo URL
# ensures the murar8/serde_nested_with patch entry (a different repo) is never
# touched.
sed -i -E "\\#${KASPA_REPO_RE}# s/tag = \"[^\"]*\"/tag = \"${TAG}\"/" "${MANIFEST}"

# Refresh the primary provenance comment's commit hash when a SHA is supplied.
if [[ -n "${SHA}" ]]; then
    SHORT_SHA="${SHA:0:7}"
    sed -i -E "s/rusty-kaspa [^ ]+ \(commit [0-9a-f]+\)/rusty-kaspa ${TAG} (commit ${SHORT_SHA})/" "${MANIFEST}"
fi

echo "==> rusty-kaspa git tag set to '${TAG}' in ${MANIFEST}:"
grep -nE "${KASPA_REPO_RE}.*tag = " "${MANIFEST}" | sed 's/^/    /'

echo
echo "==> Remaining version strings in ${MANIFEST} (review/update manually if stale):"
grep -nE 'rusty-kaspa v[0-9]' "${MANIFEST}" | sed 's/^/    /' || echo "    (none)"

cat <<EOF

==> Coupled pins to update by hand (NOT edited by this script):
    1. rust-toolchain.toml            channel  -> upstream rust-version for '${TAG}'
    2. Cargo.toml [workspace.package] rust-version -> same value
    3. clippy.toml                    msrv -> same value
    4. .github/workflows/ci.yml       toolchain: "..." (all jobs) -> same value
    5. ops/kaspad/install-kaspad-*.sh TN*_RELEASE_TAG + SHA256 -> node binary for '${TAG}'
    6. deny.toml                      re-run 'cargo deny check'; reconcile advisories/licenses

==> Then regenerate the lockfile and verify:
    cargo update -w
    ./scripts/ci-fast.sh && cargo test --workspace && cargo deny check
EOF
