<!--
Pull-request template. Required fields are marked [REQUIRED]; optional
fields can be deleted from the PR body.

If your change touches treasury custody, payouts, KRC-20 construction,
DB migrations, or CI/CD supply chain, the relevant CODEOWNERS block
will request the operator as a required reviewer automatically.
-->

## Summary [REQUIRED]

<!--
One to three sentences. What does this PR do, and why?
Link the issue / phase / ADR that motivates it.
-->

## Phase / scope [REQUIRED]

- [ ] Phase 0 — bootstrap / governance
- [ ] Phase 1 — bridge fork
- [ ] Phase 2 — database schema / migrations
- [ ] Phase 3 — accountant
- [ ] Phase 4 — KAS payout / treasury custody
- [ ] Phase 5 — KRC-20 payout
- [ ] Phase 6 — API / anti-abuse
- [ ] Phase 7 — observability stack
- [ ] Phase 8 — Railway edge
- [ ] Phase 9 — pre-cutover hardening
- [ ] Phase 10 — cutover
- [ ] Cross-cutting / docs / chore

## Change-class checklist [REQUIRED]

Tick every applicable item. Each unchecked item should have a one-line
explanation of why it doesn't apply.

### Always
- [ ] `./scripts/ci-fast.sh` passes (fmt + clippy `--locked` + doc; same flags as CI)
- [ ] `cargo fmt --all --check` passes
- [ ] `cargo clippy --workspace --all-targets --locked -- -D warnings` passes
- [ ] `cargo test --workspace` passes
- [ ] `cargo deny check` passes
- [ ] `CHANGELOG.md` updated under `[Unreleased]`

### Touches the treasury key, payout logic, or KRC-20 construction
- [ ] No new code path can read or log the private key bytes
- [ ] All new tx-construction code paths flow through
      `katpool-storagemass` with explicit assertions
- [ ] Idempotency-key handling reviewed: a mid-cycle restart can
      neither double-pay nor skip a recipient
- [ ] Property tests added or extended

### Touches a database migration
- [ ] Forward migration tested on a copy of `postgres_data`
- [ ] Down migration tested
- [ ] Legacy import reconciliation tests still pass
- [ ] Backup restore drill remains green

### Touches CI / workflows / third-party actions
- [ ] All third-party actions pinned by full commit SHA
- [ ] Top-level workflow `permissions:` block declared
- [ ] Per-job `timeout-minutes` declared
- [ ] No new secrets without an entry in
      `.github/branch-protection.md`

### Adds a new dependency
- [ ] `cargo deny check licenses` accepts the new licence
- [ ] No new transitive dep on `openssl`, `native-tls`, or `git2`
- [ ] If git source: add to `deny.toml [sources] allow-git` with reason

## Verification [REQUIRED]

<!--
How did you confirm this works? Local test output, screenshots,
shadow-run results, replay-test deltas, fuzz iteration counts —
whatever is appropriate for the change.
-->

## Risk and rollback

<!--
If something goes wrong post-merge, what's the blast radius and how do
we roll back? Specifically: is this safe to revert with `git revert`?
Does it touch on-disk state that a revert wouldn't undo?
-->

## Linked items

- ADR(s):
- Runbook(s):
- Issue(s):

## Reviewer notes

<!--
Anything specific you want the reviewer to look at carefully. Avoid
"please rubber-stamp" framing — the reviewer is here to find problems.
-->
