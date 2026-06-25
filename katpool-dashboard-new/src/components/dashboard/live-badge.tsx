"use client";

import { useEffect, useState } from "react";
import { cn } from "@/lib/utils";

/** Format an elapsed gap (ms) as a terse "just now" / "12s" / "3m" label. */
function ago(ms: number): string {
  if (ms < 3_000) return "just now";
  if (ms < 60_000) return `${Math.round(ms / 1000)}s ago`;
  if (ms < 3_600_000) return `${Math.round(ms / 60_000)}m ago`;
  return `${Math.round(ms / 3_600_000)}h ago`;
}

/**
 * A compact "live" affordance for polled panels: a pulsing dot plus a
 * self-ticking "updated Ns ago" read-out, so a 15s poll reads as a living
 * feed instead of a static snapshot. Pass React Query's `dataUpdatedAt`
 * (ms epoch) and `isFetching`.
 */
export function LiveBadge({
  updatedAt,
  isFetching = false,
  className,
}: {
  updatedAt?: number;
  isFetching?: boolean;
  className?: string;
}) {
  const [, setTick] = useState(0);

  // Re-render once a second so the relative read-out stays honest between polls.
  useEffect(() => {
    const id = setInterval(() => setTick((t) => t + 1), 1_000);
    return () => clearInterval(id);
  }, []);

  const label = isFetching
    ? "Updating…"
    : updatedAt
      ? `Updated ${ago(Date.now() - updatedAt)}`
      : "Live";

  return (
    <span
      className={cn(
        "inline-flex items-center gap-1.5 text-[0.6875rem] font-medium text-muted-foreground",
        className,
      )}
      title="Auto-refreshing"
    >
      <span className="relative flex size-1.5">
        <span className="absolute inline-flex size-full animate-ping rounded-full bg-success opacity-75" />
        <span className="relative inline-flex size-1.5 rounded-full bg-success" />
      </span>
      <span className="tnum tabular-nums">{label}</span>
    </span>
  );
}
