/**
 * Schema.org JSON-LD for the dashboard. Describes the brand (Organization), the
 * dashboard site (WebSite + wallet SearchAction) and the analytics app itself
 * (WebApplication) so search engines and AI answer engines can resolve Kat Pool
 * as a single entity and understand what the dashboard does.
 */
import { APP_ORIGIN, GITHUB_URL, SITE_URL, TWITTER_URL } from "./brand";

const ORG_ID = `${SITE_URL}/#organization`;
const SITE_ID = `${APP_ORIGIN}/#website`;

export function organizationLd() {
  return {
    "@context": "https://schema.org",
    "@type": "Organization",
    "@id": ORG_ID,
    name: "Kat Pool",
    alternateName: "katpool",
    url: SITE_URL,
    logo: `${APP_ORIGIN}/icon-512.png`,
    description:
      "Open-source Kaspa (KAS) mining pool with global anycast stratum, transparent PROP (proportional) payouts and NACHO fee rebates.",
    sameAs: [TWITTER_URL, GITHUB_URL],
  };
}

export function websiteLd() {
  return {
    "@context": "https://schema.org",
    "@type": "WebSite",
    "@id": SITE_ID,
    name: "Kat Pool Dashboard",
    url: APP_ORIGIN,
    publisher: { "@id": ORG_ID },
    inLanguage: "en",
    potentialAction: {
      "@type": "SearchAction",
      target: {
        "@type": "EntryPoint",
        urlTemplate: `${APP_ORIGIN}/miners/{search_term_string}`,
      },
      "query-input": "required name=search_term_string",
    },
  };
}

export function webApplicationLd() {
  return {
    "@context": "https://schema.org",
    "@type": "WebApplication",
    name: "Kat Pool Dashboard",
    url: APP_ORIGIN,
    applicationCategory: "FinanceApplication",
    operatingSystem: "Web",
    browserRequirements: "Requires JavaScript and a modern browser.",
    description:
      "Real-time Kaspa mining pool analytics: pool and network hashrate, blocks, payout cycles, miner leaderboard, and per-wallet worker stats.",
    provider: { "@id": ORG_ID },
    offers: { "@type": "Offer", price: "0", priceCurrency: "USD" },
  };
}
