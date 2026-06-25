import { withAlpha } from "./color";
import type { ChartTokens } from "./use-tokens";

/**
 * Bespoke ECharts styling shared across the dashboard's charts so every
 * surface speaks the same visual language: glassy tooltips, a soft dashed
 * crosshair tied to the primary series, and feather-light grid lines.
 */

/** Glassy, rounded tooltip container styled from live theme tokens. */
export function chartTooltip(tokens: ChartTokens) {
  return {
    // Keep the tooltip inside the canvas so the host card's `overflow-hidden`
    // never clips it, and let it glide rather than snap between points.
    confine: true,
    transitionDuration: 0.2,
    backgroundColor: withAlpha(tokens.tooltipBg, 0.9),
    borderColor: tokens.border,
    borderWidth: 1,
    padding: [8, 12] as [number, number],
    textStyle: { color: tokens.text, fontSize: 12, fontWeight: 500 as const },
    extraCssText:
      "border-radius:12px;backdrop-filter:blur(10px) saturate(150%);box-shadow:0 12px 36px rgba(0,0,0,0.28);",
  };
}

/** A soft dashed crosshair keyed off the primary series color. */
export function crosshair(tokens: ChartTokens) {
  const accent = tokens.series[0] ?? "#49eacb";
  return {
    type: "line" as const,
    lineStyle: { color: withAlpha(accent, 0.45), width: 1, type: "dashed" as const },
    label: {
      backgroundColor: withAlpha(tokens.tooltipBg, 0.95),
      color: tokens.muted,
      borderColor: tokens.border,
      borderWidth: 1,
      borderRadius: 6,
      padding: [3, 6] as [number, number],
      fontSize: 11,
    },
  };
}

/** Feather-light horizontal grid lines. */
export function splitLine(tokens: ChartTokens) {
  return { show: true, lineStyle: { color: tokens.grid, width: 1, type: "solid" as const } };
}

/** Vertical gradient area fill (color → transparent) for line/area series. */
export function areaGradient(color: string, topAlpha = 0.3) {
  return {
    type: "linear" as const,
    x: 0,
    y: 0,
    x2: 0,
    y2: 1,
    colorStops: [
      { offset: 0, color: withAlpha(color, topAlpha) },
      { offset: 0.55, color: withAlpha(color, topAlpha * 0.35) },
      { offset: 1, color: withAlpha(color, 0) },
    ],
  };
}
