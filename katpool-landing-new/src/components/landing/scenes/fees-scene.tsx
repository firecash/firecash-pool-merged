"use client";

import { motion } from "framer-motion";
import { Coins, Crown, Sparkles } from "lucide-react";
import type { MiningPoolStats } from "@/lib/pool-stats";
import { miningConfig } from "@/lib/mining";
import { ECOSYSTEM } from "@/lib/ecosystem";
import { ExtLink } from "../ext-link";

export function FeesScene({ stats }: { stats: MiningPoolStats | null }) {
  const { toplineFeePercent, minPayoutKas } = miningConfig();
  const listedFee = stats?.poolFee ?? 0.5;

  return (
    <div className="mx-auto grid w-full max-w-6xl gap-6 lg:grid-cols-3">
      <motion.div
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        className="glass-panel rounded-3xl p-6 lg:col-span-1"
      >
        <div className="mb-4 inline-flex items-center gap-2 rounded-full bg-primary/10 px-3 py-1 text-xs text-primary">
          <Coins className="size-3.5" />
          Transparent economics
        </div>
        <h2 className="text-2xl font-semibold tracking-tight sm:text-3xl">
          PROP payouts.
          <br />
          <span className="text-grad">Real rebates.</span>
        </h2>
        <p className="mt-4 text-sm leading-relaxed text-muted-foreground">
          {stats?.feeType ?? "PROP"} scheme with a {toplineFeePercent}% topline allocation. A portion
          returns as <ExtLink href={ECOSYSTEM.nacho}>NACHO</ExtLink> at each{" "}
          <ExtLink href={ECOSYSTEM.krc20}>KRC-20</ExtLink> payout cycle - lowering your effective cost
          without hidden spreads.
        </p>
        <dl className="mt-6 space-y-3 border-t border-border/60 pt-5 text-sm">
          <div className="flex justify-between">
            <dt className="text-muted-foreground">Effective fee</dt>
            <dd className="metric font-semibold">{listedFee}%</dd>
          </div>
          <div className="flex justify-between">
            <dt className="text-muted-foreground">Topline fee</dt>
            <dd className="metric font-semibold">{toplineFeePercent}%</dd>
          </div>
          <div className="flex justify-between">
            <dt className="text-muted-foreground">Minimum payout</dt>
            <dd className="metric font-semibold">
              {minPayoutKas} <ExtLink href={ECOSYSTEM.kaspa}>KAS</ExtLink>
            </dd>
          </div>
        </dl>
      </motion.div>

      <motion.div
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ delay: 0.08 }}
        className="glass-panel rounded-3xl p-6"
      >
        <div className="mb-3 flex items-center gap-2 text-sm font-medium">
          <Sparkles className="size-4 text-secondary" />
          Standard miners
        </div>
        <p className="text-3xl font-semibold tracking-tight">
          33%{" "}
          <span className="text-base font-normal text-muted-foreground">
            fee back in <ExtLink href={ECOSYSTEM.nacho}>NACHO</ExtLink>
          </span>
        </p>
        <p className="mt-3 text-sm text-muted-foreground">
          Every matured block accrues a <ExtLink href={ECOSYSTEM.nacho}>NACHO</ExtLink> rebate equal to
          one-third of your fee share. Converted at market rate on the{" "}
          <ExtLink href={ECOSYSTEM.krc20}>KRC-20</ExtLink> payout cycle.
        </p>
        <div className="mt-5 rounded-2xl border border-border/50 bg-background/40 p-4 font-mono text-xs text-muted-foreground">
          net_kas ≈ gross × (1 − {toplineFeePercent / 100})
          <br />
          + nacho_rebate at cycle
        </div>
      </motion.div>

      <motion.div
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ delay: 0.16 }}
        className="glass-panel rounded-3xl p-6 ring-1 ring-secondary/20"
      >
        <div className="mb-3 flex items-center gap-2 text-sm font-medium">
          <Crown className="size-4 text-secondary" />
          Elite tier
        </div>
        <p className="text-3xl font-semibold tracking-tight">
          100%{" "}
          <span className="text-base font-normal text-muted-foreground">
            fee back in <ExtLink href={ECOSYSTEM.nacho}>NACHO</ExtLink>
          </span>
        </p>
        <p className="mt-3 text-sm text-muted-foreground">
          Qualify with any one path - evaluated once per matured block:
        </p>
        <ul className="mt-4 space-y-2">
          <li className="flex items-start gap-2 text-sm text-muted-foreground">
            <span className="mt-1.5 size-1.5 shrink-0 rounded-full bg-secondary" />
            <ExtLink href={ECOSYSTEM.nacho}>NACHO</ExtLink> KRC-721 collection holder
          </li>
          <li className="flex items-start gap-2 text-sm text-muted-foreground">
            <span className="mt-1.5 size-1.5 shrink-0 rounded-full bg-secondary" />
            KATCLAIM KRC-721 collection holder
          </li>
          <li className="flex items-start gap-2 text-sm text-muted-foreground">
            <span className="mt-1.5 size-1.5 shrink-0 rounded-full bg-secondary" />
            100,000,000+ <ExtLink href={ECOSYSTEM.nacho}>NACHO</ExtLink> tokens (
            <ExtLink href={ECOSYSTEM.krc20}>KRC-20</ExtLink>)
          </li>
        </ul>
      </motion.div>
    </div>
  );
}
