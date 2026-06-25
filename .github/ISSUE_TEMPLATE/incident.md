---
name: Incident report
about: Track a live or recently-resolved production incident
title: "INC-YYYYMMDD-<short-slug>"
labels: ["incident", "needs-triage"]
assignees: []
---

<!--
File this issue while the incident is still in progress, then keep it
updated as the situation evolves. Convert to a postmortem after the
incident is resolved.
-->

## Status

- [ ] Detected
- [ ] Acknowledged
- [ ] Mitigating
- [ ] Mitigated
- [ ] Resolved
- [ ] Postmortem opened

## Severity

- [ ] SEV-1 (production fully down or treasury at active risk)
- [ ] SEV-2 (payouts blocked or miner-visible degradation)
- [ ] SEV-3 (single-component degradation, no user impact yet)
- [ ] SEV-4 (latent issue identified before user impact)

## Detection

- **First detected**: <UTC timestamp>
- **Detected by**: <alert name | runbook | direct observation>
- **Linked alert / runbook**:

## Symptom

<!-- One paragraph. What is the externally-visible behaviour? -->

## Affected components

- [ ] bridge
- [ ] accountant
- [ ] payout-kas
- [ ] payout-krc20
- [ ] api
- [ ] embedded kaspad
- [ ] PostgreSQL
- [ ] Railway edge proxies
- [ ] Observability stack
- [ ] Other:

## Current hypothesis

<!-- What do we currently believe is happening? Update as we learn. -->

## Mitigation in progress / applied

<!--
Steps taken so far, in chronological order. Include exact commands,
PRs deployed, configs changed. This is the source for the postmortem
timeline.
-->

| Time (UTC) | Action | Outcome |
|---|---|---|
|  |  |  |

## Treasury / fund impact

- [ ] No on-chain or balance impact
- [ ] Payouts delayed but reconcilable
- [ ] Payouts incorrect (specify recipients and deltas)
- [ ] Treasury at risk (escalate immediately to operator)

## Reconciliation needed?

<!--
If miner balances or payments diverged from expected, list the queries
and the expected vs. actual values here so the cleanup can be audited.
-->

## Postmortem link

<!-- Filled in once the postmortem is opened. -->
