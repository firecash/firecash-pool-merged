import type { Metadata } from "next";
import Link from "next/link";
import { ArrowRight } from "lucide-react";
import { PageShell } from "@/components/site/page-shell";
import { JsonLd } from "@/components/json-ld";
import { articleByline, breadcrumbLd } from "@/lib/structured-data";
import { APP_URL } from "@/lib/mining";
import {
  fetchPoolStats,
  parseHashRate,
  formatBlockCount,
  formatRelativeTime,
  type MiningPoolStats,
} from "@/lib/pool-stats";

const SITE_URL = process.env.NEXT_PUBLIC_SITE_URL ?? "https://katpool.com";
const TITLE = "Kat Pool Live Stats — Kaspa Pool Hashrate, Blocks & Fee";
const DESCRIPTION =
  "Live Kat Pool statistics for Kaspa (KAS) mining: current pool hashrate, total blocks found, pool fee, reward scheme and minimum payout — fetched directly from the Kat Pool API.";

export const metadata: Metadata = {
  title: TITLE,
  description: DESCRIPTION,
  alternates: { canonical: "/stats" },
  openGraph: { title: TITLE, description: DESCRIPTION, url: "/stats", type: "website" },
  twitter: { card: "summary_large_image", title: TITLE, description: DESCRIPTION },
};

// Revalidate the server-rendered snapshot every 60s so the page stays fresh for
// crawlers and AI engines without hammering the pool API.
export const revalidate = 60;

function datasetLd(stats: MiningPoolStats | null, updatedIso: string) {
  return {
    "@context": "https://schema.org",
    "@type": "Dataset",
    name: "Kat Pool live Kaspa mining statistics",
    description: DESCRIPTION,
    url: `${SITE_URL}/stats`,
    inLanguage: "en",
    creator: { "@id": `${SITE_URL}/#organization` },
    dateModified: updatedIso,
    ...(stats
      ? {
          variableMeasured: [
            { "@type": "PropertyValue", name: "Pool hashrate", value: stats.current_hashRate },
            { "@type": "PropertyValue", name: "Total blocks found", value: stats.totalBlocksCount },
            { "@type": "PropertyValue", name: "Pool fee (%)", value: stats.poolFee },
            { "@type": "PropertyValue", name: "Minimum payout (KAS)", value: stats.minPay },
          ],
        }
      : {}),
  };
}

function articleLd() {
  return {
    "@context": "https://schema.org",
    "@type": "Article",
    headline: "Kat Pool Live Kaspa Mining Stats",
    description: DESCRIPTION,
    inLanguage: "en",
    mainEntityOfPage: `${SITE_URL}/stats`,
    ...articleByline(),
  };
}

function Stat({ label, value, sub }: { label: string; value: string; sub?: string }) {
  return (
    <div className="glass-panel rounded-2xl p-5">
      <div className="text-xs uppercase tracking-wide text-muted-foreground">{label}</div>
      <div className="mt-1.5 text-2xl font-semibold text-foreground">{value}</div>
      {sub ? <div className="mt-0.5 text-xs text-muted-foreground">{sub}</div> : null}
    </div>
  );
}

export default async function StatsPage() {
  let stats: MiningPoolStats | null = null;
  try {
    stats = await fetchPoolStats();
  } catch {
    // Render the explanatory shell even if the API is briefly unavailable.
  }

  const updated = new Date();
  const hr = stats ? parseHashRate(stats.current_hashRate) : null;

  return (
    <PageShell>
      <JsonLd
        data={[
          articleLd(),
          datasetLd(stats, updated.toISOString()),
          breadcrumbLd([{ name: "Live stats", path: "/stats" }]),
        ]}
      />

      <nav aria-label="Breadcrumb" className="mb-6 text-xs text-muted-foreground">
        <Link href="/" className="transition hover:text-foreground">
          Home
        </Link>
        <span className="px-1.5">/</span>
        <span className="text-foreground">Live stats</span>
      </nav>

      <article className="space-y-10">
        <header className="space-y-4">
          <h1 className="text-3xl font-semibold tracking-tight sm:text-4xl">
            Kat Pool <span className="text-grad">live stats</span>
          </h1>
          <p className="text-lg leading-relaxed text-muted-foreground">
            These are live Kat Pool statistics for Kaspa (KAS) mining — current pool hashrate, total
            blocks found, the pool fee and reward scheme, and the minimum payout — read directly from
            the Kat Pool API. The snapshot below refreshes about once a minute.
          </p>
          <p className="text-xs text-muted-foreground">
            Updated{" "}
            <time dateTime={updated.toISOString()}>
              {updated.toLocaleString("en-US", {
                dateStyle: "medium",
                timeStyle: "short",
                timeZone: "UTC",
              })}{" "}
              UTC
            </time>
          </p>
        </header>

        {stats ? (
          <>
            <section className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
              <Stat
                label="Pool hashrate"
                value={hr ? `${hr.value} ${hr.unit}` : stats.current_hashRate}
              />
              <Stat label="Blocks found" value={formatBlockCount(stats.totalBlocksCount)} />
              <Stat
                label="Pool fee"
                value={`${stats.poolFee}%`}
                sub={stats.feeType ? stats.feeType.toUpperCase() : undefined}
              />
              <Stat label="Minimum payout" value={`${stats.minPay} KAS`} />
              <Stat
                label="Last block"
                value={stats.lastblocktime ? formatRelativeTime(stats.lastblocktime) : "—"}
                sub={stats.lastblock ? `#${stats.lastblock}` : undefined}
              />
              <Stat label="Coin" value={stats.coin_mined || "KAS"} sub="kHeavyHash" />
            </section>

            <p className="text-sm text-muted-foreground">
              Kat Pool pays on a transparent <strong>PROP (proportional)</strong> reward scheme and
              rebates part of the fee as NACHO, for an effective fee as low as 0%. For per-miner
              hashrate, payout history and block details, open the{" "}
              <a href={APP_URL} className="text-primary underline-offset-2 hover:underline">
                live dashboard
              </a>
              .
            </p>
          </>
        ) : (
          <section className="glass-panel rounded-2xl p-6 text-muted-foreground">
            Live stats are momentarily unavailable. Open the{" "}
            <a href={APP_URL} className="text-primary underline-offset-2 hover:underline">
              live dashboard
            </a>{" "}
            for real-time pool and per-miner statistics.
          </section>
        )}

        <section className="flex flex-col gap-3 rounded-2xl border border-primary/20 bg-primary/[0.06] p-6 sm:flex-row sm:items-center sm:justify-between">
          <div>
            <h2 className="text-lg font-semibold tracking-tight">Start mining Kaspa</h2>
            <p className="text-sm text-muted-foreground">
              Read the guide, or compare Kat Pool with other pools.
            </p>
          </div>
          <div className="flex gap-3">
            <Link
              href="/compare"
              className="inline-flex items-center gap-1.5 rounded-full border border-border px-4 py-2 text-sm text-muted-foreground transition hover:border-primary/40 hover:text-foreground"
            >
              Compare pools
            </Link>
            <Link
              href="/kaspa-mining-pool"
              className="inline-flex items-center gap-1.5 rounded-full bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:brightness-110"
            >
              Mining guide
              <ArrowRight className="size-3.5" />
            </Link>
          </div>
        </section>
      </article>
    </PageShell>
  );
}
