"use client";

import Link from "next/link";
import { ArrowUpRight, Crosshair } from "lucide-react";
import { useMinerProfile } from "@/lib/api/hooks";
import { useMyMiner } from "@/lib/use-my-miner";
import { formatHashrate, formatNumber, truncateMiddle } from "@/lib/format";
import { Skeleton } from "@/components/ui/skeleton";
import { useSearchFocus } from "./search-focus";

/**
 * Sidebar companion to {@link PoolPulse}: a live snapshot of the viewer's own
 * miner once they've marked one via "Set as mine". Until then it's a CTA that
 * focuses the wallet search so they can pick theirs.
 */
export function MinerPulse() {
  const { address, hydrated } = useMyMiner();
  const { focusSearch } = useSearchFocus();

  // Render nothing distinguishable until hydrated to avoid a flash / mismatch.
  if (!hydrated) {
    return <div className="mt-3 h-[4.25rem] rounded-xl border border-border bg-elevated/40" />;
  }

  if (!address) {
    return (
      <button
        type="button"
        onClick={focusSearch}
        className="group mt-3 flex w-full items-center gap-2.5 rounded-xl border border-dashed border-border bg-elevated/40 p-3 text-left transition-colors hover:border-primary/50 hover:bg-elevated/70"
      >
        <span className="flex size-7 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary">
          <Crosshair className="size-3.5" />
        </span>
        <span className="min-w-0">
          <span className="block text-xs font-medium text-foreground">Track your miner</span>
          <span className="block truncate text-[0.6875rem] text-muted-foreground">
            Search your address to begin
          </span>
        </span>
      </button>
    );
  }

  return <MinerPulseCard address={address} />;
}

function MinerPulseCard({ address }: { address: string }) {
  const { data, isLoading, isError } = useMinerProfile(address);

  const tone = isError ? "bg-destructive" : isLoading ? "bg-warning" : "bg-success";
  const status = isError ? "Offline" : isLoading ? "Connecting…" : "Live";

  return (
    <Link
      href={`/miners/${encodeURIComponent(address)}`}
      className="group mt-3 block rounded-xl border border-border bg-elevated/60 p-3 elevation-1 transition-colors hover:border-primary/40"
    >
      <div className="flex items-center justify-between gap-2">
        <div className="flex min-w-0 items-center gap-2">
          <span className="relative flex size-2">
            {!isError ? (
              <span className={`absolute inline-flex size-full animate-ping rounded-full opacity-75 ${tone}`} />
            ) : null}
            <span className={`relative inline-flex size-2 rounded-full ${tone}`} />
          </span>
          <span className="text-[0.6875rem] font-medium uppercase tracking-[0.1em] text-muted-foreground">
            Your miner
          </span>
        </div>
        <span className="flex items-center gap-1">
          <span className="text-[0.625rem] font-medium text-muted-foreground">{status}</span>
          <ArrowUpRight className="size-3.5 text-muted-foreground transition-colors group-hover:text-primary" />
        </span>
      </div>
      <p className="mt-1.5 truncate font-mono text-xs text-foreground">
        {truncateMiddle(address, 10, 6)}
      </p>
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
          <span className="text-muted-foreground">Workers</span>
          {isLoading ? (
            <Skeleton className="h-3.5 w-10" />
          ) : (
            <span className="font-medium tnum">{formatNumber(data?.workers_count ?? 0)}</span>
          )}
        </div>
      </div>
    </Link>
  );
}
