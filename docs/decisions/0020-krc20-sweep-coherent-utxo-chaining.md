---
status: accepted
date: 2026-06-02
deciders: argonmining
---

# ADR-0020: KRC-20 sweep-coherent UTXO chaining for sibling commits

## Context and Problem Statement

The KRC-20 (NACHO) payout engine settles every actionable transfer in one
`settle_pending` sweep (up to `KATPOOL_KRC20_BATCH_LIMIT`, default 1000). Each
fresh `pending` transfer funds its **commit** transaction by planning against
the treasury UTXO set, which the executor reads from
`KaspadClient::treasury_utxos` → kaspad `get_utxos_by_addresses`.

That RPC returns only the **confirmed** UTXO set. A commit broadcast earlier in
the same sweep is still in the mempool, so it neither removes its spent coin
from the snapshot nor surfaces its change output. Every fresh commit in the
sweep therefore plans against the **same stale snapshot**, and the planner's
greedy, largest-first funder selects the **same** dominant coin for all of
them. Only the first commit is accepted; the rest are rejected by the mempool
as double-spends.

Worse, KRC-20 records each commit's deterministic txid **before** broadcast
(record-before-broadcast, ADR-0019). A rejected sibling is left `commit_submitted`
with a recorded txid whose funding coin is now gone, so its crash-resume rebuild
selects a different coin, the re-derived txid no longer matches, and the
transfer strands permanently on `CommitDrift`.

This was observed live on testnet-10 the moment NACHO went live: with the
treasury consolidated into one dominant coin, two transfers pending in one
sweep raced for it — the first completed its commit/reveal, the second
double-spent and wedged. Any cycle with ≥2 recipients would strand all but one
recipient on mainnet.

## Decision Drivers

* Money/custody: a fix must **never** enable a second distinct spend of the
  same coin; refusing (drift) is always safer than guessing.
* Reproducible record-before-broadcast txids across restarts (ADR-0019) must be
  preserved — the fix cannot weaken crash-resume determinism.
* Reuse the proven pattern already in the codebase rather than invent a new one.
* Minimal, production-grade change; no schema migration; no new dependency.

## Decision

The settle sweep threads a per-sweep, in-memory **UTXO ledger**
(`SweepLedger`) through the fresh-commit path. Before planning each commit it
reconciles the freshly fetched confirmed set with the sweep's in-flight
commits, and after signing each commit it records what that commit consumed and
minted:

- **remove consumed inputs** — outpoints spent by earlier commits this sweep are
  dropped from the working set;
- **re-inject change** — each commit's treasury change output is added back as a
  spendable coin, keyed by the **real signed commit txid** at
  `COMMIT_CHANGE_OUTPUT_INDEX` (output 1).

Sibling commits thus **chain** off one another (the next commit spends the
previous commit's change) instead of colliding, exactly mirroring the KAS
planner's `plan_batches`, which re-injects each batch's change as a follow-on
funding coin. KRC-20 must key the chained coin by the real txid (not a
planning-virtual outpoint) because the KRC-20 signer rejects virtual inputs —
and it can, because each commit is signed immediately, so its id is known before
the next transfer is planned.

The ledger update runs in **dry-run** as well as live, so the Runbook-19
rehearsal validates multi-recipient cycles without false contention.

Crash-resume determinism is preserved: each commit peels only `commit_amount +
fee` (~0.25 KAS) off the dominant coin, so the chained change **stays the
largest coin** and the existing greedy rebuild re-selects it, reproducing the
recorded txid. The residual window — a crash after recording a chained commit
but before its parent confirms — is covered by the existing `CommitDrift` guard,
which refuses to broadcast a divergent spend (safe; surfaces to the operator).

Separately, `settle_pending` now logs a `WARN` per failed transfer (transfer
id, payout id, error). Previously only the aggregate `settle_errors` **count**
was logged, so the cause of a stuck sweep was invisible in the journal.

## Decision Outcome

**Chosen: real-txid change chaining within the sweep (mirror KAS `plan_batches`).**
It eliminates same-sweep contention for any treasury UTXO topology — including a
fully consolidated single coin — in a single sweep, with no schema change and no
new crash-resume failure mode beyond the one the drift guard already covers.

### Consequences

- Positive: a multi-recipient cycle settles all recipients' commits in one
  sweep regardless of treasury fragmentation; no recipient is stranded.
- Positive: the Runbook-19 dry-run now reproduces real sibling-funding, so
  contention regressions are caught before deploy.
- Positive: per-transfer settle failures are now diagnosable from the journal.
- Neutral: the sweep builds a mempool chain of commits (parent broadcast before
  child); this is standard Kaspa mempool behaviour and the same shape KAS
  already relies on.
- Negative: a crash mid-sweep, before the chain confirms, can leave a chained
  commit on `CommitDrift` until an operator resets it. Accepted — funds are
  never at risk (no double-spend) and the window is small on a fast DAG.

### Confirmation

- `payout-krc20` orchestration test `sweep_chains_sibling_commits_off_a_single_coin`
  (testcontainer Postgres + mock kaspad): two transfers swept against one coin
  produce commits with **disjoint** inputs, and the second commit spends the
  first commit's change outpoint.
- Existing crash-before-broadcast re-broadcast and UTXO-drift-refusal tests
  still pass (determinism/safety unchanged).
- Verified live on tn10: two commit/reveal pairs accepted on L1 from one
  dominant treasury coin — commits `eb6ddd03…` / `13abf205…`, reveals
  `8dec3c0e…` / `c847fa15…`.

## More Information

- Builds on ADR-0019 (KRC-20 adaptive fees, record-before-broadcast, frozen
  fees) and ADR-0018 (KAS `plan_batches` change chaining, `FeeRate`).
- Affected code: `payout-krc20::execute` (`SweepLedger`, `settle_pending`,
  `handle_pending`, `fetch_spendable_utxos` / `plan_from_utxos`),
  `payout-krc20::sign` (`COMMIT_CHANGE_OUTPUT_INDEX`).
