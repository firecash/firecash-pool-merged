"use client";

import { motion } from "framer-motion";
import { miningConfig } from "@/lib/mining";
import { EDGE_REGIONS } from "@/lib/edge-regions";
import { EdgeGlobe } from "../edge-globe";

export function EdgeScene() {
  const { host } = miningConfig();

  return (
    <div className="mx-auto w-full max-w-6xl lg:grid lg:grid-cols-[0.95fr_1.05fr] lg:items-center lg:gap-10">
      {/* Globe - compact on phones, full size on desktop */}
      <motion.div
        initial={{ opacity: 0, scale: 0.94 }}
        animate={{ opacity: 1, scale: 1 }}
        transition={{ delay: 0.08, duration: 0.7, ease: [0.22, 1, 0.36, 1] }}
        className="mx-auto mb-4 flex w-full max-w-[11.5rem] shrink-0 flex-col items-center sm:mb-5 sm:max-w-[17rem] lg:order-2 lg:mb-0 lg:max-w-none"
      >
        <EdgeGlobe className="max-w-[min(100%,540px)]" />

        <div className="mt-2 flex max-w-full flex-wrap items-center justify-center gap-x-3 gap-y-1 px-1 text-[10px] text-muted-foreground sm:gap-x-4 sm:text-[11px]">
          <span className="inline-flex items-center gap-1.5">
            <span className="size-1.5 shrink-0 rounded-full bg-[#49eacb] sm:size-2" />
            Origin · Germany
          </span>
          <span className="inline-flex items-center gap-1.5">
            <span className="size-1.5 shrink-0 rounded-full bg-[#70c7ba] sm:size-2" />
            Edge · 7 regions
          </span>
          <span className="max-w-full truncate font-mono text-foreground/80">{host}</span>
        </div>
      </motion.div>

      <div className="min-w-0 lg:order-1">
        <motion.p
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          className="text-center text-xs uppercase tracking-[0.2em] text-primary lg:text-left"
        >
          Global infrastructure
        </motion.p>
        <motion.h2
          initial={{ opacity: 0, y: 16 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.06 }}
          className="mt-2 text-center text-2xl font-semibold tracking-tight sm:mt-3 sm:text-3xl lg:text-left lg:text-[2.75rem] lg:leading-[1.08]"
        >
          Seven edge regions.
          <br />
          <span className="text-grad">One stratum URL.</span>
        </motion.h2>
        <motion.p
          initial={{ opacity: 0, y: 12 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.12 }}
          className="mx-auto mt-3 max-w-md text-center text-sm leading-relaxed text-muted-foreground sm:mt-4 sm:text-base lg:mx-0 lg:text-left"
        >
          Edge anycast routes hashrate to the nearest healthy edge. Use{" "}
          <span className="font-mono text-foreground">{host}</span> for automatic routing, or
          pin a regional host below.
        </motion.p>

        <motion.div
          initial={{ opacity: 0, y: 16 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.18 }}
          className="mt-4 grid grid-cols-2 gap-1.5 sm:mt-6 sm:grid-cols-1 sm:gap-0 sm:space-y-1.5 lg:grid-cols-2 lg:gap-2 lg:space-y-0"
        >
          {EDGE_REGIONS.map((r, i) => (
            <motion.div
              key={r.host}
              initial={{ opacity: 0, x: -12 }}
              animate={{ opacity: 1, x: 0 }}
              transition={{ delay: 0.22 + i * 0.04 }}
              className="group flex items-center gap-2 rounded-lg border border-border/40 bg-card/20 px-2 py-2 transition hover:border-border/60 hover:bg-card/35 sm:gap-3 sm:rounded-xl sm:border-transparent sm:bg-transparent sm:px-3 sm:py-2.5 sm:hover:border-border/50"
            >
              <span className="flex size-7 shrink-0 items-center justify-center rounded-md border border-primary/20 bg-primary/8 font-mono text-[9px] font-medium uppercase tracking-wide text-primary sm:size-8 sm:rounded-lg sm:text-[10px]">
                {r.fly}
              </span>
              <div className="min-w-0 flex-1">
                <p className="truncate text-xs font-medium text-foreground sm:text-sm">{r.label}</p>
                <p className="hidden truncate font-mono text-[11px] text-muted-foreground sm:block">
                  {r.host}
                </p>
              </div>
              <span className="live-dot hidden size-1.5 shrink-0 rounded-full bg-success opacity-0 transition group-hover:opacity-100 sm:block" />
            </motion.div>
          ))}
        </motion.div>
      </div>
    </div>
  );
}
