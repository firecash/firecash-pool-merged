"use client";

import { useMemo } from "react";
import { Panel } from "@/components/dashboard/panel";
import { TimeSeriesChart, type SeriesDef } from "@/components/charts/time-series-chart";
import { ChartSkeleton, EmptyState, ErrorState } from "@/components/dashboard/states";
import { usePayoutCycles } from "@/lib/api/hooks";
import { formatKas } from "@/lib/format";

const WINDOW = 100;

/**
 * Cumulative value distributed to miners over time, derived from settled
 * payout cycles — a "treasury outflow" view. KAS and NACHO (in KAS value)
 * are tracked as separate cumulative series.
 */
export function PayoutFlow() {
  const { data, isLoading, isError, refetch } = usePayoutCycles(WINDOW);

  const series = useMemo<SeriesDef[]>(() => {
    // Only settled cycles represent value actually distributed. Planned,
    // broadcasting, and failed cycles are excluded so the cumulative outflow
    // never overstates what miners have been paid.
    const cycles = [...(data?.cycles ?? [])]
      .filter((c) => c.status === "settled" && c.settled_at != null)
      .reverse(); // oldest → newest
    let kas = 0;
    let nacho = 0;
    const kasPts: { t: string; v: number }[] = [];
    const nachoPts: { t: string; v: number }[] = [];
    for (const c of cycles) {
      const t = c.settled_at ?? c.planned_at;
      const v = Number(c.total.kas);
      if (!Number.isFinite(v)) continue;
      if (c.kind === "kas") {
        kas += v;
        kasPts.push({ t, v: kas });
      } else {
        nacho += v;
        nachoPts.push({ t, v: nacho });
      }
    }
    const out: SeriesDef[] = [];
    if (kasPts.length) out.push({ name: "KAS distributed", points: kasPts, colorIndex: 0 });
    if (nachoPts.length)
      out.push({ name: "NACHO (KAS value)", points: nachoPts, colorIndex: 1 });
    return out;
  }, [data]);

  // A single settled cycle is still real, distributed value — a one-series
  // view renders it as a pulsed point with its value label rather than hiding
  // it behind the empty state.
  const hasData = series.some((s) => s.points.length > 0);

  return (
    <Panel
      eyebrow="Treasury outflow"
      title="Cumulative payouts"
      description="Value distributed to miners across recent settlement cycles"
    >
      {isError ? (
        <ErrorState onRetry={() => void refetch()} />
      ) : isLoading ? (
        <ChartSkeleton height={300} />
      ) : !hasData ? (
        <EmptyState
          title="No settled cycles yet"
          description="As distribution cycles settle, cumulative KAS and NACHO outflow will chart here."
        />
      ) : (
        <TimeSeriesChart
          series={series}
          valueFormatter={(v) => formatKas(String(v))}
          height={300}
        />
      )}
    </Panel>
  );
}
