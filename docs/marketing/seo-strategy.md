# Kat Pool — Search & AI Ranking Strategy

Evidence-based plan to rank Kat Pool on Google/Bing and get it recommended by AI
answer engines (ChatGPT, Perplexity, Gemini, Copilot) for Kaspa-mining queries.
Based on live SERP analysis (Jun 2026) of "best kaspa mining pool", "kaspa mining
pool", "kaspa mining", plus GEO/AEO research and the competitor backlink report.

---

## 1. The honest reality (read this first)

**"Best kaspa mining pool" is a comparison query — Google answers it with LISTS, not
pool homepages.** The current page-1 winners are listicles and aggregators:

- Listicles: PasingGrades, MiningReturns, EarnifyHub, the MiningPoolStats blog.
- Official Kaspa surfaces: `wiki.kaspa.org/en/mining`, `kaspa.network/mining`, `kaspa.org`.
- Aggregators: `miningpoolstats.stream/kaspa`, minerstat.
- A few **individual pools with dedicated, content-rich landing pages**: `2miners.com/kas-mining-pool`, `pool.kryptex.com/kas`, `kaspa.herominers.com`, `k1pool.com/pool/kaspa`.

Two consequences:
1. We will rarely rank our *homepage* #1 for "best kaspa mining pool." The faster win is
   to **be inside the lists/wikis/aggregators that already rank** AND build our own
   comparison content that can rank for the long tail.
2. Individual pools *can* rank for "kaspa mining pool" (2Miners/Kryptex/HeroMiners prove
   it) — but they have years of domain authority. We have ~1 referring domain. Authority
   is the gap, and it compounds over weeks–months.

**Biggest immediate finding:** Kat Pool is **missing from the Kaspa Wiki pool table**
(`wiki.kaspa.org/en/mining`, 13 pools listed) and is **not visibly listed on
MiningPoolStats**. Those two are both top organic results *and* primary sources AI cites.
Fixing those is the single highest-leverage move.

---

## 2. Query map: what's winnable, and how

| Query | Intent | SERP type | Realistic path | Horizon |
| --- | --- | --- | --- | --- |
| "kat pool" / brand | Navigational | Homepage | Already ours; ensure brand SERP is clean | Now |
| "open source kaspa pool" | Niche | Mixed | We own this angle — uniquely true | Weeks |
| "humpool alternative" | Comparison | Mixed | `/vs/humpool` page + HumPool wind-down PR | Weeks |
| "kaspa nacho pool" / "nacho mining" | Niche | Mixed | Unique to us | Weeks |
| "kaspa mining pool" | Commercial | Pools + lists | Dedicated page + authority (wiki, backlinks) | 1–3 mo |
| "how to mine kaspa" / "mine kaspa" | Informational | Guides | `/kaspa-mining-pool` guide, HowTo schema | 1–3 mo |
| "best kaspa mining pool" | Comparison | Listicles | Get into the listicles + own comparison content | 2–6 mo |
| "kaspa mining" | Broad | Mixed/guides | Topical authority cluster | 3–6 mo |

Strategy: **win the niche/long-tail now, climb the head terms with authority, and attack
"best…" by getting featured in the lists rather than trying to outrank them with a homepage.**

---

## 3. Pillar 1 — Authority & placements (highest ROI)

### 3a. Kaspa Wiki — get into the pool table (do first)
- `wiki.kaspa.org/en/mining` lists 13 pools; Kat Pool is absent.
- Process (`wiki.kaspa.org/en/how-to-help`): sign up (email/Discord/Google), then ask an
  admin for edit rights — **Tim**, **Kingu**, or **IceCreamFish** on the Kaspa Discord —
  or post the request in the **#wiki** channel. Then add Kat Pool: name, link, payout
  system (PROP), threshold (10 KAS), mining-guide link (`katpool.com/kaspa-mining-pool`).
- Value: high-authority, topical, and AI-cited backlink + listing.

### 3b. Awesome Kaspa & dev resources — open PRs (easy wins)
- `github.com/Kasbah-commons/awesome-kaspa` — submit a PR adding Kat Pool (name, URL,
  description, features, license: open source). They explicitly invite PRs / `#show-and-tell`.
- `github.com/Kaspathon/KaspaDev-Resources` — add to relevant section.
- GitHub is weighted heavily by AI models; these double as citation sources.

### 3c. MiningPoolStats — verify the listing (AI's #1 cited source)
- We already feed it via `mps_*` config + `/api/pool/miningPoolStats`. Confirm at
  `miningpoolstats.app` / `miningpoolstats.stream/kaspa` that Kat Pool appears with the
  correct URL (`katpool.com`), fee, PROP scheme, and 10 KAS min.
- If unclaimed/inaccurate: register a developer account ("List Your Project") or claim the
  pool via their Discord with domain verification.

### 3d. minerstat — list the pool
- Get Kat Pool into the minerstat pools directory/API (`api.minerstat.com/v2/pools`) so it
  surfaces in their calculator and is machine-readable for aggregators/AI.

### 3e. Listicle outreach — get featured where the answers live
- PasingGrades, MiningReturns, EarnifyHub, MiningPoolStats blog rank #1 for "best kaspa
  mining pool" and are cited by AI. Pitch inclusion/correction with the HumPool-shutdown
  angle. Being *in* the list = being in the answer.

### 3f. Replicate competitor backlinks (from the backlink report)
- WhatToMine, MiningRigRentals, and miner shops (bt-miners, miner.ae, mineshop.eu,
  leedminer, minerdash) all link HumPool/WhalePool. See `launch-kit.md` for templates.
- Anchor text: ~90% branded ("Kat Pool", "katpool.com"); light descriptive sprinkle only.

---

## 4. Pillar 2 — On-page GEO/AEO (get quoted by AI)

A 2026 ranking-factors study found Article/FAQPage/HowTo schema gave a **+73% AI-Overview
citation rate**; the Princeton GEO study found answer-first + cited data lifts AI citations
up to ~40%. Concrete changes to our content pages:

1. **Answer-first / inverted pyramid.** Under each question-style H2, lead with a
   self-contained **40–100 word** answer that states the fact directly (no "it"/"they" in
   the first sentence, so it's liftable out of context).
2. **Question-style H2s** people actually type: "How do I mine Kaspa?", "What is the best
   Kaspa mining pool?", "Is Kat Pool a good HumPool alternative?", "What does Kat Pool cost?".
3. **HowTo schema** on `/kaspa-mining-pool` (the connect steps) — currently only Article+FAQ.
4. **Comparison tables** (models lift them verbatim). Add a multi-pool comparison table
   (Kat Pool vs 2Miners/WoolyPooly/HeroMiners/HumPool) on the guide or a new `/compare` page.
5. **E-E-A-T signals:** named author/maintainer byline + visible "Published / Updated"
   dates on every content page.
6. **Unique, verifiable data = our moat.** We run a live API. Publish data models *can't*
   recall from memory: a live pool-stats page and a live multi-pool fee/hashrate comparison.
   "According to Kat Pool live data (updated hourly)…" is exactly what gets cited.
7. **Server-render everything.** Content pages are SSR ✓. The landing SPA still renders
   only the hero into HTML — add the full value-prop + FAQ to the homepage HTML so crawlers
   and JS-less AI bots read it.
8. Robots already allow GPTBot/OAI-SearchBot/PerplexityBot/ClaudeBot/Google-Extended ✓.
   ChatGPT Search cites Bing-indexed pages → keep Bing/GSC submissions current ✓.

---

## 5. Pillar 3 — Content expansion (topical authority cluster)

Build a hub-and-spoke cluster around "Kaspa mining" so we own the long tail and feed AI:

- Hardware spokes: "IceRiver KS5 Kaspa mining setup", "Bitmain KS5 pool config",
  "IceRiver KS0/KS1/KS2 guide" — buyer-intent, low competition, link to `/start`.
- "Kaspa mining calculator" — interactive tool (unique data, high citation value).
- "Kaspa mining profitability 2026" — updated data page.
- "PROP vs PPLNS vs PPS for Kaspa" — explainer that positions our scheme.
- "How to switch mining pools (no downtime)" — captures migrators.
Each links internally to the guide, comparison, and dashboard.

---

## 6. Pillar 4 — Brand & community signals (AI training data)

AI models weight Reddit, GitHub, and Discord heavily.
- **Reddit r/kaspa**, **Kaspa Discord** (#mining-and-hardware), **Telegram**, **Bitcointalk**:
  genuine participation + the HumPool-migration post (see `launch-kit.md`).
- **GitHub README** (`Nacho-the-Kat/katpool`) → link `katpool.com` (a backlink we control).
- **X (@Katpool_Mining)**: consistent posting; link from the apps (done) and bio → site.
- Encourage a few real miner reviews/mentions in the community.

---

## 7. Measurement (run weekly)

- **AI citation tracking:** maintain 20–50 high-intent prompts ("best kaspa mining pool",
  "open source kaspa pool", "humpool alternative", "how to mine kaspa", "lowest fee kaspa
  pool"). Run them weekly through ChatGPT, Perplexity, Gemini, Copilot; log where Kat Pool
  appears. Track *share of voice*, not just presence.
- **GSC:** Pages (indexed vs not), Performance (impressions/position for target queries),
  Sitemaps coverage. Bing Webmaster equivalents.
- **Backlinks:** referring-domain count vs HumPool/WhalePool; track new placements.
- Target leading indicators: wiki listing live, MiningPoolStats accurate, +N referring
  domains/mo, first AI citations for niche queries within ~30 days.

---

## 8. 30 / 60 / 90-day plan

**Days 1–30 (authority + quick wins):**
- [ ] Kaspa Wiki listing (3a). Awesome-Kaspa + dev-resources PRs (3b).
- [ ] Verify/claim MiningPoolStats; submit minerstat (3c, 3d).
- [ ] GitHub README links katpool.com; HumPool-migration posts on X/Reddit/Discord (Pillar 4).
- [ ] On-page: answer-first rewrites + HowTo schema + author/dates on the 3 content pages (4).
- [ ] Homepage: render full value-prop + FAQ into HTML (4.7).

**Days 31–60 (content + links):**
- [ ] Publish 3–4 cluster pages (hardware guides, calculator, PROP-vs-PPLNS) (Pillar 3).
- [ ] Multi-pool comparison page with table + schema (4.4).
- [ ] Listicle outreach + miner-shop backlinks (3e, 3f).

**Days 61–90 (scale + measure):**
- [ ] Live pool-stats / live comparison data page (unique-data moat) (4.6).
- [ ] Expand cluster; pursue 2–3 more authoritative backlinks.
- [ ] Weekly citation + GSC review; double down on what's moving.

---

## 9. What we can implement in-code immediately
Falls out of Pillar 2/3 and is fully in our control:
1. Answer-first restructure + question-H2s on `/kaspa-mining-pool`, `/vs/humpool`, `/about`.
2. Add `HowTo` JSON-LD to the guide; add author + `datePublished`/`dateModified` to all.
3. Multi-pool comparison page (`/compare`) with a verbatim-liftable table + schema.
4. Render the homepage value-prop + FAQ into static HTML (fix SPA thin content).
5. Hardware/long-tail guide pages + a Kaspa mining calculator.
6. A live, SSR pool-stats/comparison page powered by our API (unique-data citation magnet).

Off-page items (wiki, MiningPoolStats, PRs, outreach) are documented with copy in
`launch-kit.md`.
