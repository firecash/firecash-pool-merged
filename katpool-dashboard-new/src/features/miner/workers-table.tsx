"use client";

import { Panel } from "@/components/dashboard/panel";
import { EmptyState, ErrorState, LoadingRows } from "@/components/dashboard/states";
import { LiveBadge } from "@/components/dashboard/live-badge";
import { useMinerWorkers } from "@/lib/api/hooks";
import { formatDateTime, formatHashrate, formatNumber, formatRelative } from "@/lib/format";
import { cn } from "@/lib/utils";

/** A worker is considered live if it submitted within this window. */
const ACTIVE_WINDOW_MS = 10 * 60 * 1000;

/** Per-worker breakdown for a miner. */
export function WorkersTable({ address }: { address: string }) {
  const { data, isLoading, isError, refetch, dataUpdatedAt, isFetching } = useMinerWorkers(address);
  const workers = data?.workers ?? [];

  return (
    <Panel
      title="Workers"
      description="Per-rig activity in the recent window"
      actions={<LiveBadge updatedAt={dataUpdatedAt} isFetching={isFetching} className="hidden sm:inline-flex" />}
      bodyClassName="p-0"
    >
      {isError ? (
        <div className="p-5">
          <ErrorState onRetry={() => void refetch()} />
        </div>
      ) : isLoading ? (
        <div className="p-5">
          <LoadingRows rows={5} />
        </div>
      ) : workers.length === 0 ? (
        <div className="p-5">
          <EmptyState title="No active workers" description="Workers appear here when they submit shares." />
        </div>
      ) : (
        <div className="overflow-x-auto">
          <table className="w-full min-w-[520px] text-sm" aria-label="Workers">
            <thead>
              <tr className="border-b border-border text-left text-xs uppercase tracking-wide text-muted-foreground">
                <th className="px-3 py-3 sm:px-5 font-medium">Worker</th>
                <th className="px-3 py-3 sm:px-5 text-right font-medium">Hashrate</th>
                <th className="px-3 py-3 sm:px-5 text-right font-medium">Shares</th>
                <th className="px-3 py-3 sm:px-5 text-right font-medium">Last seen</th>
              </tr>
            </thead>
            <tbody>
              {workers.map((w) => {
                const lastSeen = new Date(w.last_seen_at).getTime();
                const online =
                  Number.isFinite(lastSeen) && Date.now() - lastSeen < ACTIVE_WINDOW_MS;
                return (
                  <tr
                    key={w.name}
                    className={cn(
                      "border-b border-border/60 transition-colors hover:bg-muted/40",
                      !online && "text-muted-foreground",
                    )}
                  >
                    <td className="px-3 py-3 sm:px-5 font-medium">
                      <span className="inline-flex items-center gap-2">
                        <span
                          className={cn(
                            "size-1.5 shrink-0 rounded-full",
                            online ? "bg-success" : "bg-muted-foreground/40",
                          )}
                          title={online ? "Active" : "Idle"}
                        />
                        {w.name}
                      </span>
                    </td>
                    <td className="px-3 py-3 sm:px-5 text-right tnum">{formatHashrate(w.hashrate_hs)}</td>
                    <td className="px-3 py-3 sm:px-5 text-right tnum text-muted-foreground">
                      {formatNumber(w.accepted_shares)}
                    </td>
                    <td
                      className="px-3 py-3 sm:px-5 text-right text-muted-foreground"
                      title={formatDateTime(w.last_seen_at)}
                    >
                      {formatRelative(w.last_seen_at)}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}
    </Panel>
  );
}
