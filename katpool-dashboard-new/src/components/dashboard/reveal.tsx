"use client";

import { type ReactNode } from "react";
import { motion, useReducedMotion } from "framer-motion";
import { cn } from "@/lib/utils";

interface RevealProps {
  children: ReactNode;
  className?: string;
  /** Stagger position; adds a small per-item delay. */
  index?: number;
  /** Initial vertical offset in px. */
  y?: number;
  /** Explicit delay (overrides index-based delay when set). */
  delay?: number;
}

/**
 * Reveals content as it scrolls into view — once, choreographed, and
 * fully disabled under `prefers-reduced-motion`. The signature easing is a
 * gentle "out-expo"-style curve for an engineered, Apple-like settle.
 */
export function Reveal({ children, className, index = 0, y = 14, delay }: RevealProps) {
  const reduce = useReducedMotion();
  if (reduce) return <div className={className}>{children}</div>;

  // Mount-based (not scroll-gated): content is always rendered visible — a
  // dashboard must never withhold data behind an IntersectionObserver — while
  // still settling in with a choreographed, staggered entrance.
  return (
    <motion.div
      className={cn(className)}
      initial={{ opacity: 0, y }}
      animate={{ opacity: 1, y: 0 }}
      transition={{
        duration: 0.55,
        ease: [0.22, 1, 0.36, 1],
        delay: delay ?? Math.min(index * 0.06, 0.36),
      }}
    >
      {children}
    </motion.div>
  );
}
