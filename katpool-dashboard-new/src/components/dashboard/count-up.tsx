"use client";

import { useEffect, useRef, useState } from "react";

interface CountUpProps {
  value: number;
  /** Maps the animated numeric value to a display string. */
  format: (v: number) => string;
  durationMs?: number;
  className?: string;
}

/** Animate a number from its previous value to the next on change (rAF). */
export function CountUp({ value, format, durationMs = 700, className }: CountUpProps) {
  const [display, setDisplay] = useState(value);
  const fromRef = useRef(value);
  const rafRef = useRef<number | null>(null);

  useEffect(() => {
    const reduce =
      typeof window !== "undefined" &&
      window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (reduce || !Number.isFinite(value)) {
      setDisplay(value);
      fromRef.current = value;
      return;
    }
    const from = fromRef.current;
    const start = performance.now();
    const tick = (now: number) => {
      const p = Math.min((now - start) / durationMs, 1);
      const eased = 1 - Math.pow(1 - p, 3);
      setDisplay(from + (value - from) * eased);
      if (p < 1) {
        rafRef.current = requestAnimationFrame(tick);
      } else {
        fromRef.current = value;
      }
    };
    rafRef.current = requestAnimationFrame(tick);
    return () => {
      if (rafRef.current != null) cancelAnimationFrame(rafRef.current);
      fromRef.current = value;
    };
  }, [value, durationMs]);

  return <span className={className}>{format(display)}</span>;
}
