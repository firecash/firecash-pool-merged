"use client";

import { useMemo } from "react";
import { Panel } from "@/components/dashboard/panel";
import { DonutChart } from "@/components/charts/donut-chart";
import { ChartSkeleton, EmptyState, ErrorState } from "@/components/dashboard/states";
import { usePoolRejects } from "@/lib/api/hooks";
import { formatNumber } from "@/lib/format";
import { rejectReasonLabel } from "@/lib/reject-reasons";

/** Pool-wide share-reject breakdown by reason — operator anti-abuse view. */
export function PoolRejectsPanel() {
  const { data, isLoading, isError, refetch } = usePoolRejects();

  const items = useMemo(
    () =>
      (data?.by_reason ?? []).map((r) => ({
        name: rejectReasonLabel(r.reason),
        value: r.count,
      })),
    [data],
  );

  return (
    <Panel
      eyebrow="Anti-abuse"
      title="Reject reasons"
      description="Pool-wide rejected shares by reason, recent window"
    >
      {isError ? (
        <ErrorState onRetry={() => void refetch()} />
      ) : isLoading ? (
        <ChartSkeleton height={300} />
      ) : items.length === 0 ? (
        <EmptyState
          title="No rejects in window"
          description="The pool has a clean submission record over the recent window."
        />
      ) : (
        <DonutChart
          data={items}
          valueFormatter={(v) => `${formatNumber(v)} ${v === 1 ? "share" : "shares"}`}
          centerValue={formatNumber(data?.total ?? 0)}
          centerLabel={(data?.total ?? 0) === 1 ? "reject" : "rejects"}
          height={300}
        />
      )}
    </Panel>
  );
}
