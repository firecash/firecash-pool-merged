"use client";

import { useMemo, useRef } from "react";
import type { EChartsCoreOption } from "echarts/core";
import { EChart } from "./echart";
import { useChartTokens } from "./use-tokens";
import { chartTooltip } from "./theme";
import { useMediaQuery } from "@/lib/use-media-query";

export interface DonutDatum {
  name: string;
  value: number;
}

interface DonutChartProps {
  data: DonutDatum[];
  height?: number;
  valueFormatter?: (v: number) => string;
  centerLabel?: string;
  centerValue?: string;
}

/** A themed donut with a centered headline and legend. */
export function DonutChart({
  data,
  height = 280,
  valueFormatter = (v) => v.toLocaleString("en-US"),
  centerLabel,
  centerValue,
}: DonutChartProps) {
  const tokens = useChartTokens();
  // On narrow viewports a right-hand vertical legend squeezes the ring into a
  // sliver and reads badly; below `lg` we stack the legend underneath instead.
  const isNarrow = useMediaQuery("(max-width: 1023px)");

  // Keep an inline `valueFormatter` from busting the option memo on each poll.
  const fmtRef = useRef(valueFormatter);
  fmtRef.current = valueFormatter;

  const option = useMemo<EChartsCoreOption>(() => {
    const single = data.length === 1;
    // Stack the legend under a centered ring when there's little horizontal
    // room — either very few slices (a right legend leaves dead space) or a
    // narrow screen (a right legend maroons the ring). Otherwise use the
    // space-efficient right-hand vertical legend.
    const compact = data.length <= 2 || isNarrow;
    const ringX = compact ? "50%" : "32%";
    const ringY = compact ? "44%" : "50%";

    return {
      animationDuration: 700,
      animationEasing: "cubicOut" as const,
      color: tokens.series,
      tooltip: {
        trigger: "item",
        valueFormatter: (v: unknown) => fmtRef.current(Number(v)),
        ...chartTooltip(tokens),
      },
      legend: {
        type: "scroll",
        // Long client/worker names would otherwise overrun the canvas.
        formatter: (name: string) => (name.length > 18 ? `${name.slice(0, 17)}…` : name),
        textStyle: { color: tokens.muted, fontSize: 12 },
        itemWidth: 10,
        itemHeight: 10,
        icon: "roundRect",
        ...(compact
          ? { orient: "horizontal" as const, bottom: 0, left: "center" as const, itemGap: 18 }
          : { orient: "vertical" as const, right: 8, top: "middle" as const, itemGap: 12 }),
      },
      // The headline lives in a `title` anchored at the ring's exact centre
      // with both axes centred — far more reliable than hand-offsetting a
      // graphic group (which anchors by its top edge and reads low/small).
      title:
        centerValue != null
          ? {
              text: centerValue,
              subtext: centerLabel ?? "",
              left: ringX,
              top: ringY,
              textAlign: "center" as const,
              textVerticalAlign: "middle" as const,
              itemGap: 6,
              textStyle: {
                color: tokens.text,
                fontSize: 28,
                fontWeight: 700 as const,
              },
              subtextStyle: {
                color: tokens.muted,
                fontSize: 12,
                fontWeight: 500 as const,
              },
            }
          : undefined,
      series: [
        {
          type: "pie",
          radius: ["62%", "84%"],
          center: [ringX, ringY],
          avoidLabelOverlap: true,
          // A single category reads as one continuous ring — no seam.
          itemStyle: {
            borderColor: tokens.card,
            borderWidth: single ? 0 : 2,
            borderRadius: single ? 0 : 6,
          },
          label: { show: false },
          emphasis: { scaleSize: 6, itemStyle: { shadowBlur: 16, shadowColor: "rgba(0,0,0,0.25)" } },
          data: data.map((d) => ({ name: d.name, value: d.value })),
        },
      ],
    };
  }, [data, tokens, centerLabel, centerValue, isNarrow]);

  return <EChart option={option} height={height} replaceMerge={REPLACE_SERIES} />;
}

const REPLACE_SERIES = ["series"];
