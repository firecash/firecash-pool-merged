"use client";

import { type ReactNode } from "react";
import { AlertTriangle, Inbox, RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { cn } from "@/lib/utils";

/**
 * A calm, intentional empty-state: a haloed brand glyph, a clear headline,
 * supporting copy, and an optional action. No dashed boxes.
 */
export function EmptyState({
  title,
  description,
  icon,
  action,
  className,
}: {
  title: string;
  description?: string;
  icon?: ReactNode;
  action?: ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn(
        "flex flex-col items-center justify-center gap-3 px-6 py-12 text-center",
        className,
      )}
    >
      <div className="relative grid size-14 place-items-center">
        <div className="absolute inset-0 rounded-2xl bg-gradient-to-br from-primary/15 to-secondary/10 blur-md" />
        <div className="relative grid size-14 place-items-center rounded-2xl border border-border bg-elevated text-muted-foreground/80 elevation-1">
          {icon ?? <Inbox className="size-6" />}
        </div>
      </div>
      <div className="space-y-1">
        <p className="text-sm font-semibold tracking-tight">{title}</p>
        {description ? (
          <p className="mx-auto max-w-xs text-xs leading-relaxed text-muted-foreground">{description}</p>
        ) : null}
      </div>
      {action ? <div className="mt-1">{action}</div> : null}
    </div>
  );
}

/**
 * An inline error state. Deliberately reassuring rather than alarming — a
 * contained amber notice with a one-tap retry, not a red wall.
 */
export function ErrorState({
  message,
  onRetry,
  className,
}: {
  message?: string;
  onRetry?: () => void;
  className?: string;
}) {
  return (
    <div
      className={cn(
        "flex flex-col items-center justify-center gap-3 rounded-xl border border-warning/25 bg-warning/[0.06] px-6 py-10 text-center",
        className,
      )}
    >
      <div className="grid size-10 place-items-center rounded-full bg-warning/15 text-warning">
        <AlertTriangle className="size-5" />
      </div>
      <div className="space-y-1">
        <p className="text-sm font-semibold tracking-tight">Couldn&apos;t load this data</p>
        <p className="mx-auto max-w-xs text-xs leading-relaxed text-muted-foreground">
          {message ?? "We'll keep retrying automatically in the background."}
        </p>
      </div>
      {onRetry ? (
        <Button variant="outline" size="sm" onClick={onRetry} className="mt-1">
          <RefreshCw className="size-3.5" /> Retry now
        </Button>
      ) : null}
    </div>
  );
}

/** A block of shimmering rows for table/list loading. */
export function LoadingRows({ rows = 5, className }: { rows?: number; className?: string }) {
  return (
    <div className={cn("space-y-2.5", className)} aria-busy="true" aria-live="polite">
      {Array.from({ length: rows }).map((_, i) => (
        <Skeleton key={i} className="h-11 w-full rounded-lg" />
      ))}
    </div>
  );
}

/**
 * A single full-height skeleton sized to the chart it stands in for, so the
 * panel doesn't jolt when loaded data swaps a row-list skeleton for a chart.
 */
export function ChartSkeleton({ height = 300, className }: { height?: number; className?: string }) {
  return (
    <Skeleton
      className={cn("w-full rounded-xl", className)}
      style={{ height }}
      aria-busy="true"
      aria-live="polite"
    />
  );
}
