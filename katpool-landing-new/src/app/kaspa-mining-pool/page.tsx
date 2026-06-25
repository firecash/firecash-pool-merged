import type { Metadata } from "next";
import Link from "next/link";
import { ArrowRight } from "lucide-react";
import { PageShell } from "@/components/site/page-shell";
import { JsonLd } from "@/components/json-ld";
import {
  FAQ_ITEMS,
  faqPageLd,
  howToLd,
  articleByline,
  breadcrumbLd,
  CONTENT_UPDATED,
  MAINTAINER,
} from "@/lib/structured-data";
import { miningConfig, APP_URL, GITHUB_URL } from "@/lib/mining";

const SITE_URL = process.env.NEXT_PUBLIC_SITE_URL ?? "https://katpool.com";
const TITLE = "Kaspa Mining Pool — How to Mine KAS on Kat Pool";
const DESCRIPTION =
  "A complete guide to mining Kaspa (KAS) on Kat Pool: fees, the PROP reward scheme, NACHO rebates, stratum host and ports, supported ASICs, and how to start mining in two minutes.";

export const metadata: Metadata = {
  title: TITLE,
  description: DESCRIPTION,
  alternates: { canonical: "/kaspa-mining-pool" },
  openGraph: {
    title: TITLE,
    description: DESCRIPTION,
    url: "/kaspa-mining-pool",
    type: "article",
  },
  twitter: { card: "summary_large_image", title: TITLE, description: DESCRIPTION },
};

function articleLd() {
  return {
    "@context": "https://schema.org",
    "@type": "TechArticle",
    headline: "How to Mine Kaspa (KAS) on Kat Pool",
    description: DESCRIPTION,
    about: "Kaspa cryptocurrency mining",
    inLanguage: "en",
    mainEntityOfPage: `${SITE_URL}/kaspa-mining-pool`,
    ...articleByline(),
  };
}

export default function KaspaMiningPoolGuide() {
  const cfg = miningConfig();
  const ports = cfg.ports.map((p) => p.port).join(", ");

  return (
    <PageShell>
      <JsonLd
        data={[
          articleLd(),
          howToLd({ host: cfg.host, ports, recommendedPort: cfg.recommended.port }),
          faqPageLd(),
          breadcrumbLd([{ name: "Kaspa mining pool guide", path: "/kaspa-mining-pool" }]),
        ]}
      />

      <nav aria-label="Breadcrumb" className="mb-6 text-xs text-muted-foreground">
        <Link href="/" className="transition hover:text-foreground">
          Home
        </Link>
        <span className="px-1.5">/</span>
        <span className="text-foreground">Kaspa mining pool guide</span>
      </nav>

      <article className="space-y-10">
        <header className="space-y-4">
          <h1 className="text-3xl font-semibold tracking-tight sm:text-4xl">
            Kaspa mining pool: how to mine <span className="text-grad">KAS</span> on Kat Pool
          </h1>
          <p className="text-lg leading-relaxed text-muted-foreground">
            Kat Pool is a fully open-source Kaspa (KAS) mining pool. It runs a globally distributed
            anycast stratum, pays miners on a transparent <strong>PROP (proportional)</strong> scheme,
            and rebates part of the pool fee as <strong>NACHO</strong> — bringing the effective fee
            down to as low as 0%. This guide covers fees, rewards, hardware, and exactly how to point
            a rig at the pool.
          </p>
          <p className="text-xs text-muted-foreground">
            By {MAINTAINER} · Updated{" "}
            <time dateTime={CONTENT_UPDATED}>
              {new Date(CONTENT_UPDATED).toLocaleDateString("en-US", {
                year: "numeric",
                month: "long",
                day: "numeric",
              })}
            </time>
          </p>
        </header>

        <section className="space-y-3">
          <h2 className="text-xl font-semibold tracking-tight">Why mine Kaspa on Kat Pool?</h2>
          <p className="text-muted-foreground">
            Mine on Kat Pool because it is the open-source, lowest-effective-fee way to mine Kaspa:
            a 0.75% topline fee drops to about 0.5% after NACHO rebates and to 0% for NACHO and NFT
            holders, payouts use a transparent PROP scheme, and a single anycast host routes every rig
            to the nearest of seven regions. The four points below summarize what sets it apart.
          </p>
          <ul className="space-y-2 text-muted-foreground">
            <li className="flex gap-2">
              <span className="mt-2 size-1.5 shrink-0 rounded-full bg-primary" />
              <span>
                <strong className="text-foreground">100% open source.</strong> The entire pool stack
                is public on{" "}
                <a href={GITHUB_URL} target="_blank" rel="noopener noreferrer" className="text-primary underline-offset-2 hover:underline">
                  GitHub
                </a>{" "}
                — audit the code and verify payouts.
              </span>
            </li>
            <li className="flex gap-2">
              <span className="mt-2 size-1.5 shrink-0 rounded-full bg-primary" />
              <span>
                <strong className="text-foreground">Lowest effective fees.</strong> A 0.75% topline
                fee, 33% rebated in NACHO (~0.5% effective) — and 0% for NACHO/NFT holders.
              </span>
            </li>
            <li className="flex gap-2">
              <span className="mt-2 size-1.5 shrink-0 rounded-full bg-primary" />
              <span>
                <strong className="text-foreground">Global anycast stratum.</strong> One host routes
                each rig to the nearest of seven regions for minimal stale shares.
              </span>
            </li>
            <li className="flex gap-2">
              <span className="mt-2 size-1.5 shrink-0 rounded-full bg-primary" />
              <span>
                <strong className="text-foreground">Dual rewards.</strong> Earn KAS plus NACHO
                (KRC-20) rebates at every payout cycle.
              </span>
            </li>
          </ul>
        </section>

        <section className="space-y-3">
          <h2 className="text-xl font-semibold tracking-tight">
            What does Kat Pool cost, and how are rewards paid?
          </h2>
          <p className="text-muted-foreground">
            Kat Pool uses a <strong>PROP (proportional)</strong> reward scheme: each matured block&apos;s
            reward is shared across miners by their contribution over a recent-share window. The topline
            fee is <strong>{cfg.toplineFeePercent}%</strong>, of which 33% is rebated to every miner in
            NACHO — an effective fee of about <strong>{cfg.displayFeePercent}%</strong>. Holders of
            NACHO tokens (100M+), Nacho Kats NFTs, or KATCLAIM NFTs receive 100% of the fee back as
            NACHO, for a <strong>0% effective fee</strong>. The minimum payout is{" "}
            <strong>{cfg.minPayoutKas} KAS</strong>.
          </p>
        </section>

        <section className="space-y-3">
          <h2 className="text-xl font-semibold tracking-tight">
            How do I start mining Kaspa on Kat Pool?
          </h2>
          <p className="text-muted-foreground">
            To start mining Kaspa on Kat Pool, point your ASIC at the anycast stratum host, set your
            Kaspa wallet address as the worker username, and start hashing — there is no account or
            signup. The three steps below take about two minutes.
          </p>
          <ol className="list-decimal space-y-2 pl-5 text-muted-foreground">
            <li>
              Point your ASIC at <code className="rounded bg-background/60 px-1.5 py-0.5 font-mono text-sm text-foreground">stratum+tcp://{cfg.host}</code>{" "}
              on a port from {ports} (vardiff on every port; {cfg.recommended.port} is a solid default).
            </li>
            <li>
              Set the worker username to your Kaspa wallet address (<code className="rounded bg-background/60 px-1.5 py-0.5 font-mono text-sm text-foreground">kaspa:…</code>).
              No account or signup is required.
            </li>
            <li>
              Start hashing — anycast routes you to the nearest region and your address appears on the
              dashboard within a minute.
            </li>
          </ol>
          <p className="text-muted-foreground">
            Prefer a guided setup? The dashboard has a step-by-step{" "}
            <a href={`${APP_URL}/start`} className="text-primary underline-offset-2 hover:underline">
              Start Mining
            </a>{" "}
            page with per-miner examples.
          </p>
        </section>

        <section className="space-y-3">
          <h2 className="text-xl font-semibold tracking-tight">
            Which mining hardware does Kat Pool support?
          </h2>
          <p className="text-muted-foreground">
            Kat Pool supports every Kaspa kHeavyHash ASIC, including the IceRiver KS series (KS0, KS1,
            KS2, KS3, KS5/KS5L/KS5M) and Bitmain KS/KA series (KS3, KS5, KA3). Any miner that speaks
            standard Stratum over the listed ports will connect. See the{" "}
            <Link href="/kaspa-asic-miners" className="text-primary underline-offset-2 hover:underline">
              Kaspa ASIC miner guide
            </Link>{" "}
            for per-model port and setup notes, or estimate returns with the{" "}
            <Link href="/kaspa-mining-calculator" className="text-primary underline-offset-2 hover:underline">
              Kaspa mining calculator
            </Link>
            .
          </p>
        </section>

        <section className="space-y-5">
          <h2 className="text-xl font-semibold tracking-tight">Frequently asked questions</h2>
          <dl className="space-y-5">
            {FAQ_ITEMS.map((item) => (
              <div key={item.question} className="glass-panel rounded-2xl p-5">
                <dt className="font-medium text-foreground">{item.question}</dt>
                <dd className="mt-2 text-sm leading-relaxed text-muted-foreground">{item.answer}</dd>
              </div>
            ))}
          </dl>
        </section>

        <section className="flex flex-col gap-3 rounded-2xl border border-primary/20 bg-primary/[0.06] p-6 sm:flex-row sm:items-center sm:justify-between">
          <div>
            <h2 className="text-lg font-semibold tracking-tight">Ready to mine Kaspa?</h2>
            <p className="text-sm text-muted-foreground">
              Open the live dashboard or compare Kat Pool with HumPool.
            </p>
          </div>
          <div className="flex gap-3">
            <Link
              href="/vs/humpool"
              className="inline-flex items-center gap-1.5 rounded-full border border-border px-4 py-2 text-sm text-muted-foreground transition hover:border-primary/40 hover:text-foreground"
            >
              Compare
            </Link>
            <a
              href={APP_URL}
              className="inline-flex items-center gap-1.5 rounded-full bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:brightness-110"
            >
              Open dashboard
              <ArrowRight className="size-3.5" />
            </a>
          </div>
        </section>
      </article>
    </PageShell>
  );
}
