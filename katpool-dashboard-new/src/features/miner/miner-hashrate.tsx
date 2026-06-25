"use client";

import { useMemo, useState } from "react";
import { Panel } from "@/components/dashboard/panel";
import { RangeToggle } from "@/components/dashboard/range-toggle";
import { TimeSeriesChart } from "@/components/charts/time-series-chart";
import { ChartSkeleton, EmptyState, ErrorState } from "@/components/dashboard/states";
import { useMinerHashrateHistory } from "@/lib/api/hooks";
import { formatHashrate } from "@/lib/format";
import { resolveRange, type RangeKey } from "@/lib/range";

/** Per-miner hashrate over time. */
export function MinerHashrate({ address }: { address: string }) {
  const [range, setRange] = useState<RangeKey>("24h");
  const resolved = useMemo(() => resolveRange(range), [range]);
  const { data, isLoading, isError, refetch } = useMinerHashrateHistory(address, {
    from: resolved.from,
    to: resolved.to,
    bucket: resolved.bucket,
  });

  const series = useMemo(
    () => [
      {
        name: "Hashrate",
        colorIndex: 0,
        points: (data?.points ?? []).map((p) => ({ t: p.bucket_start, v: p.hashrate_hs })),
      },
    ],
    [data],
  );

  return (
    <Panel
      title="Hashrate"
      description="Your estimated hashrate over time"
      actions={<RangeToggle value={range} onChange={setRange} />}
    >
      {isError ? (
        <ErrorState onRetry={() => void refetch()} />
      ) : isLoading ? (
        <ChartSkeleton height={320} />
      ) : series[0]?.points.length === 0 ? (
        <EmptyState title="No shares in this range" description="Try a wider time range." />
      ) : (
        <TimeSeriesChart series={series} valueFormatter={(v) => formatHashrate(v)} showZoom height={320} />
      )}
    </Panel>
  );
}
