#!/usr/bin/env node
/**
 * Submit the site's URLs to IndexNow (Bing, Yandex, Seznam, Naver) for
 * near-instant (re)indexing. Run this AFTER a deploy, once the new URLs are
 * live — e.g. as a post-deploy step or a scheduled job:
 *
 *     npm run indexnow
 *
 * The key is published at https://<host>/<key>.txt (see public/). IndexNow
 * keys are public by design, so committing the key file is expected.
 *
 * Env:
 *   NEXT_PUBLIC_SITE_URL  Canonical site origin (default https://katpool.com)
 *   INDEXNOW_KEY          IndexNow key (defaults to the committed key)
 */
const SITE_URL = (process.env.NEXT_PUBLIC_SITE_URL ?? "https://katpool.com").replace(/\/$/, "");
const KEY = process.env.INDEXNOW_KEY ?? "26d4c75aa86984b7c43e1f296d8bfdfd";
const ENDPOINT = "https://api.indexnow.org/indexnow";

async function getSitemapUrls() {
  const res = await fetch(`${SITE_URL}/sitemap.xml`, { headers: { accept: "application/xml" } });
  if (!res.ok) throw new Error(`sitemap fetch failed: ${res.status}`);
  const xml = await res.text();
  const urls = [...xml.matchAll(/<loc>([^<]+)<\/loc>/g)].map((m) => m[1].trim());
  // Only submit URLs on our own host.
  const host = new URL(SITE_URL).hostname;
  return [...new Set(urls.filter((u) => new URL(u).hostname === host))];
}

async function main() {
  const urlList = await getSitemapUrls();
  if (urlList.length === 0) {
    console.error("No URLs found in sitemap; aborting.");
    process.exit(1);
  }

  const host = new URL(SITE_URL).hostname;
  const body = {
    host,
    key: KEY,
    keyLocation: `${SITE_URL}/${KEY}.txt`,
    urlList,
  };

  const res = await fetch(ENDPOINT, {
    method: "POST",
    headers: { "content-type": "application/json; charset=utf-8" },
    body: JSON.stringify(body),
  });

  // IndexNow returns 200 (accepted) or 202 (accepted, pending verification).
  if (res.ok || res.status === 202) {
    console.log(`IndexNow: submitted ${urlList.length} URLs for ${host} (HTTP ${res.status}).`);
  } else {
    const text = await res.text().catch(() => "");
    console.error(`IndexNow submission failed: HTTP ${res.status} ${text}`);
    process.exit(1);
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
