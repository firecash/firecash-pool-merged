"use client";

import { useEffect, useRef } from "react";
import * as echarts from "echarts/core";
import { LineChart, BarChart, PieChart, EffectScatterChart } from "echarts/charts";
import {
  GridComponent,
  TooltipComponent,
  LegendComponent,
  DataZoomComponent,
  MarkLineComponent,
  GraphicComponent,
  TitleComponent,
} from "echarts/components";
import { CanvasRenderer } from "echarts/renderers";
import type { EChartsCoreOption } from "echarts/core";
import { cn } from "@/lib/utils";

echarts.use([
  LineChart,
  BarChart,
  PieChart,
  EffectScatterChart,
  GridComponent,
  TooltipComponent,
  LegendComponent,
  DataZoomComponent,
  MarkLineComponent,
  GraphicComponent,
  TitleComponent,
  CanvasRenderer,
]);

interface EChartProps {
  option: EChartsCoreOption;
  className?: string;
  /** Fixed pixel height; the chart fills its container width responsively. */
  height?: number;
  notMerge?: boolean;
  /**
   * Components to fully replace on update (e.g. `["series"]`). Lets live data
   * refreshes swap series cleanly — no stale slices — while merging the rest,
   * so an open tooltip/crosshair survives the refresh instead of vanishing.
   */
  replaceMerge?: string[];
}

interface PendingUpdate {
  option: EChartsCoreOption;
  notMerge: boolean;
  replaceMerge?: string[];
}

/** A disposable, resize-aware ECharts canvas. */
export function EChart({
  option,
  className,
  height = 300,
  notMerge = false,
  replaceMerge,
}: EChartProps) {
  const ref = useRef<HTMLDivElement>(null);
  const chartRef = useRef<echarts.ECharts | null>(null);
  // True while the pointer is over the chart. Applying new data mid-hover
  // strands the axis pointer (it freezes and stops tracking) and resets pie
  // emphasis (a visible flicker) — a known ECharts setOption-during-hover
  // interaction. We hold the latest update here and flush it on pointer-leave.
  //
  // Hover is tracked with the CONTAINER's mouseenter/mouseleave — NOT zrender's
  // mousemove/globalout. ECharts renders its (confined) tooltip as a child of
  // the container, so when the tooltip slides under the cursor the canvas fires
  // a spurious `globalout`; the old code treated that as "left the chart",
  // flushed a setOption mid-hover, and stranded the crosshair (it stopped
  // following the mouse and never cleared). mouseleave ignores moves onto child
  // elements, so it only fires when the pointer truly leaves the chart.
  const hoveringRef = useRef(false);
  const pendingRef = useRef<PendingUpdate | null>(null);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const chart = echarts.init(el, undefined, { renderer: "canvas" });
    chartRef.current = chart;
    const ro = new ResizeObserver(() => chart.resize());
    ro.observe(el);

    const onEnter = () => {
      hoveringRef.current = true;
    };
    const onLeave = () => {
      hoveringRef.current = false;
      const pending = pendingRef.current;
      if (pending) {
        pendingRef.current = null;
        chart.setOption(pending.option, {
          notMerge: pending.notMerge,
          replaceMerge: pending.replaceMerge,
        });
      }
    };
    el.addEventListener("mouseenter", onEnter);
    el.addEventListener("mouseleave", onLeave);

    return () => {
      ro.disconnect();
      el.removeEventListener("mouseenter", onEnter);
      el.removeEventListener("mouseleave", onLeave);
      chart.dispose();
      chartRef.current = null;
    };
  }, []);

  useEffect(() => {
    const chart = chartRef.current;
    if (!chart) return;
    // Defer live refreshes while the user is exploring the chart; the newest
    // update is applied the instant the pointer leaves (see onOut above).
    if (hoveringRef.current) {
      pendingRef.current = { option, notMerge, replaceMerge };
      return;
    }
    chart.setOption(option, { notMerge, replaceMerge });
  }, [option, notMerge, replaceMerge]);

  return <div ref={ref} className={cn("w-full", className)} style={{ height }} />;
}
