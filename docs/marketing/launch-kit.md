# Kat Pool — Findability / Backlink Launch Kit

Practical, copy-paste assets to close the backlink gap against HumPool/WhalePool and
capture HumPool's hashrate as it winds down. Every fact below is verified against the
codebase (`api/src/config.rs`, `katpool-*-new/src/lib/mining.ts`) — keep it accurate;
do not inflate fees or claim features we don't ship.

---

## 1. Canonical pool facts (single source of truth)

| Field | Value |
| --- | --- |
| Pool name | Kat Pool |
| Website | https://katpool.com |
| Dashboard (live stats) | https://app.katpool.com |
| Coin | Kaspa (KAS) |
| Algorithm | kHeavyHash |
| Reward scheme | PROP (proportional, recent-share window) |
| Pool fee | 0.75% topline · ~0.5% effective (33% rebated in NACHO) · 0% effective for NACHO token / Nacho Kats NFT / KATCLAIM NFT holders |
| NACHO rebate | Yes — paid in NACHO (KRC-20) each payout cycle |
| Minimum payout | 10 KAS |
| Stratum host | kas.katpool.com (global anycast → nearest of 7 regions) |
| Stratum ports | 1111, 2222, 3333, 4444, 5555, 6666, 7777, 8888 (vardiff on all; 3333 default) |
| Hardware | All Kaspa kHeavyHash ASICs (IceRiver KS series, Bitmain KS/KA series) |
| Open source | Yes — https://github.com/Nacho-the-Kat/katpool |
| X / Twitter | https://x.com/Katpool_Mining |
| Logo (512px) | https://katpool.com/icon-512.png |
| Country | US |

---

## 2. Descriptions (pick by length limit)

**One-liner (≤90 chars):**
> Open-source Kaspa (KAS) mining pool — PROP payouts, NACHO rebates, fees as low as 0%.

**Short (≤160 chars):**
> Kat Pool is the open-source Kaspa mining pool: global anycast stratum, transparent PROP payouts, NACHO fee rebates, and an effective fee as low as 0%.

**Medium (~300 chars):**
> Kat Pool is a fully open-source Kaspa (KAS) mining pool. It runs a global anycast stratum across seven regions, pays miners on a transparent PROP (proportional) scheme, and rebates part of the 0.75% fee in NACHO — dropping the effective fee to ~0.5%, or 0% for NACHO/NFT holders. 10 KAS minimum payout, all ASICs supported.

**Long (directory "about" field):**
> Kat Pool is an open-source mining pool for Kaspa (KAS), built within the Nacho the Kat ($NACHO) ecosystem. Miners connect to a single global anycast stratum host (kas.katpool.com) that routes each rig to the nearest of seven regions for minimal stale shares. Rewards use a transparent PROP (proportional) scheme over a recent-share window, with a 0.75% topline fee — 33% of which is rebated to every miner in NACHO (KRC-20), for an effective fee around 0.5%. Holders of NACHO tokens or Nacho Kats / KATCLAIM NFTs get 100% of the fee back, a 0% effective fee. Minimum payout is 10 KAS, all Kaspa kHeavyHash ASICs are supported (IceRiver KS, Bitmain KS/KA), and the entire stack is open source and auditable on GitHub. A real-time dashboard (app.katpool.com) shows pool/network hashrate, blocks, payouts, and per-wallet stats.

---

## 3. Directory / backlink targets (replicate competitor links)

Ranked by value. Sourced from the HumPool/WhalePool referring-domain report.

| Priority | Target | Why / action |
| --- | --- | --- |
| 1 | **kaspa.org** ecosystem / mining-pools list | Most authoritative + topical link; AI cites it. Submit via their ecosystem/GitHub list PR or community channels. |
| 2 | **MiningPoolStats** (miningpoolstats.net) | Already integrated via `mps_*` config — confirm listed URL = katpool.com and fee/scheme accurate. The #1 source AI cites for "best kaspa pool". |
| 3 | **WhatToMine** (whattomine.com) | 313 links to HumPool. Submit Kat Pool as a Kaspa pool. |
| 4 | **MiningRigRentals** (miningrigrentals.com) | Add pool so renters can point hashrate at it. |
| 5 | **minerstat** | Add pool to their pool list / mining calculator. |
| 6 | **Miner retailers' "recommended pools"** — bt-miners.com, miner.ae, mineshop.eu, leedminer.com, minerdash.ru | All link competitors; email to be added (template below). |
| 7 | **Reddit r/kaspa, Kaspa Discord, Kaspa Telegram, Bitcointalk** | Community mentions = AI training/citation signal. |

**Anchor-text rule:** keep ~90% branded ("Kat Pool", "katpool.com"); only a light
sprinkle of descriptive ("open source Kaspa mining pool"). A flood of exact-match
"kaspa mining pool" anchors on a new domain looks manipulative and gets discounted.

---

## 4. Outreach templates

### 4a. kaspa.org ecosystem listing
> Subject: Add Kat Pool (open-source Kaspa pool) to the ecosystem list
>
> Hi Kaspa team,
>
> We run Kat Pool (https://katpool.com), a fully open-source Kaspa mining pool
> (source: https://github.com/Nacho-the-Kat/katpool). It offers a global anycast
> stratum, transparent PROP payouts, and NACHO rebates, with a real-time dashboard
> at https://app.katpool.com.
>
> We'd love to be included in the mining-pools / ecosystem listing. Happy to open a
> PR against the relevant repo or provide a logo and details in whatever format you
> prefer. Thanks for building Kaspa!

### 4b. Directory / aggregator (WhatToMine, minerstat, MiningRigRentals)
> Subject: Pool submission — Kat Pool (Kaspa / KAS)
>
> Hi, please add Kat Pool to your Kaspa pool listings:
> - Name: Kat Pool
> - URL: https://katpool.com
> - Coin/algo: Kaspa (KAS) / kHeavyHash
> - Fee: 0.75% (PROP), with NACHO rebates → ~0.5% effective
> - Min payout: 10 KAS
> - Stratum: kas.katpool.com : 1111–8888 (anycast, 7 regions)
> - Logo: https://katpool.com/icon-512.png
> Thanks!

### 4c. Miner retailer "recommended pools" (HumPool-shutdown angle)
> Subject: HumPool winding down — open-source Kaspa alternative for your pool list
>
> Hi [shop],
>
> With HumPool winding down, a lot of Kaspa miners are looking for a new home. We run
> Kat Pool (https://katpool.com) — open source, 0.75% fee with NACHO rebates (as low
> as 0% effective), 10 KAS min payout, supports every IceRiver KS and Bitmain KS/KA
> rig you sell. Would you add us to your recommended-pools page? We have a comparison
> here for your customers: https://katpool.com/vs/humpool. Happy to reciprocate.

---

## 5. Announcements (HumPool migration — time-sensitive)

### 5a. X / Twitter (@Katpool_Mining)
> HumPool is winding down. 🐱
>
> If you're a Kaspa miner looking for a new home, Kat Pool is the open-source
> alternative:
> ✅ Source on GitHub — verify every payout
> ✅ 0.75% fee, rebated in $NACHO (as low as 0% effective)
> ✅ Global anycast stratum, 10 KAS min payout
>
> Switch in 2 min → https://katpool.com/vs/humpool

### 5b. Reddit r/kaspa (discussion, not spam — lead with value)
> Title: Open-source Kaspa pool option as HumPool winds down
>
> With HumPool shutting down, figured I'd share an open-source option for anyone
> migrating: Kat Pool (katpool.com). The whole stack is on GitHub
> (github.com/Nacho-the-Kat/katpool), so payouts are auditable. It uses a PROP
> scheme, 0.75% fee that's partly rebated in NACHO, 10 KAS min payout, and an anycast
> stratum (kas.katpool.com, ports 1111–8888). There's a side-by-side comparison and a
> setup guide on the site. Not affiliated beyond running it — happy to answer setup
> questions. What are folks moving to?

### 5c. Bitcointalk / Discord (short)
> HumPool is winding down — Kat Pool (https://katpool.com) is a fully open-source
> Kaspa pool alternative: PROP payouts, NACHO rebates (0% effective fee for holders),
> 10 KAS min, anycast stratum. Setup + HumPool comparison: https://katpool.com/vs/humpool

---

## 6. Post-launch checklist
- [ ] GSC: both sitemaps submitted, key URLs requested for indexing.
- [ ] Bing: imported from GSC, sitemaps confirmed.
- [ ] MiningPoolStats listing URL = katpool.com, fee/scheme accurate.
- [ ] kaspa.org ecosystem listing requested.
- [ ] WhatToMine + MiningRigRentals + minerstat submitted.
- [ ] Miner-shop outreach sent (bt-miners, miner.ae, mineshop.eu, leedminer, minerdash).
- [ ] X + Reddit + Discord/Telegram announcements posted.
- [ ] GitHub README links https://katpool.com.
- [ ] Re-check GSC Pages report + backlink count in ~7 days.
