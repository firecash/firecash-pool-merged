"use client";

import { useMemo, type ReactNode } from "react";
import { Reveal } from "@/components/dashboard/reveal";
import { Card } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { CountUp } from "@/components/dashboard/count-up";
import { DeltaChip } from "@/components/dashboard/delta-chip";
import { Sparkline } from "@/components/dashboard/sparkline";
import { usePoolHashrateHistory, usePoolLiveStats, useNetworkContext } from "@/lib/api/hooks";
import { totalBlocksFound } from "@/lib/api/types";
import { ECOSYSTEM } from "@/lib/ecosystem";
import { ExtLink } from "@/components/ext-link";
import { formatCompact, formatHashrate, formatNumber, formatUsd } from "@/lib/format";
import {
  hashrateDeltaPercent,
  LIVE_HASHRATE_WINDOW_SECS,
  referenceBucketIndex,
  sparklineWithLive,
} from "@/lib/hashrate-live";
import { resolveRange } from "@/lib/range";

const BUCKET_SECONDS = { "1m": 60, "5m": 300, "1h": 3600, "1d": 86_400 } as const;
const DELTA_LOOKBACK_SECS = 3_600;

/** A compact metric cell within the hero's hairline-separated grid. */
function HeroStat({
  label,
  value,
  loading,
  extra,
}: {
  label: ReactNode;
  value: string | null;
  loading?: boolean;
  extra?: ReactNode;
}) {
  return (
    <div className="min-w-0 bg-card px-4 py-3.5">
      <p className="text-xs text-muted-foreground">{label}</p>
      <div className="mt-1.5 flex items-center gap-2">
        {loading || value == null ? (
          <Skeleton className="h-6 w-20" />
        ) : (
          <span className="truncate text-base font-semibold metric sm:text-lg">{value}</span>
        )}
        {extra}
      </div>
    </div>
  );
}

/**
 * The Overview headline: a display-scale, live pool-hashrate figure with
 * network-share context, a living sparkline, and a dense supporting grid.
 */
export function OverviewHero() {
  const stats = usePoolLiveStats();
  const network = useNetworkContext();
  const day = useMemo(() => resolveRange("24h"), []);
  const history = usePoolHashrateHistory({ from: day.from, to: day.to, bucket: day.bucket });

  const historyPoints = useMemo(() => history.data?.points ?? [], [history.data?.points]);
  const poolHs = stats.data?.hashrate_hs ?? null;

  const hashSpark = useMemo(
    () => sparklineWithLive(historyPoints, poolHs),
    [historyPoints, poolHs],
  );

  const hashDelta = useMemo(() => {
    if (poolHs == null) return null;
    const bucketSecs = BUCKET_SECONDS[day.bucket];
    const refIdx = referenceBucketIndex(historyPoints.length, bucketSecs, DELTA_LOOKBACK_SECS);
    if (refIdx == null) return null;
    return hashrateDeltaPercent(poolHs, historyPoints[refIdx]?.hashrate_hs);
  }, [poolHs, historyPoints, day.bucket]);

  const netHs = network.data?.network_hashrate_hs ?? 0;
  const netShare = poolHs != null && netHs > 0 ? (poolHs / netHs) * 100 : null;
  const shareLabel =
    netShare == null || netShare > 100
      ? null
      : netShare > 0 && netShare < 0.001
        ? "<0.001%"
        : `${netShare.toFixed(netShare < 1 ? 3 : 2)}%`;

  const loading = stats.isLoading;
  const netLoading = network.isLoading;

  return (
    <Reveal>
      <Card className="relative overflow-hidden">
        <div className="pointer-events-none absolute inset-0 app-aurora opacity-70" />
        <div className="pointer-events-none absolute -right-28 -top-28 size-80 rounded-full bg-primary/10 blur-3xl" />

        <div className="relative grid gap-x-8 gap-y-6 p-6 sm:p-8 lg:grid-cols-12">
          <div className="flex flex-col justify-center lg:col-span-5">
            <div className="flex items-center gap-2 text-[0.6875rem] font-medium uppercase tracking-[0.14em] text-muted-foreground">
              <span className="size-2 rounded-full bg-success live-dot" />
              Pool hashrate · Live
            </div>

            <div className="mt-3 flex flex-wrap items-end gap-x-3 gap-y-2">
              {loading || poolHs == null ? (
                <Skeleton className="h-14 w-56" />
              ) : (
                <CountUp
                  value={poolHs}
                  format={(v) => formatHashrate(v)}
                  className="text-grad text-[2.75rem] font-semibold leading-none metric sm:text-[3.5rem]"
                />
              )}
              {hashDelta != null ? <DeltaChip value={hashDelta} className="mb-1.5" /> : null}
            </div>

            <p className="mt-3 text-sm text-muted-foreground">
              {shareLabel ? (
                <>
                  <span className="font-semibold text-foreground">{shareLabel}</span> of the total{" "}
                  <ExtLink href={ECOSYSTEM.kaspa}>Kaspa</ExtLink> network hashrate ·{" "}
                  {LIVE_HASHRATE_WINDOW_SECS / 60}m window
                </>
              ) : (
                `Estimated from accepted share difficulty over the last ${LIVE_HASHRATE_WINDOW_SECS / 60} minutes`
              )}
            </p>

            {hashSpark.length > 1 ? (
              <div className="mt-5 -mb-1" aria-hidden>
                <Sparkline data={hashSpark} colorIndex={0} height={56} />
              </div>
            ) : null}
          </div>

          <div className="lg:col-span-7">
            <div className="grid grid-cols-2 gap-px overflow-hidden rounded-xl border border-border bg-border sm:grid-cols-3">
              <HeroStat
                label="Active miners"
                value={stats.data ? formatNumber(stats.data.miners_active) : null}
                loading={loading}
              />
              <HeroStat
                label="Active workers"
                value={stats.data ? formatNumber(stats.data.workers_active) : null}
                loading={loading}
              />
              <HeroStat
                label="Accepted shares"
                value={stats.data ? formatCompact(stats.data.accepted_shares) : null}
                loading={loading}
              />
              <HeroStat
                label="Blocks found"
                value={stats.data ? formatCompact(totalBlocksFound(stats.data.blocks)) : null}
                loading={loading}
              />
              <HeroStat
                label="Network hashrate"
                value={network.data ? formatHashrate(network.data.network_hashrate_hs) : null}
                loading={netLoading}
              />
              <HeroStat
                label={
                  <>
                    <ExtLink href={ECOSYSTEM.kaspa}>KAS</ExtLink> price
                  </>
                }
                value={network.data ? formatUsd(network.data.prices.kas_usd) : null}
                loading={netLoading}
                extra={network.data ? <DeltaChip value={network.data.prices.kas_change_24h} /> : null}
              />
            </div>
          </div>
        </div>
      </Card>
    </Reveal>
  );
}
