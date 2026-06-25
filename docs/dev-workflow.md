# Developer workflow

Operational rules for working in this repo. Reading this once saves
re-discovering each rule the hard way; bookmark and re-read when
something feels wrong.

The substance of how the rebuild is structured lives in
[architecture](architecture.md), [phase-1-acceptance](phase-1-acceptance.md),
the ADRs under [decisions/](decisions/), and the runbooks under
[runbooks/](runbooks/). This file is just the process / hygiene
layer.

## Pre-commit local gate

Run every gate locally before pushing. CI runs the same set; matching
it locally costs ~20s on a warm cache and avoids the round-trip cost
of a failed CI job.

```bash
# 1. Formatting (instant) — `--check`, not write.
#    NEVER chain `cargo fmt --all && cargo fmt --all -- --check` in
#    a pre-commit script: the write step silently fixes things and
#    the check step then passes, hiding the fact that the working
#    tree has uncommitted formatting changes. If --check fails,
#    fix manually (or run `cargo fmt --all` as a separate step,
#    review the diff, then `git add` it) — never let a write-mode
#    run happen during the gate.
cargo fmt --all --check

# 2. Lints with strict warnings (warm: ~10s, cold: ~3min)
cargo clippy --workspace --all-targets -- -D warnings

# 3. Rustdoc with strict warnings
#    Critical: cargo clippy does NOT catch broken intra-doc links.
#    cargo doc with -D warnings DOES. If you skip this step you will
#    eat CI failures on the doc job that were trivially avoidable.
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked

# 4. Tests (warm: ~30s, cold: ~3min; integration tests need Docker)
cargo test --workspace

# 5. Supply-chain (warm: ~3s, cold: ~30s)
cargo deny check

# 6. Typos. CI runs typos v1.46.3 from the GitHub releases binary
#    (its MSRV is 1.91, ahead of our 1.88, so `cargo install` would
#    pin you to an older 1.42.x that misses dictionary updates).
#    Match CI exactly by downloading the same binary:
#
#      wget -q https://github.com/crate-ci/typos/releases/download/v1.46.3/typos-v1.46.3-x86_64-unknown-linux-musl.tar.gz
#      tar xzf typos-v1.46.3-x86_64-unknown-linux-musl.tar.gz --no-same-owner ./typos
#      install ./typos /usr/local/bin/typos
#
#    Whitelist updates live in `_typos.toml`. Add entries ONLY for
#    legitimate domain vocabulary; every false-positive entry chips
#    away at the catcher's signal.
typos

# 7. Unused dependencies.
cargo install --locked cargo-machete --version '^0.9'  # one-time
cargo machete --skip-target-dir
```

A failure in any of (1)–(7) is the same kind of failure CI will see.
If you are stuck on a CI failure that the local gate did not catch,
the local gate has a gap — fix it here in this file at the same time
you fix the CI symptom.

### Caught by `cargo doc -D warnings` but not by `cargo clippy`

- Broken intra-doc links (`` [`SomeType`] `` where `SomeType` is not
  in scope). Resolution is usually to fully-qualify the path:
  `` [`crate::module::SomeType`] `` or `` [`external_crate::Type`] ``.
- Missing items the doc references (`[`function`]` after a rename
  that didn't update the doc).
- Rustdoc tests in doc comments that fail to compile.

The cost of the doc step is small (warm: <1s; cold: ~10s on this
workspace). Just always run it.

## Stacked-PR discipline

When PR A is the base of PR B, and PR A merges:

1. **GitHub does not auto-update PR B's `baseRefName`.** It stays
   pointed at the now-merged base branch.
2. Pressing "Merge" on PR B in this state **merges B into the deleted
   base branch**, not into `main`. The squashed branch in the base
   may still exist (some merge strategies preserve it); this is a
   trap.
3. Before merging PR B, **re-target its base to `main`** explicitly:

   ```bash
   # The `gh pr edit --base main` path chokes on the Projects-classic
   # GraphQL deprecation noise as of `gh` v2.x (see issue #8). Use
   # the REST API directly:
   gh api --method PATCH \
       -H "Accept: application/vnd.github+json" \
       /repos/Nacho-the-Kat/katpool/pulls/<N> \
       -f base=main
   ```

4. If the re-target produces `mergeable: CONFLICTING`, the branch
   probably has the *original* (pre-squash) ancestor commits that
   are now duplicated by their squash-merged twins on `main`. Fix
   by hard-reset + cherry-pick:

   ```bash
   git checkout <branch>
   git reset --hard origin/main
   git cherry-pick <commit-1> <commit-2> ...   # just the new work
   git push --force-with-lease origin <branch>
   ```

   Both PR #10 (Phase 1 close-out land) and PR #7's rebase (Phase 2
   schema land) used this recipe successfully. It's the most reliable
   way to drop the squashed-twin commits without merge-conflict
   theatre.

5. Update labels via REST too if `gh pr edit --add-label` fails for
   the same reason:

   ```bash
   gh api --method POST \
       -H "Accept: application/vnd.github+json" \
       /repos/Nacho-the-Kat/katpool/issues/<N>/labels \
       -f "labels[]=phase-X" -f "labels[]=enhancement"
   ```

## Label taxonomy

See [`.github/labels.md`](../.github/labels.md) for the authoritative
list, the rules (one phase label, multiple area labels, mutually-
exclusive kind labels), and the bulk-recreate script.

## Commit messages

Squash-merge convention. The PR title is the squashed commit's
first line. Keep PR titles informative:

- Bad: `Phase 2 milestone 1`
- Good: `Phase 2 milestone 1: katpool-db schema + sqlx migrations + integration tests`

Body of squash-merge commits should explain "why" rather than "what"
— the diff shows the what. ADRs cover deeper rationale; PR bodies
link to them.

## Adding a new follow-up issue mid-PR

When you notice a small thing that can't be fixed in the current PR
without scope creep, open the issue **immediately** (before the
thought falls out). Use the REST workaround for labels (see above)
and cross-link from the PR body. Examples that have followed this
pattern:

- #6 — sub-1 pool difficulty (`min_share_diff: f64`)
- #8 — `gh pr edit` Projects-classic deprecation workaround
- #9 — `testcontainers` postgres image pinned to match production
