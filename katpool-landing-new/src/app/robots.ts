import type { MetadataRoute } from "next";

const SITE_URL = process.env.NEXT_PUBLIC_SITE_URL ?? "https://katpool.com";

/**
 * Allow everything, and explicitly welcome the major search + AI answer-engine
 * crawlers so Kat Pool is eligible to surface in Google/Bing results and in
 * ChatGPT, Perplexity, Claude, Gemini and Copilot answers. Listing the AI
 * agents by name is an explicit opt-in (some are blocked by default elsewhere)
 * — maximum findability is the goal here.
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

  return {
    rules: [
      { userAgent: "*", allow: "/" },
      ...aiAndSearchBots.map((userAgent) => ({ userAgent, allow: "/" })),
    ],
    sitemap: `${SITE_URL}/sitemap.xml`,
    host: SITE_URL,
  };
}
