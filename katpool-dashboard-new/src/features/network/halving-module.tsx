"use client";

import { useEffect, useMemo, useState } from "react";
import { ArrowDown, TrendingDown } from "lucide-react";
import { Panel } from "@/components/dashboard/panel";
import { ErrorState, LoadingRows } from "@/components/dashboard/states";
import { useNetworkContext } from "@/lib/api/hooks";
import { ECOSYSTEM } from "@/lib/ecosystem";
import { ExtLink } from "@/components/ext-link";
import { formatCompact, formatHashrate } from "@/lib/format";

interface Remaining {
  d: number;
  h: number;
  m: number;
  s: number;
  done: boolean;
}

/** Break a future unix timestamp (seconds) into d/h/m/s remaining. */
function remainingFrom(targetSec: number): Remaining {
  const total = Math.max(0, Math.floor(targetSec - Date.now() / 1000));
  return {
    d: Math.floor(total / 86400),
    h: Math.floor((total % 86400) / 3600),
    m: Math.floor((total % 3600) / 60),
    s: total % 60,
    done: total <= 0,
  };
}

function Segment({ value, label }: { value: number; label: string }) {
  return (
    <div className="flex flex-col items-center">
      <div className="grid min-w-[3.25rem] place-items-center rounded-lg border border-border bg-elevated px-2 py-2.5 elevation-1">
        <span className="text-2xl font-semibold leading-none metric tabular-nums">
          {String(value).padStart(2, "0")}
        </span>
      </div>
      <span className="mt-1.5 text-[0.625rem] font-medium uppercase tracking-[0.1em] text-muted-foreground">
        {label}
      </span>
    </div>
  );
}

/**
 * A designed emission module: a live countdown to Kaspa's next chromatic
 * reward reduction, the reward step, and circulating-supply progress —
 * replacing a row of undifferentiated stat boxes.
 */
export function HalvingModule() {
  const { data, isLoading, isError, refetch } = useNetworkContext();
  const [now, setNow] = useState<number | null>(null);

  // Tick once per second, client-only (avoids SSR/CSR time mismatch).
  useEffect(() => {
    setNow(Date.now());
    const id = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(id);
  }, []);

  const remaining = useMemo<Remaining | null>(() => {
    if (now == null || !data?.next_halving) return null;
    return remainingFrom(data.next_halving.timestamp);
  }, [now, data?.next_halving]);

  const supplyPct = useMemo(() => {
    if (!data || !data.max_supply_kas) return null;
    return Math.min(100, (data.circulating_supply_kas / data.max_supply_kas) * 100);
  }, [data]);

  const rewardDrop = useMemo(() => {
    if (!data?.next_halving || !data.block_reward_kas) return null;
    return ((data.block_reward_kas - data.next_halving.reward_kas) / data.block_reward_kas) * 100;
  }, [data]);

  return (
    <Panel eyebrow="Emission" title="Next chromatic halving" index={1}>
      {isError ? (
        <ErrorState onRetry={() => void refetch()} />
      ) : isLoading || !data ? (
        <LoadingRows rows={4} />
      ) : (
        <div className="space-y-5">
          {/* Countdown */}
          {data.next_halving ? (
            <div className="flex items-end gap-3">
              {remaining && !remaining.done ? (
                <div className="flex items-start gap-2">
                  <Segment value={remaining.d} label="days" />
                  <Segment value={remaining.h} label="hrs" />
                  <Segment value={remaining.m} label="min" />
                  <Segment value={remaining.s} label="sec" />
                </div>
              ) : remaining?.done ? (
                <span className="text-lg font-semibold text-primary">Halving imminent…</span>
              ) : (
                <div className="h-[4.25rem] w-52 animate-pulse rounded-lg bg-muted/50" />
              )}
            </div>
          ) : (
            <p className="text-sm text-muted-foreground">Halving schedule unavailable.</p>
          )}

          {/* Reward step */}
          {data.next_halving ? (
            <div className="flex items-center justify-between rounded-xl border border-border bg-muted/20 px-4 py-3">
              <div>
                <p className="text-[0.6875rem] uppercase tracking-[0.1em] text-muted-foreground">
                  Block reward
                </p>
                <div className="mt-1 flex items-center gap-2 text-base font-semibold metric">
                  <span>{data.block_reward_kas.toFixed(2)}</span>
                  <ArrowDown className="size-3.5 text-warning" />
                  <span className="text-primary">{data.next_halving.reward_kas.toFixed(2)}</span>
                  <span className="text-xs font-normal text-muted-foreground">
                    <ExtLink href={ECOSYSTEM.kaspa}>KAS</ExtLink>
                  </span>
                </div>
              </div>
              {rewardDrop != null ? (
                <span className="inline-flex items-center gap-1 rounded-full bg-warning/15 px-2 py-1 text-xs font-medium text-warning tnum">
                  <TrendingDown className="size-3" />
                  {rewardDrop.toFixed(1)}%
                </span>
              ) : null}
            </div>
          ) : null}

          {/* Supply progress */}
          {supplyPct != null ? (
            <div>
              <div className="flex items-center justify-between text-xs">
                <span className="text-muted-foreground">Circulating supply</span>
                <span className="font-medium tnum">{supplyPct.toFixed(1)}%</span>
              </div>
              <div className="mt-2 h-2 overflow-hidden rounded-full bg-muted">
                <div
                  className="h-full rounded-full bg-gradient-to-r from-primary/70 to-primary"
                  style={{ width: `${supplyPct}%` }}
                />
              </div>
              <div className="mt-1.5 flex items-center justify-between text-[0.6875rem] text-muted-foreground tnum">
                <span>
                  {formatCompact(data.circulating_supply_kas)}{" "}
                  <ExtLink href={ECOSYSTEM.kaspa}>KAS</ExtLink>
                </span>
                <span>
                  {formatCompact(data.max_supply_kas)} <ExtLink href={ECOSYSTEM.kaspa}>KAS</ExtLink> max
                </span>
              </div>
            </div>
          ) : null}

          {/* Context */}
          <div className="flex items-center justify-between border-t border-border pt-3 text-xs">
            <span className="text-muted-foreground">Network hashrate</span>
            <span className="font-medium tnum">{formatHashrate(data.network_hashrate_hs)}</span>
          </div>
        </div>
      )}
    </Panel>
  );
}
