"use client";

import { motion } from "framer-motion";
import { ArrowDown, Blocks, Gauge, Percent, Timer } from "lucide-react";
import type { MiningPoolStats } from "@/lib/pool-stats";
import { formatBlockCount, formatRelativeTime } from "@/lib/pool-stats";
import { useNow } from "@/hooks/use-now";
import { APP_URL } from "@/lib/mining";
import { ECOSYSTEM } from "@/lib/ecosystem";
import { ExtLink } from "../ext-link";
import { AnimatedHashrate, AnimatedNumber } from "../animated-stat";

interface HeroSceneProps {
  stats: MiningPoolStats | null;
  syncing: boolean;
  onNext: () => void;
}

function StatCard({
  icon: Icon,
  label,
  delay,
  live,
  children,
  sub,
}: {
  icon: React.ComponentType<{ className?: string }>;
  label: string;
  delay: number;
  live?: boolean;
  children: React.ReactNode;
  sub?: React.ReactNode;
}) {
  return (
    <motion.div
      initial={{ opacity: 0, y: 16 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ delay, duration: 0.5, ease: [0.22, 1, 0.36, 1] }}
      className="glass-panel rounded-2xl p-4 sm:p-5"
    >
      <div className="mb-3 flex items-center justify-between gap-2">
        <div className="flex items-center gap-2 text-xs uppercase tracking-wider text-muted-foreground">
          <Icon className="size-3.5 text-primary" />
          {label}
        </div>
        {live && (
          <span className="inline-flex items-center gap-1 rounded-full bg-success/10 px-1.5 py-0.5 text-[9px] font-medium uppercase tracking-wide text-success">
            <span className="live-dot size-1.5 rounded-full bg-success" />
            Live
          </span>
        )}
      </div>
      <div className="min-h-[2.25rem] sm:min-h-[2.75rem]">{children}</div>
      {sub && <div className="mt-1 text-xs text-muted-foreground">{sub}</div>}
    </motion.div>
  );
}

function LastBlockAge({ iso }: { iso: string }) {
  const now = useNow(1000);
  return (
    <p className="metric text-2xl font-semibold sm:text-3xl">{formatRelativeTime(iso, now)}</p>
  );
}

export function HeroScene({ stats, syncing, onNext }: HeroSceneProps) {
  return (
    <div className="mx-auto grid w-full max-w-6xl gap-8 lg:grid-cols-[1.1fr_0.9fr] lg:items-center">
      <div>
        <motion.div
          initial={{ opacity: 0, y: 12 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.5 }}
          className="mb-5 inline-flex items-center gap-2 rounded-full border border-border bg-card/50 px-3 py-1.5 text-xs text-muted-foreground backdrop-blur-sm"
        >
          <span className="live-dot size-2 rounded-full bg-success" />
          {syncing && !stats ? "Syncing live pool stats…" : "Live mainnet pool · PROP payouts"}
        </motion.div>

        <motion.h1
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.08, duration: 0.55 }}
          className="max-w-xl text-4xl font-semibold leading-[1.05] tracking-tight sm:text-5xl lg:text-6xl"
        >
          Mine <ExtLink href={ECOSYSTEM.kaspa}>Kaspa</ExtLink> at the{" "}
          <span className="text-grad">edge</span>
        </motion.h1>

        <motion.p
          initial={{ opacity: 0, y: 16 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.16, duration: 0.5 }}
          className="mt-5 max-w-lg text-base leading-relaxed text-muted-foreground sm:text-lg"
        >
          Open-source stratum with global anycast, transparent PROP rewards, and{" "}
          <ExtLink href={ECOSYSTEM.nacho}>NACHO</ExtLink> fee rebates. Built for serious{" "}
          <ExtLink href={ECOSYSTEM.kaspa}>Kaspa</ExtLink> hashrate.
        </motion.p>

        <motion.div
          initial={{ opacity: 0, y: 12 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.24, duration: 0.5 }}
          className="mt-8 flex flex-wrap items-center gap-3"
        >
          <a
            href={APP_URL}
            className="inline-flex items-center gap-2 rounded-full bg-primary px-6 py-3 text-sm font-medium text-primary-foreground shadow-[0_0_48px_oklch(0.82_0.15_184/30%)] transition hover:brightness-110"
          >
            Start mining
            <ArrowDown className="size-4 rotate-[-90deg]" />
          </a>
          <button
            type="button"
            onClick={onNext}
            className="inline-flex items-center gap-2 rounded-full border border-border px-5 py-3 text-sm text-muted-foreground transition hover:border-primary/40 hover:text-foreground"
          >
            Explore the pool
            <ArrowDown className="size-4" />
          </button>
        </motion.div>
      </div>

      <div className="grid grid-cols-2 gap-3 sm:gap-4">
        <StatCard icon={Gauge} label="Pool hashrate" live delay={0.2} sub="updates every 10s">
          {stats ? (
            <AnimatedHashrate raw={stats.current_hashRate} />
          ) : (
            <p className="metric text-2xl font-semibold text-muted-foreground sm:text-3xl">—</p>
          )}
        </StatCard>

        <StatCard icon={Blocks} label="Blocks found" live delay={0.28} sub="all-time mainnet">
          {stats ? (
            <AnimatedNumber
              value={stats.totalBlocksCount}
              format={formatBlockCount}
              className="text-2xl font-semibold sm:text-3xl"
            />
          ) : (
            <p className="metric text-2xl font-semibold text-muted-foreground sm:text-3xl">—</p>
          )}
        </StatCard>

        <StatCard
          icon={Percent}
          label="Effective fee"
          delay={0.36}
          sub={
            <>
              PROP · min 10 <ExtLink href={ECOSYSTEM.kaspa}>KAS</ExtLink> payout
            </>
          }
        >
          <p className="metric text-2xl font-semibold sm:text-3xl">
            {stats ? `${stats.poolFee}%` : "0.5%"}
          </p>
        </StatCard>

        <StatCard icon={Timer} label="Last block" live delay={0.44} sub={stats?.feeType ?? "PROP"}>
          {stats?.lastblocktime ? (
            <LastBlockAge iso={stats.lastblocktime} />
          ) : (
            <p className="metric text-2xl font-semibold text-muted-foreground sm:text-3xl">—</p>
          )}
        </StatCard>
      </div>
    </div>
  );
}
