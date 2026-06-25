---
status: accepted
date: 2026-06-02
deciders: argonmining
consulted: argonmining
informed: argonmining
---

# ADR-0024: Dashboard design overhaul — neo-terminal × Apple refinement

## Context and Problem Statement

[ADR-0023](0023-new-dashboard-architecture-and-stack.md) shipped the new
dashboard (`katpool-dashboard-new/`, Next 15 + React 19 + Tailwind v4 +
TanStack Query + ECharts) and it is now live, pointed at the TN10
instance:
<https://tn10-dashboard-stage-testnet-10.up.railway.app/>.

The stack, data map, and BFF are sound. The **visual result is not**.
A first-principles design audit — code review of the design system plus
12 headless full-page captures (all 6 pages × dark/light ×
desktop 1440 / mobile 390, in `.design-audit/`) — found a *clean,
competent admin template*, not the "puts every other pool to shame,
gorgeous, bleeding-edge" product the brief demands. Worse, sparse TN10
data (one active miner) and intermittent BFF failures make the live site
read as **broken and barren** on first impression.

This ADR records the audit, fixes the design direction, and lays out a
phased plan to close the gap. It does **not** change the stack
(ADR-0023) or the API (ADR-0021).

## Decision drivers

- "Anything short of a fully-impressed end user on the design is a
  failure" — the explicit, standing bar.
- The home page's most important visuals currently render error boxes.
- Mining-pool audiences are data-hungry and discerning; density and
  correctness matter as much as polish.
- Real data is thin on TN10 today and will be thin for any *new* miner
  on mainnet — the design must look intentional and premium when empty.

## Audit findings (evidence-grounded)

Severity P0 = looks broken; P1 = template-vs-bleeding-edge gap;
P2 = missing signature value; P3 = polish.

### P0 — currently reads as broken
1. **Hero panels error.** Overview "Pool hashrate" and "Active miners"
   charts, and Miner-Lookup "Top miners", intermittently show
   *"Couldn't load this data / Retry"*. The marquee home-page visual is
   a red warning box. Root-cause the BFF→katpool history/leaderboard
   path (timeout/caching/retry) **and** redesign the error state.
2. **`Network share 3,066,155.40%`.** `poolHs / networkHs` is unguarded;
   on TN10 the pool dwarfs the tiny network figure. Never render an
   implausible ratio — clamp/guard and present sanely.
3. **`Network hashrate 49.96 MH/s`.** The TH/s→H/s normalization in
   `/api/network` is almost certainly wrong for TN10's `/info/hashrate`
   shape; verify against the live contract (not an assumption).
4. **`REWARD` column entirely `—`** on Blocks — a dead column reads as
   unfinished. Populate it or remove it.

### P1 — competent template → bleeding-edge (design system)
5. **No hierarchy / no hero.** Six identical KPI tiles; every panel the
   same `rounded-2xl border shadow p-5`; the largest number is only
   `text-3xl`. Hashrate — the heartbeat of a pool — deserves a
   display-scale, live, animated hero with network-share context.
6. **Brand is an emoji.** `⛏` in a gradient square + `katpool.` is
   amateur. Needs a bespoke, ultra-premium SVG mark + wordmark.
7. **Generic ECharts.** Donuts are left-offset (`center:["32%"]`) with
   cramped, clipped legends; single-segment donuts look broken; the line
   chart lacks premium treatment (no line glow, last-point pulse,
   current-value annotation, refined crosshair/grid).
8. **Unbalanced grids / dead space.** Tall tables sit beside short
   companion cards (donut, treasury) leaving large voids; the one-row
   leaderboard floats on an empty page. Layouts don't reflow gracefully
   with thin data.
9. **Undifferentiated motion.** Every card shares one mount fade
   (`opacity/y`), so all animate at once and below-the-fold content
   animates before it's seen. No choreography, stagger, or
   `whileInView`.
10. **Flat depth.** Aurora/glow is barely perceptible, borders are very
    low-contrast, surfaces are uniform slate — no layering or signature
    surface treatment.

### P2 — signature value (what makes it *shame* other pools)
- Live **block-found feed** with a tasteful celebration when the pool
  solves a block.
- **Hero hashrate** with animated stream/gauge + network-share framing.
- **Worker / firmware / geo distribution** as real visualizations, not
  single-ring donuts.
- **Share accept/reject stream**, **treasury/payout flow over time**,
  and richer per-miner pages.
- **Difficulty / halving** as a designed countdown module, not eight
  tiny boxes.

### P3 — polish
Light-mode contrast pass; status page enrichment (API latency, last
block age, version/uptime); mobile KPI density; favicon/OG art; refined
copy affordances.

## Considered options (aesthetic direction)

1. **Neo-terminal "mission control"** — dense, glowing live data,
   dark-first, mono accents.
2. **Refined aurora-glass** — evolve the current palette with depth and
   hero moments.
3. **Bold editorial** — large display type, generous space,
   marketing-grade.

## Decision

**Direction: neo-terminal data density executed with Apple-grade
refinement** — dense, live, "mission-control" information surfaces, but
restrained, elegant, and mature: precise typography, deliberate
whitespace, quiet color, materials and motion that feel engineered, not
decorated. Dark-first; light fully realized.

Design-language commitments:

- **Type scale.** Introduce a display tier (hero metrics `~text-6xl`+),
  tighten tracking, keep tabular numerals; numbers are the typography.
- **Hero.** Overview leads with a single hashrate hero (live ticker +
  stream + network share), not a flat 6-tile grid; KPIs become a
  supporting, differentiated row.
- **Materials & depth.** Layered surfaces, a real (subtle) signature
  glow/specular treatment, higher-contrast hairlines, consistent radii
  rhythm — Apple-like restraint, not neon overload.
- **Charts.** A bespoke ECharts theme: centered/auto-balanced donuts
  with legible legends and a graceful single-category state; line charts
  with line glow, last-point pulse, current-value annotation, refined
  crosshair and grid.
- **Motion.** Choreographed, `whileInView`, staggered, reduced-motion
  aware; motion conveys liveness (data updating), not entrance flourish.
- **Empty/sparse states (chosen: honest "design-empty").** No demo/seed
  fakery. Every empty state is a *designed* state that looks intentional
  and premium and tells the miner exactly what fills it — so one miner on
  TN10 (or a brand-new mainnet miner) still sees a beautiful product.
  Layouts reflow to avoid voids when companion data is thin.
- **Brand.** Commission a bespoke, ultra-premium mark + wordmark. Process
  here is **explore concepts → user approves/iterates → implement as
  crisp production SVG** (favicon/OG derived).

## Consequences

- **Good:** the live product matches the brief; the home page leads with
  a correct, reliable, striking hero; the design degrades gracefully on
  thin data; a real brand replaces the emoji.
- **Cost:** a focused redesign pass touching the design system, the
  overview composition, the chart theme, and the shell; plus P0 BFF
  reliability/correctness work. No stack/API churn.
- **Risk:** "bleeding-edge" is subjective — mitigated by the
  approve-as-we-go brand process and iterating against live captures.

## Implementation plan (phased, reviewable PRs)

- **P0 — Reliability & correctness.** Fix/redesign hero error states;
  harden the history/leaderboard BFF path; guard network-share; verify
  & fix network-hashrate units; resolve the `REWARD` column.
- **P1 — Design-system uplift.** Brand mark; type/display scale;
  materials/depth; bespoke ECharts theme; hero hashrate + KPI
  restructure; motion choreography; grid/empty-state reflow.
- **P2 — Signature visualizations.** Live block feed + celebration;
  distribution viz; share stream; treasury flow; richer miner pages;
  difficulty/halving module.
- **P3 — Polish.** Light-mode contrast; status enrichment; mobile
  density; favicon/OG; copy.

## Implementation status

- **P0** (#59): query-resilience + network-share guard; network-hashrate
  units verified correct (TN10's tiny network legitimately yields a small
  ratio). Hero error states redesigned in P1.
- **P1** (#61): brand mark/wordmark (#60), token & material system,
  display/metric type scale, bespoke ECharts theme, live hashrate hero,
  motion choreography, grid/empty-state reflow.
- **P2/P3** (#62): live block feed with rate-limited celebration; chromatic
  halving countdown module; Network-panel trim; Blocks `REWARD` column
  hidden until populated; Status operational-metrics grid.
- **P2 (follow-up):** treasury/payout-flow-over-time and richer per-miner
  pages — data-backed by `/api/v1`, tracked separately.
- **Blocked on API (ADR-0021 extension):** pool-wide share accept/reject
  stream and geo distribution have no current data source; deferred until
  endpoints exist.

## More information

- Consumed API: [ADR-0021](0021-public-read-only-http-api.md); prior
  dashboard ADR: [ADR-0023](0023-new-dashboard-architecture-and-stack.md).
- Audit artifacts: `.design-audit/*.png` (12 headless captures) —
  scratch only, not committed.
- Live target during TN10 staging:
  <https://tn10-dashboard-stage-testnet-10.up.railway.app/>.
