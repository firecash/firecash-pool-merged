"use client";

import { useNow } from "@/hooks/use-now";
import { formatRelative } from "@/lib/format";

/** Relative time string that ticks every second between API polls. */
export function useLiveRelative(iso: string | number | Date | null | undefined): string {
  const now = useNow();
  if (iso == null) return "—";
  return formatRelative(iso, now);
}
