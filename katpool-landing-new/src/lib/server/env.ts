import "server-only";

/** Server-side configuration for BFF routes. Never imported by client code. */
export const serverEnv = {
  /** Legacy miningPoolStats feed base (no trailing slash). */
  poolApiBaseUrl: () =>
    (process.env.POOL_API_URL ?? process.env.NEXT_PUBLIC_POOL_API_URL ?? "https://api.katpool.com").replace(
      /\/+$/,
      "",
    ),
} as const;
