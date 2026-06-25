"use client";

import { useEffect, useRef, useState } from "react";

/**
 * Returns the subset of `ids` that newly appeared since the previous update,
 * held for a brief highlight window so freshly-arrived rows can flash. The
 * first population never flashes (so an initial load doesn't light up wholesale).
 */
export function useHighlightNew<T extends string | number>(
  ids: T[],
  durationMs = 1600,
): Set<T> {
  const seen = useRef<Set<T> | null>(null);
  const idsRef = useRef(ids);
  idsRef.current = ids;
  const [fresh, setFresh] = useState<Set<T>>(() => new Set());

  // A stable primitive key so the effect only re-runs when the id set changes,
  // not on every parent render (callers pass a fresh array each time).
  const key = ids.join("\u0000");

  useEffect(() => {
    const current = idsRef.current;
    const prev = seen.current;
    seen.current = new Set(current);
    if (prev === null) return; // first load: establish baseline, don't flash
    const added = current.filter((id) => !prev.has(id));
    if (added.length === 0) return;
    setFresh(new Set(added));
    const timer = setTimeout(() => setFresh(new Set()), durationMs);
    return () => clearTimeout(timer);
  }, [key, durationMs]);

  return fresh;
}
