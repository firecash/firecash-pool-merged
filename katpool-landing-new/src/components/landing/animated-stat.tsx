"use client";

import { useEffect, useRef, useState } from "react";
import { motion, useMotionValueEvent, useSpring } from "framer-motion";
import { cn } from "@/lib/utils";

export function AnimatedNumber({
  value,
  format,
  className,
}: {
  value: number;
  format: (n: number) => string;
  className?: string;
}) {
  const spring = useSpring(value, { stiffness: 85, damping: 22, mass: 0.6 });
  const textRef = useRef<HTMLSpanElement>(null);
  const prev = useRef(value);
  const [bump, setBump] = useState(false);

  useEffect(() => {
    spring.set(value);
    if (value > prev.current) {
      setBump(true);
      const t = window.setTimeout(() => setBump(false), 450);
      prev.current = value;
      return () => window.clearTimeout(t);
    }
    prev.current = value;
  }, [value, spring]);

  useMotionValueEvent(spring, "change", (v) => {
    if (textRef.current) textRef.current.textContent = format(v);
  });

  return (
    <motion.span
      className={cn("metric inline-block tabular-nums", className)}
      animate={bump ? { scale: [1, 1.04, 1] } : { scale: 1 }}
      transition={{ duration: 0.45, ease: [0.22, 1, 0.36, 1] }}
    >
      <span ref={textRef}>{format(value)}</span>
    </motion.span>
  );
}

export function AnimatedHashrate({ raw, className }: { raw: string; className?: string }) {
  const m = raw.match(/^([\d.]+)(.*)$/);
  const num = m ? Number.parseFloat(m[1]) : 0;
  const unit = m?.[2] ?? "TH/s";

  return (
    <span className={cn("inline-flex items-baseline gap-0.5", className)}>
      <AnimatedNumber
        value={num}
        format={(n) => (Number.isFinite(n) ? n.toFixed(2) : "—")}
        className="text-2xl font-semibold sm:text-3xl"
      />
      <span className="text-base font-medium text-muted-foreground sm:text-lg">{unit}</span>
    </span>
  );
}
