import "server-only";

/** Read a required env var, throwing a clear error when missing at runtime. */
function required(key: string, fallback?: string): string {
  const value = process.env[key] ?? fallback;
  if (value == null || value === "") {
    throw new Error(`Missing required environment variable: ${key}`);
  }
  return value;
}

/** Server-side configuration. Never imported by client components. */
export const serverEnv = {
  /** katpool v1 API base (no trailing slash). */
  katpoolApiBaseUrl: () =>
    required("KATPOOL_API_BASE_URL", "https://kas.katpool.com/api/v1").replace(/\/+$/, ""),

  /** Kaspa public REST API base. */
  kaspaApiBaseUrl: () =>
    (process.env.KASPA_API_BASE_URL ?? "https://api.kaspa.org").replace(/\/+$/, ""),

  /** CoinGecko API key (optional; demo endpoint works key-less but rate-limited). */
  coinGeckoApiKey: () => process.env.COINGECKO_API_KEY ?? "",

  /** CoinGecko plan — `pro` or `demo`. Selects host + auth header. */
  coinGeckoPlan: (): "pro" | "demo" =>
    (process.env.COINGECKO_PLAN ?? "demo").toLowerCase() === "pro" ? "pro" : "demo",
} as const;
