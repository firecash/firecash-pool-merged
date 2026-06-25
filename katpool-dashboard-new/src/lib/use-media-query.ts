"use client";

import { useEffect, useState } from "react";

/**
 * Subscribe to a CSS media query. SSR-safe: returns `false` until mounted, then
 * reflects the live match and updates on change. Use for layout that can't be
 * expressed purely in CSS (e.g. switching an ECharts legend orientation).
 */
export function useMediaQuery(query: string): boolean {
  const [matches, setMatches] = useState(false);

  useEffect(() => {
    const mql = window.matchMedia(query);
    const onChange = () => setMatches(mql.matches);
    onChange();
    mql.addEventListener("change", onChange);
    return () => mql.removeEventListener("change", onChange);
  }, [query]);

  return matches;
}
