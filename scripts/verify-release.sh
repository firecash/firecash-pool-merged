#!/usr/bin/env bash
# Verify a katpool release artifact against its cosign Sigstore bundle.
#
# Releases are signed keyless (OIDC) by the GitHub Actions `release.yml`
# workflow (see .github/workflows/release.yml) — no long-lived keys. The
# signature is bound to that workflow's identity and recorded in the Rekor
# transparency log; this script re-checks both, so a binary built by any
# other workflow, fork, or hand is rejected.
#
# Usage:
#   scripts/verify-release.sh <artifact> [bundle]
#
#   <artifact>  file to verify (e.g. the downloaded `katpool` binary or SBOM)
#   [bundle]    Sigstore bundle; defaults to "<artifact>.sigstore-bundle.json"
#
# The expected signer identity / issuer are overridable for forks via env:
#   KATPOOL_RELEASE_REPO              owner/repo (default Nacho-the-Kat/katpool)
#   KATPOOL_RELEASE_IDENTITY_REGEXP   full identity regexp (default: the repo's
#                                     release.yml on a v* tag ref)
#   KATPOOL_RELEASE_OIDC_ISSUER       OIDC issuer (default GitHub Actions)
#
# Exit status: 0 = verified, non-zero = unverified/error (safe to gate on).

set -euo pipefail

repo="${KATPOOL_RELEASE_REPO:-Nacho-the-Kat/katpool}"
# Keyless GitHub Actions identity for a tag-triggered release build looks like
#   https://github.com/<repo>/.github/workflows/release.yml@refs/tags/v1.2.3
# The release job only publishes assets on a `v*` tag ref, so anchoring to
# `@refs/tags/v` is both correct and the tightest match.
identity_regexp="${KATPOOL_RELEASE_IDENTITY_REGEXP:-^https://github\.com/${repo}/\.github/workflows/release\.yml@refs/tags/v}"
oidc_issuer="${KATPOOL_RELEASE_OIDC_ISSUER:-https://token.actions.githubusercontent.com}"

die() {
    echo "verify-release: $*" >&2
    exit 1
}

case "${1:-}" in
    -h | --help)
        sed -n '2,28p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
        exit 0
        ;;
    "")
        die "missing <artifact> (use --help)"
        ;;
esac

artifact="$1"
bundle="${2:-${artifact}.sigstore-bundle.json}"

command -v cosign >/dev/null 2>&1 ||
    die "cosign not found on PATH; install from https://docs.sigstore.dev/cosign/system_config/installation/"
[[ -f "${artifact}" ]] || die "artifact not found: ${artifact}"
[[ -f "${bundle}" ]] || die "bundle not found: ${bundle} (pass it explicitly, or place it beside the artifact)"

echo "==> verifying $(basename "${artifact}") against $(basename "${bundle}")"
echo "    identity =~ ${identity_regexp}"
echo "    issuer    = ${oidc_issuer}"

# `cosign verify-blob` fetches Sigstore's public-good trust root via TUF and
# checks: the bundle's signature over the artifact, the Fulcio cert's identity
# and OIDC issuer, and the Rekor inclusion proof. Any mismatch is a non-zero
# exit, which propagates out of this script under `set -e`.
cosign verify-blob "${artifact}" \
    --bundle "${bundle}" \
    --certificate-identity-regexp "${identity_regexp}" \
    --certificate-oidc-issuer "${oidc_issuer}"

echo "==> OK: signature verified"
