/**
 * Live Kaspa network parameters from the official community REST API
 * (api.kaspa.org), used to power the on-site mining calculator.
 *
 * Verified against the kaspa-rest-server source:
 *  - `/info/hashrate`    → network hashrate already expressed in TH/s
 *                          (difficulty × 2 × BPS / 1e12).
 *  - `/info/blockreward` → current per-block subsidy in KAS.
 *  - `/info/price`       → KAS price in USD.
 *
 * Kaspa runs at 10 blocks/sec on mainnet (the Crescendo block rate), so daily
 * network emission = blockReward × BPS × 86,400.
 */
export const KASPA_BPS = 10;
const SECONDS_PER_DAY = 86_400;

const KASPA_API = (process.env.NEXT_PUBLIC_KASPA_API_URL ?? "https://api.kaspa.org").replace(/\/$/, "");

export interface KaspaNetwork {
  priceUsd: number;
  networkHashrateTerahashes: number;
  blockRewardKas: number;
}

async function getJson<T>(path: string): Promise<T> {
  const res = await fetch(`${KASPA_API}${path}`, { next: { revalidate: 300 } });
  if (!res.ok) throw new Error(`${path} ${res.status}`);
  return res.json() as Promise<T>;
}

export async function fetchKaspaNetwork(): Promise<KaspaNetwork> {
  const [price, hashrate, reward] = await Promise.all([
    getJson<{ price: number }>("/info/price"),
    getJson<{ hashrate: number }>("/info/hashrate"),
    getJson<{ blockreward: number }>("/info/blockreward"),
  ]);
  return {
    priceUsd: price.price,
    networkHashrateTerahashes: hashrate.hashrate,
    blockRewardKas: reward.blockreward,
  };
}

/** Network-wide KAS emitted per day at the current block reward and block rate. */
export function dailyNetworkEmissionKas(net: KaspaNetwork): number {
  return net.blockRewardKas * KASPA_BPS * SECONDS_PER_DAY;
}

/**
 * Estimated miner revenue for a given hashrate (in TH/s) after the pool fee.
 * Returns gross KAS/day and USD/day — power costs are applied by the caller.
 */
export function estimateMinerDaily(params: {
  userHashrateTerahashes: number;
  net: KaspaNetwork;
  poolFeePercent: number;
}): { kasPerDay: number; usdPerDay: number } {
  const { userHashrateTerahashes, net, poolFeePercent } = params;
  if (userHashrateTerahashes <= 0 || net.networkHashrateTerahashes <= 0) {
    return { kasPerDay: 0, usdPerDay: 0 };
  }
  const share = userHashrateTerahashes / net.networkHashrateTerahashes;
  const grossKas = share * dailyNetworkEmissionKas(net);
  const kasPerDay = grossKas * (1 - poolFeePercent / 100);
  return { kasPerDay, usdPerDay: kasPerDay * net.priceUsd };
}
