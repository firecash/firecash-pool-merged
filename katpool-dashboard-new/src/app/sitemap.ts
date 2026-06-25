import type { MetadataRoute } from "next";
import { APP_ORIGIN } from "@/lib/brand";

/**
 * Static, indexable destinations. Per-miner pages (`/miners/[address]`) are
 * intentionally excluded — they are user-specific, effectively infinite, and
 * marked noindex — so the sitemap stays focused on canonical pool pages.
 */
const ROUTES: { path: string; priority: number; changeFrequency: MetadataRoute.Sitemap[number]["changeFrequency"] }[] = [
  { path: "/", priority: 1, changeFrequency: "always" },
  { path: "/start", priority: 0.9, changeFrequency: "monthly" },
  { path: "/blocks", priority: 0.8, changeFrequency: "hourly" },
  { path: "/payouts", priority: 0.8, changeFrequency: "hourly" },
  { path: "/leaders", priority: 0.7, changeFrequency: "hourly" },
  { path: "/status", priority: 0.5, changeFrequency: "hourly" },
];

export default function sitemap(): MetadataRoute.Sitemap {
  const now = new Date();
  return ROUTES.map((route) => ({
    url: `${APP_ORIGIN}${route.path}`,
    lastModified: now,
    changeFrequency: route.changeFrequency,
    priority: route.priority,
  }));
}
