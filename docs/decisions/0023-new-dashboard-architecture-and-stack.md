---
status: accepted
date: 2026-06-02
deciders: argonmining
consulted: argonmining
informed: argonmining
---

# ADR-0023: New miner dashboard — stack, architecture, and data map

## Context and Problem Statement

The legacy dashboard (`katpool-dashboard`, Next.js 14 + Chart.js) maps
to the *old* pool's REST API + VictoriaMetrics + `api.kaspa.org` +
CoinGecko, fanning out to **four** backends from the browser/edge. The
rebuild ships a single, hardened, read-only HTTP API
([ADR-0021](0021-public-read-only-http-api.md)) — 3 health probes plus
**14** `/api/v1` endpoints (pool stats/hashrate/history/blocks/payouts/
**leaderboard**/**miners-history**/**firmware**; per-wallet balance/
profile/workers/hashrate-history/payouts/rejects/full_rebate), all
rate-limited, cached, and money-typed (`KasAmount = {sompi, kas}` dual
strings).

> **Update (2026-06-02):** the three v1 endpoints the legacy dashboard
> needed but v1 lacked were **added in this phase** rather than
> deferred, so the new dashboard ships at full parity-plus:
> `GET /api/v1/pool/leaderboard` (top miners by window hashrate +
> pool-share), `GET /api/v1/pool/miners/history` (active-miner count
> time-series), and `GET /api/v1/pool/firmware` (miner-software
> breakdown). The firmware breakdown is backed by a new, hot-path-safe
> persistence pipeline: a `PoolEvent::SessionClosed` emitted at the
> bridge's disconnect hook (carrying the reported `mining.subscribe`
> user-agent + worker identity + real client IP) is consumed by the
> accountant into `connection_session`. Share crediting is untouched;
> the firmware data accrues from deploy time forward.

We want a **ground-up** dashboard at `/root/katpool/new-katpool-dashboard`
that "puts other pools to shame": latest stack, gorgeous + responsive,
enterprise-grade, and exhaustive — every datum a miner could want, shown
well. This ADR fixes the stack, the data-flow architecture, the
information architecture, and an explicit endpoint-by-endpoint data map,
including where the v1 API is insufficient.

Key tension: the v1 API is **pool-scoped**. It deliberately does **not**
expose network context (network hashrate, difficulty, coin supply,
halving, KAS/NACHO USD price), a **top-miners leaderboard**, or
**miner-firmware** breakdown — all of which the legacy dashboard showed.
The architecture must source those without weakening the v1 API's
read-only/least-privilege posture.

## Decision Drivers

- Latest, well-supported stack; beautiful + fully responsive; a11y-clean.
- Comprehensive: show everything the data tier can support.
- No secrets or write paths in the browser; no direct browser→backend
  fan-out (CORS, caching, key hiding, one origin to harden).
- Numeric correctness for money (never lose precision on `sompi`).
- Low cost, fast cold start, simple deploy/rollback.
- Reuse the v1 API as the single source of pool truth; isolate the few
  non-pool data needs behind one seam.

## Considered Options

**Rendering/data architecture**

1. **SPA hits the v1 API directly** from the browser (enable CORS).
2. **BFF (backend-for-frontend)**: Next.js Route Handlers proxy +
   aggregate the v1 API (primary) and the few external sources (Kaspa
   public API for network context, a price oracle), with server-side
   caching; the browser only ever talks to same-origin `/bff/*`.

**Framework/stack** (the user fixed the broad strokes; options are
within that):

A. Next.js (App Router) + TypeScript + Tailwind + shadcn/ui + TanStack
   Query + ECharts.
B. Vite SPA + the same libraries (no SSR/BFF).

**Deploy**: Railway (container) vs Vercel vs the NetCup origin/nginx.

## Decision Outcome

**Architecture: Option 2 (BFF).** **Stack: Option A.** **Deploy:
Railway** (container), same provider posture as the rest of the rebuild's
managed pieces.

### Stack (pinned at scaffold time to current stable)

- **Next.js (App Router) + React + TypeScript**, server components by
  default; client components only for interactive/charting islands.
- **Tailwind CSS + shadcn/ui** (Radix primitives) for the design system;
  dark/light via `next-themes`.
- **TanStack Query v5** for client cache/refetch, with `staleTime`
  aligned to the v1 API cache TTLs (pool 10 s, wallet 5 s) so we don't
  out-poll the upstream cache.
- **ECharts** (`echarts-for-react`) for all time-series/﻿categorical
  charts (canvas perf, rich interactions) — replaces Chart.js.
- **Money**: a tiny `KasAmount` helper consumes `{sompi, kas}` and
  formats from the integer `sompi` (via a decimal lib), never parsing
  `kas` as an IEEE float for precision-sensitive display.

### Design language (the bar: "puts other pools to shame")

This is a hard requirement, not a nicety: the dashboard must read as a
modern, premium, *bleeding-edge* product. The visual system is specified
here so the build is deterministic rather than improvised.

- **Identity & theme.** Dark-first (default), with a fully realized light
  theme — both driven by CSS variables + `next-themes`, no hard-coded
  colors in components. Brand axis: Kaspa teal (`#49EACB`) as the primary
  accent and NACHO as a secondary accent, expressed through an OKLCH
  token ramp (consistent perceptual lightness across the palette).
- **Design tokens.** A single `tokens.css` (and Tailwind theme extension)
  owns color, spacing (4px base grid), radius (`xl`/`2xl` cards), shadow
  (layered, low-opacity), and typography. Type: a geometric/grotesk
  display face for headings + `Geist`/`Inter` for body, **tabular-nums**
  everywhere numbers live (hashrate, money, counts) so columns don't jitter.
- **Surface system.** Glass/elevated cards on a subtle dotted or
  radial-gradient backdrop; 1px hairline borders in a low-contrast token;
  no flat gray boxes. Each KPI is a "stat card" with label, big tabular
  value, unit, and a sparkline + delta chip (▲/▼ vs previous window).
- **Data-viz.** ECharts with a **custom katpool theme** (gradient area
  fills, rounded bars, teal/NACHO series, dark-aware axis/grid, unified
  tooltip). Hashrate auto-scales units (H/s→TH/s→PH/s). Time-series get
  brushing/zoom + range toggles (1h/24h/7d/30d/90d/1y). Empty/loading
  states are first-class (skeleton shimmer, never layout shift).
- **Motion.** Framer Motion for tasteful entrance/stagger on cards, number
  count-up on KPIs, and route transitions; all motion respects
  `prefers-reduced-motion`. Live values pulse subtly on refresh.
- **Layout.** Responsive 12-col grid; a collapsible sidebar + sticky
  command bar (global wallet search with ⌘K palette). Mobile: bottom-nav,
  single-column card stacks, charts remain interactive.
- **Polish details.** Copy-to-clipboard on every address/hash with toast;
  explorer deep-links; relative + absolute timestamps; per-card info
  tooltips; favicon/OG images; 404/500 designed pages.
- **Quality gates.** Lighthouse ≥ 90 (perf/a11y/best-practices/SEO) on
  Pool Overview + Miner; keyboard-navigable; AA contrast in both themes;
  zero CLS on first paint.

### Data-flow (BFF)

```
browser ──same-origin──▶ Next.js Route Handlers (/bff/*)
                           ├─ katpool v1 API   (KATPOOL_API_BASE_URL, server-side)
                           ├─ Kaspa public API (network context)
                           └─ price oracle      (KAS/NACHO USD, keyed)
```

The browser never holds API base URLs/keys and never triggers CORS. The
v1 API keeps `KATPOOL_API_CORS_ALLOW_ORIGIN` **unset** (BFF calls it
server-side). BFF responses are cached server-side at the v1 TTLs;
TanStack Query handles client revalidation + background refetch.

### Information architecture (pages)

| Route | Purpose |
|---|---|
| `/` Pool Overview | pool KPIs, hashrate history, recent blocks, recent payouts, network context |
| `/miner?address=` | wallet-scoped: balances/rebate, hashrate history, workers, recent payouts, rejects, rebate tier |
| `/blocks` | paginated found-blocks history (keyset) |
| `/payouts` | pool-wide payout cycles; `/miner` links to per-wallet payouts |
| `/status` | `/health` `/ready` `/started` of the API + upstreams |
| `/resources` | static guides/links, connect wizard (region + port→difficulty seed) |
| `/leaders` | top-miners leaderboard (`/api/v1/pool/leaderboard`) — **ships now** |

Wallet search is validated (`kaspa:`/`kaspatest:`), persisted to
`localStorage` + `?address=`, and miner widgets show a locked state
until a valid address is entered.

### Data map (widget → source)

Pool Overview / wallet views bind to the v1 endpoints from the
inventory (`/api/v1/pool/stats|hashrate|hashrate/history|blocks|payouts`,
`/api/v1/balance/{a}`, `/api/v1/miners/{a}{,/workers,/hashrate/history,
/payouts,/rejects}`, `/api/v1/full_rebate/{a}`). The full
endpoint→widget table lives in the build's `README` and is kept in sync
with `api/tests/wire_contract.rs` (the wire-contract snapshot).

**Sourcing of non-v1 / newly-added data:**

- *Network context* (network hashrate, difficulty, coin supply, halving,
  KAS/NACHO USD): sourced via the BFF from the Kaspa public API
  (`api.kaspa.org`) + **CoinGecko** (KAS + `nacho-the-kat`), the latter
  keyed by `COINGECKO_API_KEY` held **server-side only**. USD
  valuations, "pool luck", and earnings estimates are derived in the BFF
  from pool hashrate + network reward + price. No price/network call
  ever originates in the browser.
- *Top-miners leaderboard*, *active-miners-over-time*, *miner-firmware
  breakdown*: **now first-class v1 endpoints** (see the Context update)
  — `GET /api/v1/pool/{leaderboard,miners/history,firmware}`. The
  `/leaders` page and the firmware chart ship in this build. The
  firmware breakdown renders a "collecting since deploy" affordance
  while `connection_session` fills, since it is populated forward-only.

### Consequences

- Positive: one hardened browser origin; secrets/keys server-side; the
  v1 API stays least-privilege read-only with CORS off.
- Positive: a single, typed pool source; external context isolated to
  one BFF seam that's easy to cache and swap.
- Positive: modern, fast, beautiful, accessible; charts scale to large
  series via ECharts canvas.
- Negative: BFF adds a hop + a deployable. Mitigation: trivial Route
  Handlers, cached at upstream TTLs; Railway autoscaling.
- Negative: the firmware breakdown is forward-only (it persists from
  deploy time as sessions close), so it's sparse initially. Mitigation:
  a "collecting since deploy" affordance; no other feature is gated.

### Confirmation

- Lighthouse: performance/﻿a11y/best-practices ≥ 90 on Pool Overview +
  Miner.
- Every bound widget renders from a live tn10 v1 API with correct
  `KasAmount` formatting (sompi-exact), verified against
  `wire_contract.rs` field names.
- No browser network call targets anything but the dashboard's own
  origin (verified in devtools/CI e2e).
- Wallet flow: invalid → locked state; valid → full data; refresh
  cadence ≤ upstream TTL.

## Pros and Cons of the Options

### Architecture 1 — direct SPA→v1

- Good: no extra deployable.
- Bad: forces CORS on the public API; leaks base URLs; no place to hide
  price-oracle keys or aggregate network context; per-client fan-out.

### Architecture 2 — BFF (chosen)

- Good: one hardened origin, server-side keys/caching, clean seam for
  non-pool data.
- Bad: one more service (small, cacheable).

### Stack A (Next.js + shadcn + TanStack + ECharts) — chosen

- Good: SSR/RSC + Route Handlers give the BFF for free; mature, current,
  gorgeous component/chart story.
- Bad: heavier than a pure SPA. Acceptable for an enterprise dashboard.

### Stack B (Vite SPA)

- Good: simplest build.
- Bad: no first-class BFF; would reintroduce direct fan-out/CORS.

## More Information

- [ADR-0021](0021-public-read-only-http-api.md) (the consumed API),
  [ADR-0022](0022-multiport-stratum-and-flyio-anycast-edge.md).
- v1 wire contract: `api/tests/wire_contract.rs` (authoritative shapes,
  now including `leaderboard`, `active_miners_history`, `firmware`).
- **Resolved forks:** price oracle = **CoinGecko**, key via
  `COINGECKO_API_KEY` (server-side BFF only); deploy = **Railway**.
- The leaderboard/firmware/active-miners endpoints are implemented (this
  phase); firmware data is forward-only (populated as sessions close).

## Implementation status

Built at `new-katpool-dashboard/` and green end-to-end (`tsc --noEmit`,
`next lint`, `next build` — 10 routes, standalone output). Highlights:

- **BFF**: `GET /api/v1/[...path]` (same-origin proxy, allowlisted to
  `pool`/`miners`/`balance`/`full_rebate`) and `GET /api/network`
  (Kaspa API + CoinGecko aggregation, server-side key, graceful
  `degraded[]` fallback; Kaspa hashrate normalized from TH/s → H/s).
- **Design system**: OKLCH token ramp in `globals.css` (dark-first +
  light via `next-themes`), Geist type with tabular numerals, glass
  surfaces + aurora backdrop, custom-themed ECharts, Framer-Motion
  entrance/count-up (reduced-motion aware), skeleton loading, copy +
  explorer affordances, ⌘-style wallet search, mobile bottom-nav.
- **Pages**: Overview, Blocks, Payouts, Leaderboard, Miner Lookup
  (`/miners/:address`), Status — every v1 endpoint surfaced.
- **Deploy**: `Dockerfile` (Next standalone) + `railway.json`
  (healthcheck `/api/network`).
