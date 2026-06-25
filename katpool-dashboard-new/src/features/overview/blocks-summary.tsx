"use client";

import { useMemo } from "react";
import { Panel } from "@/components/dashboard/panel";
import { DonutChart } from "@/components/charts/donut-chart";
import { ChartSkeleton, EmptyState, ErrorState } from "@/components/dashboard/states";
import { usePoolStats } from "@/lib/api/hooks";
import { formatNumber } from "@/lib/format";

/** Distribution of pool blocks across their lifecycle states. */
export function BlocksSummary() {
  const { data, isLoading, isError, refetch } = usePoolStats();

  const items = useMemo(() => {
    const b = data?.blocks;
    if (!b) return [];
    return [
      { name: "Matured", value: b.matured },
      { name: "Confirmed", value: b.confirmed_blue },
      { name: "Submitted", value: b.submitted_to_node },
      { name: "Found", value: b.found },
      { name: "Orphaned", value: b.orphaned },
    ].filter((i) => i.value > 0);
  }, [data]);

  const total = useMemo(() => items.reduce((s, i) => s + i.value, 0), [items]);

  return (
    <Panel title="Blocks by status" description="Lifecycle of all blocks found by the pool">
      {isError ? (
        <ErrorState onRetry={() => void refetch()} />
      ) : isLoading ? (
        <ChartSkeleton height={300} />
      ) : items.length === 0 ? (
        <EmptyState title="No blocks yet" description="Block stats appear once the pool finds its first block." />
      ) : (
        <DonutChart
          data={items}
          valueFormatter={(v) => `${formatNumber(v)} ${v === 1 ? "block" : "blocks"}`}
          centerValue={formatNumber(total)}
          centerLabel={total === 1 ? "block" : "blocks"}
          height={300}
        />
      )}
    </Panel>
  );
}
