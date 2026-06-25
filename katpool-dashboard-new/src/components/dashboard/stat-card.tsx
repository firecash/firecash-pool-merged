"use client";

import { type ReactNode } from "react";
import { Info } from "lucide-react";
import { Card } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { Reveal } from "./reveal";
import { CountUp } from "./count-up";
import { DeltaChip } from "./delta-chip";
import { Sparkline } from "./sparkline";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";

interface StatCardProps {
  label: ReactNode;
  /** Numeric value to count up, or null while loading. */
  value: number | null;
  format: (v: number) => string;
  unit?: string;
  icon?: ReactNode;
  delta?: number | null;
  invertDelta?: boolean;
  spark?: number[];
  colorIndex?: number;
  hint?: string;
  loading?: boolean;
  /** Stagger position within a row. */
  index?: number;
  className?: string;
}

/** A premium KPI tile: label, animated value, delta chip, and sparkline. */
export function StatCard({
  label,
  value,
  format,
  unit,
  icon,
  delta,
  invertDelta,
  spark,
  colorIndex = 0,
  hint,
  loading = false,
  index = 0,
  className,
}: StatCardProps) {
  return (
    <Reveal index={index} className="h-full">
      <Card
        className={cn(
          "group relative flex h-full flex-col overflow-hidden p-5 transition-[transform,box-shadow] duration-300 ease-out hover:-translate-y-0.5 hover:elevation-2",
          className,
        )}
      >
        {/* hover sheen */}
        <div className="pointer-events-none absolute -right-8 -top-10 size-28 rounded-full bg-primary/10 opacity-0 blur-2xl transition-opacity duration-300 group-hover:opacity-100" />

        <div className="flex items-start justify-between gap-2">
          <div className="flex items-center gap-2 text-[0.8125rem] font-medium text-muted-foreground">
            {icon ? (
              <span className="grid size-6 place-items-center rounded-md bg-muted/60 text-muted-foreground">
                {icon}
              </span>
            ) : null}
            <span>{label}</span>
            {hint ? (
              <Tooltip>
                <TooltipTrigger asChild>
                  <button
                    aria-label={`About ${label}`}
                    className="text-muted-foreground/60 transition-colors hover:text-foreground"
                  >
                    <Info className="size-3.5" />
                  </button>
                </TooltipTrigger>
                <TooltipContent>{hint}</TooltipContent>
              </Tooltip>
            ) : null}
          </div>
          {delta !== undefined ? <DeltaChip value={delta} invert={invertDelta} /> : null}
        </div>

        <div className="mt-3 flex flex-1 flex-wrap items-end gap-x-1.5">
          {loading || value == null ? (
            <Skeleton className="h-9 w-28" />
          ) : (
            <CountUp
              value={value}
              format={format}
              className="text-[1.75rem] font-semibold leading-none metric"
            />
          )}
          {unit ? <span className="pb-0.5 text-sm text-muted-foreground">{unit}</span> : null}
        </div>

        {spark && spark.length > 1 ? (
          <div className="mt-4 -mb-1" aria-hidden>
            <Sparkline data={spark} colorIndex={colorIndex} />
          </div>
        ) : null}
      </Card>
    </Reveal>
  );
}
