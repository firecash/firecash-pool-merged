import { NextResponse, type NextRequest } from "next/server";
import { serverEnv } from "@/lib/server/env";
import { fetchJson, UpstreamError, safeUrl } from "@/lib/server/upstream";

export const runtime = "nodejs";

/**
 * Same-origin read-only proxy to the katpool v1 API.
 *
 * The browser never talks to the public API directly: this keeps the data
 * surface same-origin (no CORS, no client-side base-URL leakage). Responses
 * are marked no-store so Cloudflare and other shared caches never serve stale
 * JSON to live polls.
 */
const ALLOWED_PREFIXES = ["pool", "miners", "balance", "full_rebate"] as const;

/** Per-tree upstream coalesce window (seconds), tuned to the API TTL caches. */
function revalidateFor(first: string): number {
  return first === "balance" || first === "miners" ? 5 : 10;
}

const LIVE_HEADERS = {
  "Cache-Control": "private, no-store, no-cache, must-revalidate",
  "CDN-Cache-Control": "no-store",
} as const;

export async function GET(
  req: NextRequest,
  ctx: { params: Promise<{ path: string[] }> },
): Promise<NextResponse> {
  const { path } = await ctx.params;
  const first = path[0] ?? "";
  if (!ALLOWED_PREFIXES.includes(first as (typeof ALLOWED_PREFIXES)[number])) {
    return NextResponse.json(
      { error: { code: "not_found", message: "not found" } },
      { status: 404 },
    );
  }

  const search = req.nextUrl.search;
  const target = `${serverEnv.katpoolApiBaseUrl()}/${path.map(encodeURIComponent).join("/")}${search}`;

  try {
    const data = await fetchJson<unknown>(target, { revalidate: revalidateFor(first) });
    return NextResponse.json(data, { headers: LIVE_HEADERS });
  } catch (err) {
    const status = err instanceof UpstreamError ? err.status : 502;

    // Pass 429 through verbatim (with Retry-After) rather than masking it as a
    // 502. A 502 reads as a hard fault and triggers the client's retry path,
    // which would pile more requests onto an already-throttled upstream; a 429
    // tells React Query to stop and back off until the next scheduled poll.
    if (status === 429) {
      const retryAfter = err instanceof UpstreamError ? err.retryAfter : undefined;
      return NextResponse.json(
        { error: { code: "rate_limited", message: "rate limited" } },
        { status: 429, headers: retryAfter ? { "Retry-After": retryAfter } : undefined },
      );
    }

    if (status >= 500) {
      console.error("v1 proxy error", { target: safeUrl(target), status });
    }
    return NextResponse.json(
      {
        error: {
          code: status === 404 ? "not_found" : "upstream_error",
          message: status === 404 ? "not found" : "upstream unavailable",
        },
      },
      { status: status === 404 ? 404 : 502 },
    );
  }
}
