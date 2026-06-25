import type { MetadataRoute } from "next";
import { APP_ORIGIN } from "@/lib/brand";

/**
 * Welcome every search + AI answer-engine crawler so the live pool dashboard is
 * eligible to surface in Google/Bing and in ChatGPT/Perplexity/Claude/Gemini
 * answers. Per-miner pages and API routes are disallowed to keep crawl budget
 * on the canonical pool pages.
 */
export default function robots(): MetadataRoute.Robots {
  const aiAndSearchBots = [
    "Googlebot",
    "Bingbot",
    "DuckDuckBot",
    "GPTBot",
    "OAI-SearchBot",
    "ChatGPT-User",
    "PerplexityBot",
    "Perplexity-User",
    "ClaudeBot",
    "Claude-Web",
    "anthropic-ai",
    "Google-Extended",
    "Applebot",
    "Applebot-Extended",
    "Amazonbot",
    "CCBot",
    "cohere-ai",
    "Bytespider",
  ];

  const allow = "/";
  const disallow = ["/miners/", "/api/"];

  return {
    rules: [
      { userAgent: "*", allow, disallow },
      ...aiAndSearchBots.map((userAgent) => ({ userAgent, allow, disallow })),
    ],
    sitemap: `${APP_ORIGIN}/sitemap.xml`,
    host: APP_ORIGIN,
  };
}
