import type { BucketToken } from "./api/types";

export type RangeKey = "1h" | "24h" | "7d" | "30d" | "90d" | "1y";

export interface ResolvedRange {
  from: string;
  to: string;
  bucket: BucketToken;
  windowSecs: number;
}

const SECONDS: Record<RangeKey, number> = {
  "1h": 3600,
  "24h": 86_400,
  "7d": 604_800,
  "30d": 2_592_000,
  "90d": 7_776_000,
  "1y": 31_536_000,
};

/** Pick a bucket so a range yields a sensible, bounded number of points. */
function bucketFor(key: RangeKey): BucketToken {
  switch (key) {
    case "1h":
      return "1m";
    case "24h":
      return "5m";
    case "7d":
      return "1h";
    default:
      return "1d";
  }
}

export const RANGE_KEYS: RangeKey[] = ["1h", "24h", "7d", "30d", "90d", "1y"];

export const RANGE_LABELS: Record<RangeKey, string> = {
  "1h": "1H",
  "24h": "24H",
  "7d": "7D",
  "30d": "30D",
  "90d": "90D",
  "1y": "1Y",
};

/**
 * Resolve a range key to API params. `to` is **now** (not bucket-floored) so
 * the API can prorate the trailing bucket; completed buckets remain stable and
 * the live sliding-window estimate is overlaid client-side on the chart tail.
 */
export function resolveRange(key: RangeKey): ResolvedRange {
  const span = SECONDS[key];
  const bucket = bucketFor(key);
  const to = new Date().toISOString();
  const from = new Date(Date.now() - span * 1000).toISOString();
  return { from, to, bucket, windowSecs: span };
}
