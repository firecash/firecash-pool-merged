# Branch Protection — required settings on `main`

This document is the source-of-truth for the GitHub branch protection
rules that **must** be applied to the `main` branch of
`Nacho-the-Kat/katpool`. GitHub does not version-control these
settings, so we keep them written down here and reconcile manually (or
via `gh api repos/Nacho-the-Kat/katpool/branches/main/protection`)
after any change.

If you discover that production settings drift from this document,
open a PR to either update the settings or update the document — but
not both silently. The discrepancy itself is a finding.

## Required settings on `main`

### Restrict pushes

- [x] **Require a pull request before merging**
- [x] Require approvals: **1** (operator account `@argonmining`)
- [x] Dismiss stale pull-request approvals when new commits are pushed
- [x] Require review from Code Owners (enforced via [`/CODEOWNERS`](../CODEOWNERS))
- [x] Restrict who can dismiss pull request reviews: maintainers only
- [x] Allow specified actors to bypass required pull requests: **none**
- [x] Require approval of the most recent reviewable push

### Required status checks

All of the following must pass before merge. Names match the workflow
job names in `.github/workflows/`:

- [x] `ci / fmt`
- [x] `ci / clippy`
- [x] `ci / test`
- [x] `ci / deny`
- [x] `ci / doc`
- [x] `ci / coverage`

Settings:

- [x] Require branches to be up to date before merging
- [x] Require status checks to pass before merging
- [x] Require conversation resolution before merging

### Commit hygiene

- [x] **Require signed commits** (GPG or SSH-signed)
- [x] **Require linear history** (no merge commits)
- [x] Require deployments to succeed before merging: **off** (deploys
      happen post-merge, gated by their own workflow)

### Push & deletion restrictions

- [x] Lock branch: **off**
- [x] Restrict force pushes: **on** (no force push, ever)
- [x] Restrict deletions: **on** (no branch deletion, ever)

### Bypass

- [x] Do not allow bypassing the above settings
- [x] Rules apply to repository administrators

## Required repository settings (separate from branch protection)

These live under `Settings → General` and `Settings → Actions`:

- [x] **Default branch**: `main`
- [x] **Merge button**:
  - Allow squash merging: **on** (preferred)
  - Allow merge commits: **off**
  - Allow rebase merging: **off**
  - Default merge message: "Pull request title and description"
- [x] **Automatically delete head branches**: **on**
- [x] **Always suggest updating pull request branches**: **on**
- [x] **Allow auto-merge**: **on**

### Actions permissions

- [x] **Allow GitHub Actions**: enabled
- [x] **Allow actions from**: selected actions and reusable workflows
  - `actions/*` (first-party)
  - `dtolnay/rust-toolchain@*` (pinned by SHA in our workflows)
  - `Swatinem/rust-cache@*` (pinned by SHA in our workflows)
  - `sigstore/cosign-installer@*` (pinned by SHA)
  - `anchore/syft@*` (pinned by SHA)
  - `aquasecurity/trivy-action@*` (pinned by SHA)
  - `rustsec/audit-check@*` (pinned by SHA)
- [x] **Workflow permissions**: read-only by default; jobs opt into writes
      via job-level `permissions:` blocks
- [x] **Allow GitHub Actions to create and approve pull requests**: **off**
- [x] **Fork pull request workflows**: require approval for first-time
      contributors

### Secrets management

Secrets are scoped to environments where possible:

- `production` environment — gated by required reviewer
  (`@argonmining`) on workflow runs that deploy to NetCup or Railway:
  - `RAILWAY_TOKEN`
  - `B2_APP_KEY_ID`, `B2_APP_KEY`
  - `COSIGN_PRIVATE_KEY` (only if not using keyless OIDC; preferred is
    keyless)
- `release` environment — gated for publishing signed artifacts:
  - GitHub OIDC is used for keyless cosign signing; no long-lived
    keys required.

## Verification

After updating branch protection, verify with:

```bash
gh api repos/Nacho-the-Kat/katpool/branches/main/protection \
  --jq '{
    required_pull_request_reviews,
    required_status_checks: .required_status_checks.checks,
    enforce_admins: .enforce_admins.enabled,
    required_signatures: .required_signatures.enabled,
    required_linear_history: .required_linear_history.enabled,
    allow_force_pushes: .allow_force_pushes.enabled,
    allow_deletions: .allow_deletions.enabled
  }'
```

The expected output matches the checked boxes above. If anything
diverges, the discrepancy is a finding to be triaged.
