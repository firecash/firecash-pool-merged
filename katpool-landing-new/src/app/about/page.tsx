import type { Metadata } from "next";
import Link from "next/link";
import { ArrowRight, Github, ShieldCheck, Globe2, Coins } from "lucide-react";
import { PageShell } from "@/components/site/page-shell";
import { JsonLd } from "@/components/json-ld";
import {
  organizationLd,
  breadcrumbLd,
  CONTENT_PUBLISHED,
  CONTENT_UPDATED,
  MAINTAINER,
} from "@/lib/structured-data";
import { miningConfig, APP_URL, GITHUB_URL, TWITTER_URL } from "@/lib/mining";
import { XIcon } from "@/components/x-icon";

const SITE_URL = process.env.NEXT_PUBLIC_SITE_URL ?? "https://katpool.com";
const TITLE = "About Kat Pool — Open-Source Kaspa Mining Pool";
const DESCRIPTION =
  "Kat Pool is an open-source Kaspa (KAS) mining pool from the Nacho the Kat ecosystem: transparent PROP payouts, NACHO fee rebates, a global anycast stratum, and fully auditable code.";

export const metadata: Metadata = {
  title: TITLE,
  description: DESCRIPTION,
  alternates: { canonical: "/about" },
  openGraph: { title: TITLE, description: DESCRIPTION, url: "/about", type: "website" },
  twitter: { card: "summary_large_image", title: TITLE, description: DESCRIPTION },
};

function aboutPageLd() {
  return {
    "@context": "https://schema.org",
    "@type": "AboutPage",
    name: TITLE,
    description: DESCRIPTION,
    url: `${SITE_URL}/about`,
    inLanguage: "en",
    datePublished: CONTENT_PUBLISHED,
    dateModified: CONTENT_UPDATED,
    mainEntity: { "@id": `${SITE_URL}/#organization` },
  };
}

export default function AboutPage() {
  const cfg = miningConfig();

  return (
    <PageShell>
      <JsonLd
        data={[organizationLd(), aboutPageLd(), breadcrumbLd([{ name: "About", path: "/about" }])]}
      />

      <nav aria-label="Breadcrumb" className="mb-6 text-xs text-muted-foreground">
        <Link href="/" className="transition hover:text-foreground">
          Home
        </Link>
        <span className="px-1.5">/</span>
        <span className="text-foreground">About</span>
      </nav>

      <article className="space-y-10">
        <header className="space-y-4">
          <h1 className="text-3xl font-semibold tracking-tight sm:text-4xl">
            About <span className="text-grad">Kat Pool</span>
          </h1>
          <p className="text-lg leading-relaxed text-muted-foreground">
            Kat Pool is a fully open-source mining pool for{" "}
            <a href="https://kaspa.org" target="_blank" rel="noopener noreferrer" className="text-primary underline-offset-2 hover:underline">
              Kaspa
            </a>{" "}
            (KAS), built within the Nacho the Kat ($NACHO) ecosystem. Our goal is simple: give Kaspa
            miners a transparent, low-fee, censorship-resistant pool whose code anyone can read, run,
            and verify.
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

        <section className="grid gap-4 sm:grid-cols-3">
          <div className="glass-panel rounded-2xl p-5">
            <ShieldCheck className="size-5 text-primary" />
            <h2 className="mt-3 text-sm font-semibold">Open & auditable</h2>
            <p className="mt-1.5 text-sm text-muted-foreground">
              The entire pool stack is public on GitHub. Payout logic and allocations are verifiable —
              no black boxes.
            </p>
          </div>
          <div className="glass-panel rounded-2xl p-5">
            <Globe2 className="size-5 text-primary" />
            <h2 className="mt-3 text-sm font-semibold">Global by design</h2>
            <p className="mt-1.5 text-sm text-muted-foreground">
              A single anycast stratum host routes every rig to the nearest of seven regions for
              minimal stale shares.
            </p>
          </div>
          <div className="glass-panel rounded-2xl p-5">
            <Coins className="size-5 text-primary" />
            <h2 className="mt-3 text-sm font-semibold">Dual rewards</h2>
            <p className="mt-1.5 text-sm text-muted-foreground">
              Earn KAS plus NACHO (KRC-20) rebates each payout cycle — taking the effective fee as low
              as 0%.
            </p>
          </div>
        </section>

        <section className="space-y-3">
          <h2 className="text-xl font-semibold tracking-tight">What we stand for</h2>
          <p className="text-muted-foreground">
            Kaspa is the fastest open, proof-of-work BlockDAG, and we think its mining infrastructure
            should be just as open. Kat Pool publishes its source, uses a transparent{" "}
            <strong>PROP (proportional)</strong> reward scheme that shares each matured block by
            contribution over a recent-share window, and returns part of the{" "}
            <strong>{cfg.toplineFeePercent}%</strong> fee to miners as NACHO. There are no accounts, no
            KYC, and no custody of your earnings beyond the standard pool payout cycle (minimum{" "}
            <strong>{cfg.minPayoutKas} KAS</strong>).
          </p>
        </section>

        <section className="space-y-3">
          <h2 className="text-xl font-semibold tracking-tight">Contact & community</h2>
          <p className="text-muted-foreground">
            Questions, integrations, or want to run your own node off our code? Reach us here:
          </p>
          <div className="flex flex-wrap gap-3">
            <a
              href={TWITTER_URL}
              target="_blank"
              rel="noopener noreferrer"
              className="inline-flex items-center gap-2 rounded-full border border-border px-4 py-2 text-sm text-muted-foreground transition hover:border-primary/40 hover:text-foreground"
            >
              <XIcon className="size-3.5" /> @Katpool_Mining
            </a>
            <a
              href={GITHUB_URL}
              target="_blank"
              rel="noopener noreferrer"
              className="inline-flex items-center gap-2 rounded-full border border-border px-4 py-2 text-sm text-muted-foreground transition hover:border-primary/40 hover:text-foreground"
            >
              <Github className="size-3.5" /> Source on GitHub
            </a>
          </div>
        </section>

        <section className="flex flex-col gap-3 rounded-2xl border border-primary/20 bg-primary/[0.06] p-6 sm:flex-row sm:items-center sm:justify-between">
          <div>
            <h2 className="text-lg font-semibold tracking-tight">Start mining Kaspa</h2>
            <p className="text-sm text-muted-foreground">
              Read the guide or open the live dashboard.
            </p>
          </div>
          <div className="flex gap-3">
            <Link
              href="/kaspa-mining-pool"
              className="inline-flex items-center gap-1.5 rounded-full border border-border px-4 py-2 text-sm text-muted-foreground transition hover:border-primary/40 hover:text-foreground"
            >
              Mining guide
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
