/**
 * Canonical brand + deployment URLs for metadata, sitemap, robots and JSON-LD.
 *
 * The dashboard is served from its own origin (e.g. `app.katpool.com`), which
 * is distinct from the marketing brand site (`katpool.com`). Keeping them
 * separate means canonical/OG/sitemap URLs resolve against the dashboard's own
 * host while JSON-LD can still link back to the brand for entity consolidation.
 */

/** The dashboard's own public origin — used for metadataBase, canonical, sitemap. */
export const APP_ORIGIN = process.env.NEXT_PUBLIC_APP_URL ?? "https://app.katpool.com";

/** The marketing/brand site — used as the Organization URL and cross-links. */
export const SITE_URL = process.env.NEXT_PUBLIC_SITE_URL ?? "https://katpool.com";

export const GITHUB_URL =
  process.env.NEXT_PUBLIC_GITHUB_URL ?? "https://github.com/Nacho-the-Kat/katpool";

export const TWITTER_URL = process.env.NEXT_PUBLIC_TWITTER_URL ?? "https://x.com/Katpool_Mining";

/** X / Twitter handle (with leading @) derived from the profile URL. */
export const TWITTER_HANDLE = `@${TWITTER_URL.replace(/\/+$/, "").split("/").pop()}`;

export const POOL_NAME = process.env.NEXT_PUBLIC_POOL_NAME ?? "katpool";
