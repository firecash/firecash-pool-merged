# Issue & PR label taxonomy

Authoritative source of truth for the label taxonomy used on
[Nacho-the-Kat/katpool](https://github.com/Nacho-the-Kat/katpool)
issues and pull requests. Mirrors what's actually configured in the
repo's label settings. When the taxonomy changes, update this file
**and** the repo labels in the same PR.

Bulk-recreate from this file:

```bash
# requires `gh` authenticated against the repo
for row in \
    'phase-0|Phase 0 — bootstrap & governance|1d4ed8' \
    'phase-1|Phase 1 — vendored bridge & event bus|2563eb' \
    'phase-2|Phase 2 — db schema & migrations|3b82f6' \
    'phase-3|Phase 3 — pool-accountant (PROP + NACHO accrual)|60a5fa' \
    'phase-4|Phase 4 — KAS payout engine (KIP-9/13)|93c5fd' \
    'phase-5|Phase 5 — KRC-20 / NACHO payout cycle|bfdbfe' \
    'phase-6|Phase 6 — public API|dbeafe' \
    'phase-7|Phase 7 — production infra & cutover|172554' \
    'bridge|Touches the vendored stratum bridge|fef3c7' \
    'accountant|Touches pool-accountant / PROP allocation|fde68a' \
    'payout-kas|Touches KAS payout engine|fcd34d' \
    'payout-krc20|Touches NACHO/KRC-20 payout engine|f59e0b' \
    'consensus|kaspa consensus / PoW / Toccata-touching|d97706' \
    'db|schema / migrations / sqlx|059669' \
    'infra|systemd / CI / deployment / ops|10b981' \
    'security|Security-relevant change or report|991b1b' \
    'cleanup|Tech debt / refactor / hygiene|94a3b8' \
    'breaking|Requires migration (config / schema / API)|b91c1c' \
    'blocked|Blocked on external dependency|78716c' \
; do
    IFS='|' read -r name desc color <<< "${row}"
    gh label create "${name}" --description "${desc}" --color "${color}" --force
done
```

## Phase labels (cross-cutting timeline tracking)

| Label | Meaning |
|---|---|
| `phase-0` | Bootstrap & governance (workspace, CI, ADRs, runbooks) |
| `phase-1` | Vendored stratum bridge, event bus, anti-abuse, fuzz, kaspad-tn10 |
| `phase-2` | PostgreSQL schema, sqlx migrations, legacy import reconciler |
| `phase-3` | pool-accountant with PROP allocation + NACHO accrual |
| `phase-4` | KAS payout engine + KIP-9/KIP-13 mass batcher |
| `phase-5` | Native Rust KRC-20 / NACHO rebate cycle |
| `phase-6` | Public read-only HTTP API |
| `phase-7` | Production infra, shadow run, cutover, rollback |

Apply *exactly one* phase label per issue/PR (the phase that owns the
work). Cross-phase work belongs to whichever phase has the dependency
edge from the plan.

## Functional-area labels (subsystem ownership)

| Label | Touches |
|---|---|
| `bridge` | `bridge/` — the vendored stratum bridge crate |
| `accountant` | `accountant/` — PROP allocation, share accounting |
| `payout-kas` | `payout-kas/` — KAS payout transactions |
| `payout-krc20` | `payout-krc20/` — KRC-20 commit/reveal, NACHO rebates |
| `consensus` | Anything that interacts with `kaspa_pow`, header math, fork rules |
| `db` | `crates/katpool-db`, sqlx migrations, schema design |
| `infra` | `ops/`, systemd units, CI workflows, deployment scripts |
| `security` | Security report or fix; defaults to private disclosure first |

Multiple functional-area labels are allowed when an issue/PR touches
more than one subsystem.

## Kind labels (work-type)

| Label | Meaning |
|---|---|
| `bug` | A defect — something is wrong with shipped behaviour |
| `enhancement` | A new feature or substantive improvement |
| `cleanup` | Tech debt, refactor, dead code removal, lint hygiene |
| `documentation` | Docs-only changes (no code, no config schema) |
| `breaking` | Requires operator action: config/schema/API migration |

`bug`, `enhancement`, and `cleanup` are mutually exclusive — pick the
dominant kind. `breaking` is additive (`enhancement` + `breaking` is
common).

## Meta labels

| Label | Meaning |
|---|---|
| `blocked` | Cannot proceed until external dependency resolves |
| `duplicate`, `invalid`, `wontfix` | Self-explanatory triage outcomes |
| `good first issue`, `help wanted`, `question` | GitHub defaults; usable but rarely needed in this repo today |

## When to add a new label

Open a PR that updates **both** this file and the repo's labels (via
the bulk-recreate script above). The PR should explain why the
existing taxonomy isn't sufficient — taxonomy bloat is its own
problem. Removing a label requires a separate PR that also re-labels
any issues that carry the removed label.
