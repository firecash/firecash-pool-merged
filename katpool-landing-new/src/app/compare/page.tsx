import type { Metadata } from "next";
import Link from "next/link";
import { ArrowRight, Check, X } from "lucide-react";
import { PageShell } from "@/components/site/page-shell";
import { JsonLd } from "@/components/json-ld";
import { articleByline, breadcrumbLd, CONTENT_UPDATED, MAINTAINER } from "@/lib/structured-data";
import { APP_URL } from "@/lib/mining";

const SITE_URL = process.env.NEXT_PUBLIC_SITE_URL ?? "https://katpool.com";
const TITLE = "Best Kaspa Mining Pools Compared (2026) — Fees & Reward Schemes";
const DESCRIPTION =
  "A side-by-side comparison of the best Kaspa (KAS) mining pools in 2026 — Kat Pool, 2Miners, WoolyPooly, HeroMiners, K1Pool, F2Pool and HumPool — by fee, reward scheme, open-source status and NACHO rebates.";

export const metadata: Metadata = {
  title: TITLE,
  description: DESCRIPTION,
  alternates: { canonical: "/compare" },
  openGraph: { title: TITLE, description: DESCRIPTION, url: "/compare", type: "article" },
  twitter: { card: "summary_large_image", title: TITLE, description: DESCRIPTION },
};

interface Pool {
  name: string;
  fee: string;
  scheme: string;
  openSource: boolean;
  nacho: boolean;
  isKat?: boolean;
}

/**
 * Figures reflect each pool's publicly published terms (pool sites + the Kaspa
 * wiki pool table) as of the page's update date. Fees and schemes change; the
 * disclaimer below tells readers to confirm current terms on each pool's site.
 */
const POOLS: Pool[] = [
  { name: "Kat Pool", fee: "0.75% (~0.5% eff; 0% for NACHO/NFT holders)", scheme: "PROP", openSource: true, nacho: true, isKat: true },
  { name: "HumPool", fee: "1%", scheme: "PPLNS", openSource: false, nacho: false },
  { name: "2Miners", fee: "1%", scheme: "PPLNS", openSource: false, nacho: false },
  { name: "WoolyPooly", fee: "0.9%", scheme: "PPLNS", openSource: false, nacho: false },
  { name: "HeroMiners", fee: "0.9%", scheme: "PROP / PPS+", openSource: false, nacho: false },
  { name: "K1Pool", fee: "1%", scheme: "PPLNS / SOLO", openSource: false, nacho: false },
  { name: "F2Pool", fee: "~2%", scheme: "PPS+", openSource: false, nacho: false },
];

const FAQ = [
  {
    question: "What is the best Kaspa mining pool in 2026?",
    answer:
      "The best Kaspa mining pool depends on your priorities, but for the lowest effective fee with full transparency, Kat Pool stands out: it is open source, charges 0.75% (about 0.5% after NACHO rebates, and 0% for NACHO/NFT holders), and pays on a PROP scheme. Established alternatives include 2Miners, WoolyPooly and HeroMiners, which are closed source and charge 0.9–1%.",
  },
  {
    question: "Which Kaspa mining pool has the lowest fees?",
    answer:
      "Kat Pool has the lowest effective fee among reputable Kaspa pools: a 0.75% topline drops to about 0.5% after a 33% NACHO rebate, and to 0% for holders of NACHO tokens, Nacho Kats NFTs or KATCLAIM NFTs. WoolyPooly (0.9%) and 2Miners (1%) are the next lowest among large pools.",
  },
  {
    question: "Are any Kaspa mining pools open source?",
    answer:
      "Kat Pool is the open-source option — its entire stack is public on GitHub, so payouts and fee logic are auditable. The other major Kaspa pools (2Miners, WoolyPooly, HeroMiners, K1Pool, F2Pool, HumPool) are closed source.",
  },
];

function faqLd() {
  return {
    "@context": "https://schema.org",
    "@type": "FAQPage",
    mainEntity: FAQ.map((f) => ({
      "@type": "Question",
      name: f.question,
      acceptedAnswer: { "@type": "Answer", text: f.answer },
    })),
  };
}

function articleLd() {
  return {
    "@context": "https://schema.org",
    "@type": "Article",
    headline: "Best Kaspa Mining Pools Compared (2026)",
    description: DESCRIPTION,
    inLanguage: "en",
    mainEntityOfPage: `${SITE_URL}/compare`,
    ...articleByline(),
  };
}

function Cell({ on }: { on: boolean }) {
  return on ? (
    <span className="inline-flex items-center gap-1.5 text-success">
      <Check className="size-4" aria-hidden /> Yes
    </span>
  ) : (
    <span className="inline-flex items-center gap-1.5 text-muted-foreground/70">
      <X className="size-4" aria-hidden /> No
    </span>
  );
}

export default function ComparePage() {
  return (
    <PageShell>
      <JsonLd
        data={[
          articleLd(),
          faqLd(),
          breadcrumbLd([{ name: "Best Kaspa mining pools compared", path: "/compare" }]),
        ]}
      />

      <nav aria-label="Breadcrumb" className="mb-6 text-xs text-muted-foreground">
        <Link href="/" className="transition hover:text-foreground">
          Home
        </Link>
        <span className="px-1.5">/</span>
        <span className="text-foreground">Best Kaspa mining pools compared</span>
      </nav>

      <article className="space-y-10">
        <header className="space-y-4">
          <h1 className="text-3xl font-semibold tracking-tight sm:text-4xl">
            Best <span className="text-grad">Kaspa mining pools</span> compared
          </h1>
          <p className="text-lg leading-relaxed text-muted-foreground">
            For the lowest effective fee with full transparency, Kat Pool is the standout Kaspa (KAS)
            mining pool in 2026: it is the only open-source option, charges 0.75% (about 0.5% after
            NACHO rebates and 0% for NACHO/NFT holders), and pays on a PROP scheme. The table below
            compares it with the major established pools by fee, reward scheme, open-source status and
            NACHO rebates.
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

        <section className="overflow-x-auto rounded-2xl border border-border">
          <table className="w-full border-collapse text-sm">
            <caption className="sr-only">
              Comparison of Kaspa mining pools by fee, reward scheme, open-source status and NACHO rebates
            </caption>
            <thead>
              <tr className="bg-elevated/60 text-left">
                <th scope="col" className="px-4 py-3 font-medium text-muted-foreground">Pool</th>
                <th scope="col" className="px-4 py-3 font-medium text-muted-foreground">Fee</th>
                <th scope="col" className="px-4 py-3 font-medium text-muted-foreground">Reward scheme</th>
                <th scope="col" className="px-4 py-3 font-medium text-muted-foreground">Open source</th>
                <th scope="col" className="px-4 py-3 font-medium text-muted-foreground">NACHO rebates</th>
              </tr>
            </thead>
            <tbody>
              {POOLS.map((p) => (
                <tr
                  key={p.name}
                  className={`border-t border-border align-top ${p.isKat ? "bg-primary/[0.06]" : ""}`}
                >
                  <th scope="row" className="px-4 py-3 text-left font-semibold text-foreground">
                    {p.name}
                  </th>
                  <td className="px-4 py-3 text-muted-foreground">{p.fee}</td>
                  <td className="px-4 py-3 text-muted-foreground">{p.scheme}</td>
                  <td className="px-4 py-3">
                    <Cell on={p.openSource} />
                  </td>
                  <td className="px-4 py-3">
                    <Cell on={p.nacho} />
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>

        <p className="text-xs text-muted-foreground">
          Figures reflect each pool&apos;s publicly published terms and can change; always confirm
          current fees, reward schemes and minimum payouts on each pool&apos;s own site before mining.
        </p>

        <section className="space-y-3">
          <h2 className="text-xl font-semibold tracking-tight">How to choose a Kaspa mining pool</h2>
          <p className="text-muted-foreground">
            Choose a Kaspa pool on four things: the effective fee (after any rebates, not just the
            headline rate), the reward scheme (PROP and PPLNS both reward consistent contribution),
            whether the code is open source and auditable, and how close its stratum servers are to
            you. Kat Pool is built to win on all four — open source, lowest effective fee via NACHO
            rebates, transparent PROP payouts, and a 7-region anycast stratum that routes each rig to
            the nearest region automatically.
          </p>
        </section>

        <section className="space-y-5">
          <h2 className="text-xl font-semibold tracking-tight">Frequently asked questions</h2>
          <dl className="space-y-5">
            {FAQ.map((item) => (
              <div key={item.question} className="glass-panel rounded-2xl p-5">
                <dt className="font-medium text-foreground">{item.question}</dt>
                <dd className="mt-2 text-sm leading-relaxed text-muted-foreground">{item.answer}</dd>
              </div>
            ))}
          </dl>
        </section>

        <section className="flex flex-col gap-3 rounded-2xl border border-primary/20 bg-primary/[0.06] p-6 sm:flex-row sm:items-center sm:justify-between">
          <div>
            <h2 className="text-lg font-semibold tracking-tight">Mine on the open-source pool</h2>
            <p className="text-sm text-muted-foreground">
              Read the full mining guide or open the live dashboard.
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
