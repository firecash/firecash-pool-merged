# Documentation index

Canonical documentation for the katpool monorepo. Start here, then drill into
the topic you need.

## Operators & on-call

| Document | Purpose |
|---|---|
| [`runbooks/README.md`](runbooks/README.md) | Incident runbooks (symptom → verify → remediate) |
| [`cutover-plan.md`](cutover-plan.md) | Mainnet cutover procedure and rollback |
| [`cutover-stratum-compatibility.md`](cutover-stratum-compatibility.md) | Stratum port/edge inventory (production reference) |
| [`custody.md`](custody.md) | Treasury key custody (sops/age, OS isolation) |
| [`capacity-plan.md`](capacity-plan.md) | Sizing and capacity baselines |
| [`db-schema.md`](db-schema.md) | Operator-facing database schema reference |

## Engineers

| Document | Purpose |
|---|---|
| [`onboarding.md`](onboarding.md) | New developer setup |
| [`dev-workflow.md`](dev-workflow.md) | Local gates, stacked PRs, labels |
| [`architecture.md`](architecture.md) | Components, data flow, deployment topology |
| [`threat-model.md`](threat-model.md) | STRIDE threat model and mitigations |
| [`kips.md`](kips.md) | KIP-9 / KIP-13 implementation notes |
| [`decisions/README.md`](decisions/README.md) | Architectural Decision Records (MADR) |
| [`roadmap.md`](roadmap.md) | Phase status and remaining hardening work |

## Phase acceptance (historical record)

Closed phases keep their acceptance matrices for auditability. They are not
day-to-day runbooks.

| Phase | Document |
|---|---|
| 1 | [`phase-1-acceptance.md`](phase-1-acceptance.md) |
| 2 | [`phase-2-acceptance.md`](phase-2-acceptance.md) |
| 3 | [`phase-3-acceptance.md`](phase-3-acceptance.md) |
| 4 | [`phase-4-acceptance.md`](phase-4-acceptance.md) |
| 5 | [`phase-5-acceptance.md`](phase-5-acceptance.md) |
| 6 | [`phase-6-acceptance.md`](phase-6-acceptance.md) |

## Frontends

| Path | Document |
|---|---|
| `katpool-dashboard-new/` | [`../katpool-dashboard-new/README.md`](../katpool-dashboard-new/README.md) |
| `katpool-landing-new/` | Railway deploy via `railway.toml`; env in `.env.example` |

## Archives

Superseded handoffs, one-shot audits, and retired artifacts live under
[`../archives/`](../archives/). Do not treat them as current runbooks.

## Repo root

| Path | Purpose |
|---|---|
| [`../README.md`](../README.md) | Project overview and quick start |
| [`../SECURITY.md`](../SECURITY.md) | Vulnerability disclosure |
| [`../CHANGELOG.md`](../CHANGELOG.md) | Release history |
