"use client";

import { useMemo } from "react";
import { Panel } from "@/components/dashboard/panel";
import { HBarChart } from "@/components/charts/bar-chart";
import { ChartSkeleton, EmptyState, ErrorState } from "@/components/dashboard/states";
import { useMinerRejects } from "@/lib/api/hooks";
import { formatNumber } from "@/lib/format";
import { rejectReasonLabel } from "@/lib/reject-reasons";

/** Reject reasons for a miner, as a horizontal bar chart. */
export function RejectsPanel({ address }: { address: string }) {
  const { data, isLoading, isError, refetch } = useMinerRejects(address);

  const bars = useMemo(
    () =>
      (data?.by_reason ?? []).map((r) => ({
        label: rejectReasonLabel(r.reason),
        value: r.count,
      })),
    [data],
  );

  return (
    <Panel title="Rejected shares" description={data ? `${formatNumber(data.total)} total in window` : "By reason"}>
      {isError ? (
        <ErrorState onRetry={() => void refetch()} />
      ) : isLoading ? (
        <ChartSkeleton height={220} />
      ) : bars.length === 0 ? (
        <EmptyState title="No rejects" description="This miner has a clean record in the window." />
      ) : (
        <HBarChart data={bars} valueFormatter={(v) => formatNumber(v)} colorIndex={4} height={Math.max(180, bars.length * 48)} />
      )}
    </Panel>
  );
}
