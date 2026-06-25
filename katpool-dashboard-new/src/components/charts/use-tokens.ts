"use client";

import { useEffect, useState } from "react";
import { useTheme } from "next-themes";

export interface ChartTokens {
  series: string[];
  text: string;
  muted: string;
  grid: string;
  tooltipBg: string;
  border: string;
  card: string;
}

function read(name: string, fallback: string): string {
  if (typeof window === "undefined") return fallback;
  const value = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return value || fallback;
}

function snapshot(): ChartTokens {
  return {
    series: [
      read("--chart-1", "#49eacb"),
      read("--chart-2", "#f2c14e"),
      read("--chart-3", "#a78bfa"),
      read("--chart-4", "#60a5fa"),
      read("--chart-5", "#fb7185"),
    ],
    text: read("--foreground", "#e8eef2"),
    muted: read("--muted-foreground", "#8a97a5"),
    grid: read("--grid", "rgba(255,255,255,0.08)"),
    tooltipBg: read("--popover", "#1c2230"),
    border: read("--border", "rgba(255,255,255,0.1)"),
    card: read("--card", "#1c2230"),
  };
}

/** Resolve the live chart palette from CSS tokens, refreshed on theme change. */
export function useChartTokens(): ChartTokens {
  const { resolvedTheme } = useTheme();
  const [tokens, setTokens] = useState<ChartTokens>(() => snapshot());

  useEffect(() => {
    // Defer one frame so the `.dark` class is applied before we read vars.
    const id = requestAnimationFrame(() => setTokens(snapshot()));
    return () => cancelAnimationFrame(id);
  }, [resolvedTheme]);

  return tokens;
}
