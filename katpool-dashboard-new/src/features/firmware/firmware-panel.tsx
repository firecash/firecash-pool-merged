"use client";

import { useMemo } from "react";
import { Info } from "lucide-react";
import { Panel } from "@/components/dashboard/panel";
import { DonutChart } from "@/components/charts/donut-chart";
import { ChartSkeleton, EmptyState, ErrorState } from "@/components/dashboard/states";
import { useFirmware } from "@/lib/api/hooks";
import { formatNumber } from "@/lib/format";

const WINDOW = 24 * 60 * 60;

/** Distribution of miner client software (forward-only: fills from deploy). */
export function FirmwarePanel() {
  const { data, isLoading, isError, refetch } = useFirmware(WINDOW);

  const items = useMemo(
    () =>
      // Count distinct miners (workers) per client, not cumulative sessions: one
      // device reconnecting many times would otherwise dominate the breakdown.
      (data?.entries ?? []).map((e) => ({
        name: e.app ?? "Unknown client",
        value: e.workers,
      })),
    [data],
  );

  const totalMiners = useMemo(() => items.reduce((sum, i) => sum + i.value, 0), [items]);

  return (
    <Panel
      title="Miner software"
      description="Miners by reported stratum user-agent (last 24h)"
    >
      {isError ? (
        <ErrorState onRetry={() => void refetch()} />
      ) : isLoading ? (
        <ChartSkeleton height={300} />
      ) : items.length === 0 ? (
        <EmptyState
          icon={<Info className="size-7" />}
          title="Collecting since deploy"
          description="Firmware data is recorded as miner sessions close, so this fills in over time."
        />
      ) : (
        <DonutChart
          data={items}
          valueFormatter={(v) => `${formatNumber(v)} ${v === 1 ? "miner" : "miners"}`}
          centerValue={formatNumber(totalMiners)}
          centerLabel={totalMiners === 1 ? "miner" : "miners"}
          height={300}
        />
      )}
    </Panel>
  );
}
