"use client";

import { CheckCircle2, CircleAlert, CircleSlash, Coins, ExternalLink, Landmark, Loader2, Sparkles } from "lucide-react";
import { Card } from "@/components/ui/card";
import { Panel } from "@/components/dashboard/panel";
import { CopyButton } from "@/components/dashboard/copy-button";
import { EmptyState } from "@/components/dashboard/states";
import {
  usePoolStats,
  useNetworkContext,
  useBlocks,
  useActiveSessions,
} from "@/lib/api/hooks";
import { totalBlocksFound } from "@/lib/api/types";
import { useLiveRelative } from "@/hooks/use-live-relative";
import { formatCompact, formatHashrate, formatKas, formatNumber, formatUsd, sompiToUsd, truncateMiddle } from "@/lib/format";
import { streamAddress } from "@/lib/explorer";
import { ECOSYSTEM } from "@/lib/ecosystem";
import { ExtLink } from "@/components/ext-link";
import { miningConfig } from "@/lib/mining";
import { cn } from "@/lib/utils";
import { PoolRejectsPanel } from "./pool-rejects-panel";

type Health = "ok" | "degraded" | "down" | "pending";

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="bg-card px-4 py-3.5">
      <p className="text-xs text-muted-foreground">{label}</p>
      <p className="mt-1 truncate text-base font-semibold metric">{value}</p>
    </div>
  );
}

const PILL_STYLE: Record<Health, { icon: typeof CheckCircle2; tone: string; spin?: boolean }> = {
  ok: { icon: CheckCircle2, tone: "text-success bg-success/10 border-success/30" },
  degraded: { icon: CircleAlert, tone: "text-warning bg-warning/10 border-warning/30" },
  down: { icon: CircleSlash, tone: "text-destructive bg-destructive/10 border-destructive/30" },
  pending: { icon: Loader2, tone: "text-muted-foreground bg-muted/50 border-border", spin: true },
};

function StatusPill({ state, label, detail }: { state: Health; label: string; detail: string }) {
  const { icon: Icon, tone, spin } = PILL_STYLE[state];
  return (
    <Card className="flex items-center gap-4 p-5">
      <span className={cn("flex size-11 items-center justify-center rounded-xl border", tone)}>
        <Icon className={cn("size-5", spin && "animate-spin")} />
      </span>
      <div>
        <p className="font-medium">{label}</p>
        <p className="text-sm text-muted-foreground">{detail}</p>
      </div>
    </Card>
  );
}

/** Operational status board for the public API and network data sources. */
export function StatusBoard() {
  const stats = usePoolStats();
  const network = useNetworkContext();
  const latest = useBlocks(1);
  const active = useActiveSessions();

  const topBlock = latest.data?.blocks[0];
  const lastBlockAge = useLiveRelative(topBlock?.found_at);
  const treasury = stats.data?.treasury ?? null;
  const treasuryAge = useLiveRelative(treasury?.captured_at);
  const kasUsd = network.data?.prices.kas_usd ?? null;
  const treasuryAddress = miningConfig().treasuryAddress;

  const poolState: Health = stats.isError ? "down" : stats.isLoading ? "pending" : "ok";
  const degraded = network.data?.degraded ?? [];
  const netState: Health = network.isError
    ? "down"
    : network.isLoading
      ? "pending"
      : degraded.length > 0
        ? "degraded"
        : "ok";

  return (
    <div className="space-y-6">
      <div className="grid grid-cols-1 gap-4 md:grid-cols-2" role="status" aria-live="polite">
        <StatusPill
          state={poolState}
          label="Pool API"
          detail={
            poolState === "ok"
              ? "Serving live pool data"
              : poolState === "down"
                ? "Unreachable — retrying"
                : "Connecting…"
          }
        />
        <StatusPill
          state={netState}
          label="Network & price feeds"
          detail={
            netState === "down"
              ? "All upstream sources unavailable"
              : netState === "pending"
                ? "Connecting…"
                : degraded.length > 0
                  ? `Degraded: ${degraded.join(", ")}`
                  : "Kaspa API + CoinGecko healthy"
          }
        />
      </div>

      <Card className="flex flex-col gap-4 p-5 sm:flex-row sm:items-center sm:justify-between">
        <div className="flex items-center gap-3">
          <span className="flex size-11 items-center justify-center rounded-xl border border-success/30 bg-success/10 text-success">
            <span className="size-2.5 rounded-full bg-success live-dot" />
          </span>
          <div>
            <p className="font-medium">Connected now</p>
            <p className="text-sm text-muted-foreground">
              Open stratum sessions right now — live, not a rolling average
            </p>
          </div>
        </div>
        <div className="flex gap-px self-stretch overflow-hidden rounded-xl border border-border bg-border sm:self-auto">
          <div className="flex-1 bg-card px-6 py-3 text-center">
            <p className="text-xs text-muted-foreground">Sessions</p>
            <p className="mt-1 text-2xl font-semibold metric">
              {active.data ? formatNumber(active.data.active_sessions) : "—"}
            </p>
          </div>
          <div className="flex-1 bg-card px-6 py-3 text-center">
            <p className="text-xs text-muted-foreground">Workers</p>
            <p className="mt-1 text-2xl font-semibold metric">
              {active.data ? formatNumber(active.data.active_workers) : "—"}
            </p>
          </div>
        </div>
      </Card>

      <Panel title="Operational metrics" description="Live pool health at a glance">
        <div className="grid grid-cols-2 gap-px overflow-hidden rounded-xl border border-border bg-border sm:grid-cols-3">
          <Metric label="Last block found" value={lastBlockAge} />
          <Metric
            label="Blocks found"
            value={stats.data ? formatCompact(totalBlocksFound(stats.data.blocks)) : "—"}
          />
          <Metric
            label="Active miners"
            value={stats.data ? formatNumber(stats.data.miners_active) : "—"}
          />
          <Metric
            label="Accepted shares"
            value={stats.data ? formatCompact(stats.data.accepted_shares) : "—"}
          />
          <Metric
            label="Confirmed payouts"
            value={stats.data ? formatNumber(stats.data.payouts.confirmed_payouts) : "—"}
          />
          <Metric
            label="Pool hashrate"
            value={stats.data ? formatHashrate(stats.data.hashrate_hs) : "—"}
          />
        </div>
      </Panel>

      <Panel title="Treasury" description="On-chain pool reserves — independently verifiable">
        {treasury ? (
          <div className="grid grid-cols-1 gap-px overflow-hidden rounded-xl border border-border bg-border sm:grid-cols-2">
            <div className="flex flex-col gap-3 bg-card p-5">
              <div>
                <p className="flex items-center gap-2 text-xs text-muted-foreground">
                  <Coins className="size-3.5" /> <ExtLink href={ECOSYSTEM.kaspa}>KAS</ExtLink> balance
                </p>
                <p className="mt-1 text-2xl font-semibold metric">{formatKas(treasury.kas_balance.kas)}</p>
                {kasUsd != null ? (
                  <p className="text-sm text-muted-foreground tnum">
                    {formatUsd(sompiToUsd(treasury.kas_balance.sompi, kasUsd))}
                  </p>
                ) : null}
              </div>
              <p className="flex items-center gap-2 text-sm text-muted-foreground">
                <Sparkles className="size-3.5" /> <ExtLink href={ECOSYSTEM.nacho}>NACHO</ExtLink>
                <span className="font-medium text-foreground tnum">{formatNumber(Number(treasury.nacho_balance))}</span>
              </p>
            </div>
            <div className="flex flex-col justify-between gap-3 bg-card p-5">
              <div>
                <p className="flex items-center gap-2 text-xs text-muted-foreground">
                  <Landmark className="size-3.5" /> Treasury address
                </p>
                {treasuryAddress ? (
                  <div className="mt-1.5 flex items-center gap-1">
                    <span className="truncate font-mono text-sm" title={treasuryAddress}>
                      {truncateMiddle(treasuryAddress)}
                    </span>
                    <CopyButton value={treasuryAddress} label="Copy treasury address" />
                    <a
                      href={streamAddress(treasuryAddress)}
                      target="_blank"
                      rel="noopener noreferrer"
                      aria-label="View treasury on kaspa.stream"
                      className="inline-flex size-7 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                    >
                      <ExternalLink className="size-3.5" />
                    </a>
                  </div>
                ) : (
                  <p className="mt-1.5 text-sm text-muted-foreground">Not configured</p>
                )}
                <a
                  href={treasuryAddress ? streamAddress(treasuryAddress) : "https://kaspa.stream"}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="mt-1 inline-block text-xs text-primary underline-offset-2 hover:underline"
                >
                  Verify on kaspa.stream →
                </a>
              </div>
              <p className="text-xs text-muted-foreground tnum">
                Snapshot {treasuryAge} · DAA {formatCompact(treasury.daa_score)} · blue{" "}
                {formatCompact(treasury.blue_score)}
              </p>
            </div>
          </div>
        ) : (
          <EmptyState
            title="No snapshot yet"
            description="Treasury reserves appear once the pool records its first on-chain snapshot."
          />
        )}
      </Panel>

      <div className="grid grid-cols-1 items-stretch gap-6 lg:grid-cols-2">
        <PoolRejectsPanel />
        <Panel title="About this data" description="How the dashboard sources its numbers">
          <ul className="space-y-2 text-sm text-muted-foreground">
            <li>
              <span className="text-foreground">Pool metrics</span> come from katpool&apos;s public,
              read-only v1 API (hashrate, blocks, payouts, miners, firmware, rejects, geo).
            </li>
            <li>
              <span className="text-foreground">Network context</span> (hashrate, difficulty, supply,
              halving) is sourced from the{" "}
              <ExtLink href={ECOSYSTEM.kaspaApi}>Kaspa</ExtLink> public API.
            </li>
            <li>
              <span className="text-foreground">Prices</span> (
              <ExtLink href={ECOSYSTEM.kaspa}>KAS</ExtLink>,{" "}
              <ExtLink href={ECOSYSTEM.nacho}>NACHO</ExtLink>) come from CoinGecko. All
              on-chain amounts are computed with exact integer math — never floating point.
            </li>
            <li>
              <span className="text-foreground">Geo distribution</span> is aggregate-only (country
              counts, never individual IPs). This product includes GeoLite Data created by MaxMind,
              available from{" "}
              <a
                href="https://www.maxmind.com"
                target="_blank"
                rel="noreferrer"
                className="text-foreground underline underline-offset-2 hover:text-primary"
              >
                maxmind.com
              </a>
              .
            </li>
            <li>Data refreshes automatically; on-chain figures lag the network by confirmation depth.</li>
          </ul>
        </Panel>
      </div>
    </div>
  );
}
