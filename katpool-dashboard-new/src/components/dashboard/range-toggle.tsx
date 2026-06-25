"use client";

import { RANGE_KEYS, RANGE_LABELS, type RangeKey } from "@/lib/range";
import { cn } from "@/lib/utils";

interface RangeToggleProps {
  value: RangeKey;
  onChange: (key: RangeKey) => void;
  /** Restrict to a subset of presets. */
  options?: RangeKey[];
}

/** A compact segmented control for time-range selection. */
export function RangeToggle({ value, onChange, options = RANGE_KEYS }: RangeToggleProps) {
  return (
    <div
      role="radiogroup"
      aria-label="Time range"
      className="inline-flex items-center gap-0.5 rounded-lg border border-border bg-muted/40 p-0.5"
    >
      {options.map((key) => (
        <button
          key={key}
          type="button"
          role="radio"
          aria-checked={value === key}
          onClick={() => onChange(key)}
          className={cn(
            "rounded-md px-2.5 py-1 text-xs font-medium transition-colors tnum",
            value === key
              ? "bg-background text-foreground shadow-sm"
              : "text-muted-foreground hover:text-foreground",
          )}
        >
          {RANGE_LABELS[key]}
        </button>
      ))}
    </div>
  );
}
