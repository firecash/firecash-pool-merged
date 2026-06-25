"use client";

import { useMemo } from "react";
import type { EChartsCoreOption } from "echarts/core";
import { EChart } from "@/components/charts/echart";
import { useChartTokens } from "@/components/charts/use-tokens";
import { withAlpha } from "@/components/charts/color";

/** A minimal area sparkline for stat cards (no axes, no tooltip). */
export function Sparkline({
  data,
  colorIndex = 0,
  height = 44,
}: {
  data: number[];
  colorIndex?: number;
  height?: number;
}) {
  const tokens = useChartTokens();
  const option = useMemo<EChartsCoreOption>(() => {
    const color = tokens.series[colorIndex % tokens.series.length] ?? "#49eacb";
    return {
      animation: false,
      grid: { left: 0, right: 0, top: 2, bottom: 2 },
      xAxis: { type: "category", show: false, boundaryGap: false },
      yAxis: { type: "value", show: false, scale: true },
      series: [
        {
          type: "line",
          data,
          smooth: true,
          showSymbol: false,
          lineStyle: { width: 2, color, shadowColor: withAlpha(color, 0.4), shadowBlur: 6 },
          areaStyle: {
            color: {
              type: "linear",
              x: 0,
              y: 0,
              x2: 0,
              y2: 1,
              colorStops: [
                { offset: 0, color: withAlpha(color, 0.35) },
                { offset: 1, color: withAlpha(color, 0) },
              ],
            },
          },
        },
      ],
    };
  }, [data, tokens, colorIndex]);

  if (!data.length) return <div style={{ height }} />;
  return <EChart option={option} height={height} notMerge />;
}
