"use client";

import { useLiveRelative } from "@/hooks/use-live-relative";

/** Relative timestamp that ticks every second between API polls. */
export function LiveRelative({
  at,
  className,
  title,
}: {
  at: string | number | Date | null | undefined;
  className?: string;
  title?: string;
}) {
  const label = useLiveRelative(at);
  return (
    <span className={className} title={title}>
      {label}
    </span>
  );
}
