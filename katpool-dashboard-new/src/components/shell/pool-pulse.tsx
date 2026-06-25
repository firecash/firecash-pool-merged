"use client";

import { usePoolStats } from "@/lib/api/hooks";
import { formatHashrate, formatNumber } from "@/lib/format";
import { Skeleton } from "@/components/ui/skeleton";

/** A compact live pool status block for the sidebar footer. */
export function PoolPulse() {
  const { data, isLoading, isError } = usePoolStats();

  // Colour the pulse by real connection state: red when unreachable, a calm
  // amber while first connecting, green only once live data is in hand.
  const tone = isError ? "bg-destructive" : isLoading ? "bg-warning" : "bg-success";
  const status = isError ? "Offline" : isLoading ? "Connecting…" : "Live";

  return (
    <div className="mt-4 rounded-xl border border-border bg-elevated/60 p-3 elevation-1">
      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-2">
          <span className="relative flex size-2">
            {!isError ? (
              <span className={`absolute inline-flex size-full animate-ping rounded-full opacity-75 ${tone}`} />
            ) : null}
            <span className={`relative inline-flex size-2 rounded-full ${tone}`} />
          </span>
          <span className="text-[0.6875rem] font-medium uppercase tracking-[0.1em] text-muted-foreground">
            Pool
          </span>
        </div>
        <span className="text-[0.625rem] font-medium text-muted-foreground">{status}</span>
      </div>
      <p className="mt-1.5 text-[0.6875rem] text-muted-foreground/70">Network-wide totals</p>
      <div className="mt-2 space-y-1">
        <div className="flex items-center justify-between text-xs">
          <span className="text-muted-foreground">Hashrate</span>
          {isLoading ? (
            <Skeleton className="h-3.5 w-16" />
          ) : (
            <span className="font-medium tnum">{formatHashrate(data?.hashrate_hs ?? 0)}</span>
          )}
        </div>
        <div className="flex items-center justify-between text-xs">
          <span className="text-muted-foreground">Miners</span>
          {isLoading ? (
            <Skeleton className="h-3.5 w-10" />
          ) : (
            <span className="font-medium tnum">{formatNumber(data?.miners_active ?? 0)}</span>
          )}
        </div>
      </div>
    </div>
  );
}
