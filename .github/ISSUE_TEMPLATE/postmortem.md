---
name: Postmortem
about: Blameless retrospective on a resolved incident
title: "PM-YYYYMMDD-<short-slug>"
labels: ["postmortem"]
assignees: []
---

<!--
Blameless. The goal of this document is to make the system more
resilient, not to assign fault. Describe what happened, what we
learned, and what we will change — not who made what call.

Open within 48 hours of incident resolution. The first version can be
incomplete; iterate.
-->

## Incident summary

- **Incident issue**: #
- **Severity at peak**: SEV-?
- **Time to detect**: <minutes>
- **Time to mitigate**: <minutes>
- **Time to resolve**: <minutes>
- **User-visible impact**: <e.g. payouts delayed by 4h; no fund loss>

## TL;DR

<!--
Two to four sentences. What happened, why, and what we changed. A
reader who only reads this section should be able to answer "would
this happen again?"
-->

## Timeline (UTC)

| Time | Event |
|---|---|
|  |  |
|  |  |

## Detection

- How did we find out?
- Did the right alert fire? Did it fire in time?
- If detection was via direct observation or a miner report rather
  than an alert, what alert would have caught this? File an issue.

## Impact

- Miners affected:
- Treasury impact:
- Reconciliation performed:
- Data lost:

## Root cause

<!--
Five-whys, not finger-pointing. End on a system property, not a
person. Example: "The deploy succeeded but the new pod cached the old
DB IP; we have no test for DNS re-resolution after a stateful
component restart."
-->

## What went well

<!--
Honest credit to whatever worked: alerts fired, rollback was clean,
runbook applied. This is how the system tells us what to keep
investing in.
-->

## What went poorly

<!--
Honest list of things that slowed us down: missing runbook, ambiguous
alert message, slow rollback, unclear ownership.
-->

## Lessons learned

<!-- Generalisable principles, not action items. -->

## Action items

<!--
Concrete, owned, time-bounded. Every action item must be a GitHub
issue, not an aspiration in this document.
-->

| Action | Owner | Issue | Due |
|---|---|---|---|
|  |  | # |  |

## Linked items

- Runbook(s) updated:
- ADR(s) needed or updated:
- Code changes:
