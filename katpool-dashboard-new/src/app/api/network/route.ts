import { NextResponse } from "next/server";
import { serverEnv } from "@/lib/server/env";
import { fetchJson, safeUrl } from "@/lib/server/upstream";
import type { NetworkContext } from "@/lib/api/types";

export const runtime = "nodejs";
export const revalidate = 30;

const SOMPI_PER_KAS = 100_000_000;

interface KaspaHashrate {
  hashrate: number;
}
interface KaspaBlockdag {
  difficulty: number;
  blueScore?: string;
  virtualDaaScore?: string;
}
interface KaspaCoinSupply {
  circulatingSupply: string;
  maxSupply: string;
}
interface KaspaBlockReward {
  blockreward: number;
}
interface KaspaHalving {
  nextHalvingTimestamp: number;
  nextHalvingDate: string;
  nextHalvingAmount: number;
}
interface CoinGeckoPrice {
  [id: string]: { usd?: number; usd_market_cap?: number; usd_24h_change?: number };
}

/** Resolve a promise, recording the source name in `degraded` on failure. */
async function settle<T>(name: string, p: Promise<T>, degraded: string[]): Promise<T | null> {
  try {
    return await p;
  } catch (err) {
    degraded.push(name);
    console.error("network source failed", { source: name, error: String(err) });
    return null;
  }
}

function coinGeckoUrl(base: string): { url: string; headers: Record<string, string> } {
  const plan = serverEnv.coinGeckoPlan();
  const key = serverEnv.coinGeckoApiKey();
  const host = plan === "pro" ? "https://pro-api.coingecko.com" : "https://api.coingecko.com";
  const headers: Record<string, string> = {};
  if (key) headers[plan === "pro" ? "x-cg-pro-api-key" : "x-cg-demo-api-key"] = key;
  return { url: `${host}${base}`, headers };
}

export async function GET(): Promise<NextResponse> {
  const kaspa = serverEnv.kaspaApiBaseUrl();
  const degraded: string[] = [];

  const price = coinGeckoUrl(
    "/api/v3/simple/price?ids=kaspa,nacho-the-kat&vs_currencies=usd&include_24hr_change=true&include_market_cap=true",
  );

  const [hashrate, blockdag, supply, reward, halving, prices] = await Promise.all([
    settle("hashrate", fetchJson<KaspaHashrate>(`${kaspa}/info/hashrate?stringOnly=false`, { revalidate: 30 }), degraded),
    settle("blockdag", fetchJson<KaspaBlockdag>(`${kaspa}/info/blockdag`, { revalidate: 30 }), degraded),
    settle("coinsupply", fetchJson<KaspaCoinSupply>(`${kaspa}/info/coinsupply`, { revalidate: 300 }), degraded),
    settle("blockreward", fetchJson<KaspaBlockReward>(`${kaspa}/info/blockreward?stringOnly=false`, { revalidate: 300 }), degraded),
    settle("halving", fetchJson<KaspaHalving>(`${kaspa}/info/halving`, { revalidate: 300 }), degraded),
    settle("prices", fetchJson<CoinGeckoPrice>(price.url, { headers: price.headers, revalidate: 60 }), degraded),
  ]);

  const kas = prices?.["kaspa"];
  const nacho = prices?.["nacho-the-kat"];

  const body: NetworkContext = {
    network_hashrate_hs: hashrate ? hashrate.hashrate * 1e12 : 0,
    difficulty: blockdag?.difficulty ?? 0,
    block_reward_kas: reward?.blockreward ?? 0,
    circulating_supply_kas: supply ? Number(BigInt(supply.circulatingSupply)) / SOMPI_PER_KAS : 0,
    max_supply_kas: supply ? Number(BigInt(supply.maxSupply)) / SOMPI_PER_KAS : 0,
    blue_score: blockdag?.blueScore != null ? Number(blockdag.blueScore) : null,
    next_halving: halving
      ? {
          timestamp: halving.nextHalvingTimestamp,
          date: halving.nextHalvingDate,
          reward_kas: halving.nextHalvingAmount,
        }
      : null,
    prices: {
      kas_usd: kas?.usd ?? null,
      kas_market_cap_usd: kas?.usd_market_cap ?? null,
      kas_change_24h: kas?.usd_24h_change ?? null,
      nacho_usd: nacho?.usd ?? null,
      nacho_change_24h: nacho?.usd_24h_change ?? null,
    },
    degraded,
  };

  if (degraded.length) {
    console.warn("network context degraded", { sources: degraded, kaspa: safeUrl(kaspa) });
  }

  return NextResponse.json(body, {
    headers: { "Cache-Control": "public, s-maxage=30, stale-while-revalidate=120" },
  });
}
