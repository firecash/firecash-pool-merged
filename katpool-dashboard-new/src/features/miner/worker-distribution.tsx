"use client";

import { useMemo } from "react";
import { Panel } from "@/components/dashboard/panel";
import { DonutChart } from "@/components/charts/donut-chart";
import { ChartSkeleton, EmptyState, ErrorState } from "@/components/dashboard/states";
import { useMinerWorkers } from "@/lib/api/hooks";
import { formatHashrate } from "@/lib/format";

/** Share of a miner's hashrate contributed by each worker rig. */
export function WorkerDistribution({ address }: { address: string }) {
  const { data, isLoading, isError, refetch } = useMinerWorkers(address);

  const items = useMemo(
    () =>
      (data?.workers ?? [])
        .filter((w) => w.hashrate_hs > 0)
        .map((w) => ({ name: w.name, value: w.hashrate_hs })),
    [data],
  );

  const total = useMemo(() => items.reduce((s, i) => s + i.value, 0), [items]);

  return (
    <Panel eyebrow="Distribution" title="Hashrate by worker">
      {isError ? (
        <ErrorState onRetry={() => void refetch()} />
      ) : isLoading ? (
        <ChartSkeleton height={300} />
      ) : items.length === 0 ? (
        <EmptyState
          title="No worker hashrate"
          description="Once workers submit shares, their share of your hashrate charts here."
        />
      ) : (
        <DonutChart
          data={items}
          valueFormatter={(v) => formatHashrate(v)}
          centerValue={formatHashrate(total)}
          centerLabel="total"
          height={300}
        />
      )}
    </Panel>
  );
}
