import type { HashratePointView } from "./api/types";

/** Sliding window for the live headline hashrate (`pool/stats?window=`). */
export const LIVE_HASHRATE_WINDOW_SECS = 300;

/** Poll cadence for the live headline (ms). */
export const LIVE_HASHRATE_POLL_MS = 5_000;

/** Compare live rate to a reference bucket from the history series. */
export function hashrateDeltaPercent(
  live: number,
  reference: number | null | undefined,
): number | null {
  if (reference == null || reference === 0) return null;
  return ((live - reference) / reference) * 100;
}

/**
 * Index of the history point `lookbackSecs` before the series end, using the
 * chart's bucket width. Returns `null` when the range is too short.
 */
export function referenceBucketIndex(
  pointCount: number,
  bucketSecs: number,
  lookbackSecs: number,
): number | null {
  const bucketsBack = Math.max(1, Math.round(lookbackSecs / bucketSecs));
  const idx = pointCount - 1 - bucketsBack;
  return idx >= 0 ? idx : null;
}

/**
 * Build sparkline values from bucketed history, substituting the prorated
 * trailing bucket (if any) with the live sliding-window estimate.
 */
export function sparklineWithLive(
  points: HashratePointView[],
  liveHs: number | null,
): number[] {
  if (liveHs == null) return points.map((p) => p.hashrate_hs);
  const values = points.map((p) => p.hashrate_hs);
  if (values.length === 0) return [liveHs];
  const last = points[points.length - 1];
  if (last?.partial) return [...values.slice(0, -1), liveHs];
  return [...values, liveHs];
}

/** Append or replace the chart's trailing point with the live estimate. */
export function chartPointsWithLive(
  points: { t: string; v: number }[],
  history: HashratePointView[],
  liveHs: number | null,
  asOf: string | null,
): { t: string; v: number }[] {
  if (liveHs == null) return points;
  const stamp = asOf ?? new Date().toISOString();
  const live = { t: stamp, v: liveHs };
  const lastHist = history[history.length - 1];
  if (lastHist?.partial && points.length > 0) {
    return [...points.slice(0, -1), live];
  }
  return [...points, live];
}
