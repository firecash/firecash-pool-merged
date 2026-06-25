# katpool dashboard

A bleeding-edge, real-time analytics dashboard for the **katpool** Kaspa
mining pool. It renders the pool's public, read-only v1 API alongside live
Kaspa network and market data.

> Architecture & decisions: see
> [`docs/decisions/0023-new-dashboard-architecture-and-stack.md`](../docs/decisions/0023-new-dashboard-architecture-and-stack.md).

## Stack

- **Next.js 15** (App Router) · **React 19** · **TypeScript** (strict)
- **Tailwind CSS v4** with an OKLCH design-token system (dark-first + light)
- **TanStack Query v5** for live polling
- **ECharts** (custom themed wrapper) for all data-viz
- **Framer Motion** for entrance/number motion (respects reduced-motion)
- Hand-rolled, Radix-based UI primitives

## Architecture

The browser never talks to upstreams directly. A **Backend-for-Frontend**
(Next Route Handlers) proxies and aggregates server-side:

- `GET /api/v1/[...path]` — same-origin read-only proxy to the katpool v1 API
  (`pool`, `miners`, `balance`, `full_rebate` trees only).
- `GET /api/network` — aggregates the Kaspa public API (hashrate, difficulty,
  supply, halving) + **CoinGecko** prices (KAS, NACHO). The CoinGecko key is
  held **server-side only**.

All on-chain amounts are rendered from exact decimal/`BigInt` math — never
floats.

## Pages

Overview · Blocks · Payouts · Leaderboard · Miner Lookup (`/miners/:address`)
· Status.

## Local development

```bash
cp .env.example .env        # fill in KATPOOL_API_BASE_URL, COINGECKO_API_KEY
npm install
npm run dev                 # http://localhost:3000
```

## Quality gates

```bash
npm run typecheck           # tsc --noEmit (strict)
npm run lint                # eslint (next/core-web-vitals + typescript)
npm run build               # production build (standalone output)
```

## Environment variables

| Variable | Required | Description |
|---|---|---|
| `KATPOOL_API_BASE_URL` | yes | katpool v1 API base, e.g. `https://kas.katpool.com/api/v1` |
| `KASPA_API_BASE_URL` | no | Kaspa public API (default `https://api.kaspa.org`) |
| `COINGECKO_API_KEY` | no\* | CoinGecko key (server-side). \*Recommended to avoid rate limits |
| `COINGECKO_PLAN` | no | `demo` (default) or `pro` — selects host + auth header |
| `NEXT_PUBLIC_EXPLORER_BASE_URL` | no | Explorer base for deep links |
| `NEXT_PUBLIC_POOL_NAME` | no | Wordmark / titles (default `katpool`) |

## Deploy (Railway, container)

Railway builds the included `Dockerfile` (Next standalone output) per
`railway.json`. Set the env vars above in the service; the health check
targets `/api/network`.
