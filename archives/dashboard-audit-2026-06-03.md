> **Archived.** Findings were merged into the dashboard; see
> [`archives/README.md`](README.md).

# Dashboard end-to-end audit — 2026-06-03

A fine-toothed audit of the deployed dashboard
(`https://tn10-dashboard-stage-testnet-10.up.railway.app`) covering visual
design (spacing, alignment, centering, typography, responsive) **and** data
correctness. Every rendered figure was cross-checked against the live tn10
API (`127.0.0.1:18080`), the `katpool_tn10` database, the kaspad node, and the
upstream network/price sources.

Method: full-page screenshots of all seven routes at desktop (1440×900) and
mobile (390×844) via headless Chromium; per-endpoint ground-truth snapshots;
direct DB queries; and source review of every panel/formatter implicated.

## Summary

The dashboard is structurally sound — centered shell (`max-w-[1600px]`),
consistent panel system, faithful per-miner numbers (balances, payouts,
workers, shares all matched the API within live-growth tolerance). The audit
found **six dashboard defects** (now fixed) and **a set of backend/config
follow-ups** that affect data *meaning* rather than rendering.

## Fixed in this pass (dashboard)

| # | Severity | Finding | Fix |
|---|----------|---------|-----|
| F1 | High | **Delta chips always printed a `+` sign.** `DeltaChip` rendered `formatPercent(Math.abs(value))`, and `formatPercent` prepends `+` for any positive input — so a *downtrend* showed a red down-arrow next to text reading `+25.9%`. | Render the signed value (`formatPercent(value)`) so the text sign matches the arrow + colour. (`components/dashboard/delta-chip.tsx`) |
| F2 | High | **Geo + firmware panels plotted `workers`, which is structurally 0.** Closed sessions are persisted with `worker_id = NULL` (verified: 3/3 rows null in `connection_session`), so `count(DISTINCT worker_id)` is always 0. "Global distribution" rendered "United States — 0 workers" with a zero-width bar; "Miner software" centred on "0 workers". | Display `sessions` (the populated aggregate) and relabel accordingly. (`features/geo/geo-panel.tsx`, `features/firmware/firmware-panel.tsx`) |
| F3 | Medium | **"Blocks matured" headline stat was always 0** (`matured_at` is never set in current data), making the pool look idle on the hero and Status page. | Show **"Blocks found"** (6.5K) — a meaningful, non-zero figure. (`features/overview/hero.tsx`, `features/status/status-board.tsx`) |
| F4 | High | **Cumulative payouts overstated distribution ~5×.** The chart summed *all* cycles — including `planned`, `broadcasting`, and a `failed` 13,030 KAS cycle — and labelled it "Value distributed to miners" (~400K KAS), while confirmed paid is ~80K. | Count **settled cycles only**; the curve now peaks at ~80K, matching "KAS paid (confirmed)". (`features/payouts/payout-flow.tsx`) |
| F5 | Medium | **Hero printed an alarming ">100% of the total Kaspa network hashrate."** A pool cannot exceed the network it mines; the ratio is implausible when the network context and pool estimate are momentarily out of step (see B2/B3). | Suppress the share line when the ratio is implausible (`> 100%`), falling back to the descriptive copy. (`features/overview/hero.tsx`) |
| F6 | Low/Perf | **`resolveRange` did not round to the bucket** despite its doc claiming it did, so the hero sparkline and the hashrate panel computed timestamps milliseconds apart → two distinct cache keys → the *same* 24h series fetched twice with no cache sharing. | Floor `to`/`from` to the bucket boundary; hero + panel now dedupe to one fetch and keys are stable across refetches. (`lib/range.ts`) |

All changes pass `tsc --noEmit` and `next lint`, and were visually verified
against a local production build pointed at the live tn10 API.

## Follow-ups (backend / configuration — not dashboard rendering)

- **B2 (resolved 2026-06-05 — not a pool bug; a cross-convention comparison
  artifact).** Investigated against the live tn10 DB and node. The pool's
  share-difficulty → hashrate conversion is the standard kaspa-stratum
  convention and is correct: a share of difficulty `D` represents `D × 2^32`
  expected hashes, which is exactly what the difficulty-1 target
  `2^224 - 1 / D` that the bridge sends to (and the ASIC honours) implies.
  Measured: Σ(difficulty)=394,442 over 300 s ⇒ ≈5.6 TH/s, the single
  Goldshell's real stratum-side rate. The apparent "≫ network" was a
  units mismatch with the *baseline*: the Kaspa API's network hashrate is
  derived from the node's block difficulty (1,652,213 ⇒ `difficulty × 2 /
  block_time(0.1 s)` = 33.0 MH/s, matching the API's `3.304e-05`), a different
  scale than stratum share difficulty. On a low-difficulty testnet the two
  scales make a single ASIC look larger than the whole network; at mainnet
  difficulty (~EH/s) the ratio is sane. Locked in by regression tests
  (`bridge/src/hasher.rs` diff→target; `katpool-db` `hashrate_hs` convention +
  the live 5.6 TH/s sample). Defence-in-depth on the render side already
  suppresses an implausible (>100%) ratio (F5). No pool code change required.
- **B3 (Medium, config): Network context must match the pool's network per
  environment.** The Railway (tn10) deployment correctly uses
  `api-tn10.kaspa.org`; local dev uses mainnet `api.kaspa.org`. The `×1e12`
  conversion in `app/api/network/route.ts` is correct (the Kaspa API returns
  TH/s). Document/standardize the env per network so the comparison is always
  apples-to-apples.
- **B1 (Medium, data richness): `connection_session.worker_id` recording.**
  *Resolved by PR #71* — sessions now `open` a live row at `mining.authorize`
  with `worker_id` bound up-front (correlated to the close by connection id),
  so `count(DISTINCT worker_id)` over open rows is populated for authorized
  miners. The earlier "always NULL" symptom (and F2) reflected the pre-#71
  close-only `record_closed` path. The one remaining null case is a *bare*
  authorize (address with no `.worker` suffix): such a connection carries no
  worker identity anywhere — its shares are also unattributed (the bridge gates
  `ShareCredited` on a non-empty `WorkerName`) — so the null is correct, not a
  gap. A worker-name fallback would invent phantom workers inconsistent with
  the share path, so it is deliberately *not* added. Locked in by accountant
  session-handler tests + the `connection_session` repo tests.
- **B5 (Medium, reliability): Overview pool-hashrate chart intermittently
  (~1/3) stays on its skeleton** with no client error; the endpoint is fast and
  healthy (~0.4 s, 289 points). Correlates with occasional cold BFF/edge
  latency. Partially mitigated by F6 (one fewer concurrent fetch) and
  `keepPreviousData`. A deeper fix would SSR/prefetch the first chart.
- **B4 (Low): USD valuations apply the CoinGecko mainnet KAS price to testnet
  earnings** (e.g. "$4,322.84" on testnet KAS). Resolves at mainnet cutover;
  consider gating USD by network until then.
- **B6 (Low): Recorded sessions have `connected_at == disconnected_at`**
  (0-duration ephemeral reconnects).

## Design / polish notes

- **D1:** The Leaderboard route shows a single-row table on a near-empty page
  (testnet reality). Consider capping width / centering or adding supplemental
  context when miner count is low.
- **D2:** "Treasury snapshot" reads "—" (treasury is null) on Overview/Status.

## Verified accurate (no change needed)

Per-miner profile, balances (allocated/paid/payable), NACHO rebate
(accrued/paid/pending), worker list, reject rate, payout history (incl.
failed/planned/settled states), block list (hashes, DAA scores, statuses,
"found N ago"), block-status donut, and the halving/emission module all matched
ground truth. Global layout is centered and spacing is consistent across
breakpoints; mobile stacks correctly.
