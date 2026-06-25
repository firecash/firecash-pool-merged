import { NextResponse } from "next/server";
import { serverEnv } from "@/lib/server/env";
import type { MiningPoolStats } from "@/lib/pool-stats";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

const LIVE_HEADERS = {
  "Cache-Control": "private, no-store, no-cache, must-revalidate",
  "CDN-Cache-Control": "no-store",
} as const;

/** Same-origin proxy for the legacy miningPoolStats feed (avoids cross-origin CORS). */
export async function GET(): Promise<NextResponse> {
  const target = `${serverEnv.poolApiBaseUrl()}/api/pool/miningPoolStats`;

  try {
    const res = await fetch(target, {
      cache: "no-store",
      headers: { accept: "application/json" },
    });
    if (!res.ok) {
      return NextResponse.json(
        { error: { code: "upstream_error", message: `pool stats ${res.status}` } },
        { status: res.status >= 500 ? 502 : res.status, headers: LIVE_HEADERS },
      );
    }
    const data = (await res.json()) as MiningPoolStats;
    return NextResponse.json(data, { headers: LIVE_HEADERS });
  } catch {
    return NextResponse.json(
      { error: { code: "upstream_error", message: "upstream unavailable" } },
      { status: 502, headers: LIVE_HEADERS },
    );
  }
}
