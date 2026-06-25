"use client";

import { useMemo, useRef } from "react";
import type { EChartsCoreOption } from "echarts/core";
import { EChart } from "./echart";
import { useChartTokens } from "./use-tokens";
import { withAlpha } from "./color";
import { chartTooltip, splitLine } from "./theme";

export interface BarDatum {
  label: string;
  value: number;
}

interface HBarChartProps {
  data: BarDatum[];
  height?: number;
  valueFormatter?: (v: number) => string;
  colorIndex?: number;
}

/** A themed horizontal bar chart (e.g. reject reasons, top workers). */
export function HBarChart({
  data,
  height = 280,
  valueFormatter = (v) => v.toLocaleString("en-US"),
  colorIndex = 0,
}: HBarChartProps) {
  const tokens = useChartTokens();

  // Keep an inline `valueFormatter` from busting the option memo on each poll.
  const fmtRef = useRef(valueFormatter);
  fmtRef.current = valueFormatter;

  const option = useMemo<EChartsCoreOption>(() => {
    const color = tokens.series[colorIndex % tokens.series.length] ?? "#49eacb";
    const sorted = [...data].sort((a, b) => a.value - b.value);
    return {
      animationDuration: 600,
      animationEasing: "cubicOut" as const,
      // Reserve room on the right for the unit-bearing value label (e.g.
      // "3 sessions") so it never clips against the card edge.
      grid: { left: 8, right: 64, top: 8, bottom: 8, containLabel: true },
      tooltip: {
        trigger: "axis",
        axisPointer: { type: "shadow", shadowStyle: { color: withAlpha(color, 0.08) } },
        valueFormatter: (v: unknown) => fmtRef.current(Number(v)),
        ...chartTooltip(tokens),
      },
      xAxis: {
        type: "value",
        // Counts are whole numbers; keep ticks integer and unit-free (the unit
        // lives on the bar label + tooltip) so short ranges don't overlap.
        minInterval: 1,
        axisLabel: {
          color: tokens.muted,
          fontSize: 11,
          formatter: (v: number) => v.toLocaleString("en-US"),
        },
        splitLine: splitLine(tokens),
      },
      yAxis: {
        type: "category",
        data: sorted.map((d) => d.label),
        axisLine: { show: false },
        axisTick: { show: false },
        axisLabel: { color: tokens.muted, fontSize: 12 },
      },
      series: [
        {
          type: "bar",
          data: sorted.map((d) => d.value),
          barWidth: "58%",
          showBackground: true,
          backgroundStyle: { color: withAlpha(tokens.muted, 0.07), borderRadius: 6 },
          itemStyle: {
            borderRadius: [0, 6, 6, 0],
            color: {
              type: "linear",
              x: 0,
              y: 0,
              x2: 1,
              y2: 0,
              colorStops: [
                { offset: 0, color: withAlpha(color, 0.55) },
                { offset: 1, color },
              ],
            },
          },
          label: {
            show: true,
            position: "right",
            color: tokens.muted,
            fontSize: 11,
            formatter: (p: { value: number }) => fmtRef.current(p.value),
          },
        },
      ],
    };
  }, [data, tokens, colorIndex]);

  return <EChart option={option} height={height} replaceMerge={REPLACE_SERIES} />;
}

const REPLACE_SERIES = ["series"];
