import type { Metadata } from "next";
import Link from "next/link";
import { ArrowRight, Check, Minus } from "lucide-react";
import { PageShell } from "@/components/site/page-shell";
import { JsonLd } from "@/components/json-ld";
import { articleByline, breadcrumbLd, CONTENT_UPDATED, MAINTAINER } from "@/lib/structured-data";
import { miningConfig, APP_URL } from "@/lib/mining";

const SITE_URL = process.env.NEXT_PUBLIC_SITE_URL ?? "https://katpool.com";
const TITLE = "Kat Pool vs HumPool — Kaspa Mining Pool Comparison";
const DESCRIPTION =
  "Comparing Kat Pool and HumPool for Kaspa (KAS) mining: fees, reward scheme, NACHO rebates, open source, minimum payout, and global stratum. The open-source, lower-fee HumPool alternative.";

export const metadata: Metadata = {
  title: TITLE,
  description: DESCRIPTION,
  alternates: { canonical: "/vs/humpool" },
  openGraph: { title: TITLE, description: DESCRIPTION, url: "/vs/humpool", type: "article" },
  twitter: { card: "summary_large_image", title: TITLE, description: DESCRIPTION },
};

interface Row {
  label: string;
  kat: string;
  katWin?: boolean;
  hum: string;
}

const ROWS: Row[] = [
  { label: "Open source", kat: "Yes — full stack on GitHub", katWin: true, hum: "No (closed source)" },
  { label: "Pool fee", kat: "0.75% topline · ~0.5% effective · 0% for NACHO/NFT holders", katWin: true, hum: "1%" },
  { label: "Reward scheme", kat: "PROP (proportional, recent-share window)", hum: "PPLNS" },
  { label: "NACHO rebates", kat: "Yes — KAS + NACHO every cycle", katWin: true, hum: "No" },
  { label: "Minimum payout", kat: "10 KAS (anonymous)", katWin: true, hum: "15 KAS (logged-in) / 30 KAS (anonymous)" },
  { label: "Global stratum", kat: "7-region anycast (one host)", katWin: true, hum: "Regional endpoints" },
  { label: "Supported hardware", kat: "All kHeavyHash ASICs (IceRiver KS, Bitmain KS/KA)", hum: "All kHeavyHash ASICs" },
];

const FAQ = [
  {
    question: "Is Kat Pool a good alternative to HumPool?",
    answer:
      "Yes. Kat Pool is open source, charges a lower effective fee than HumPool's 1%, supports the same Kaspa ASICs, and uses a comparable proportional (PROP) reward model over a recent-share window — so migrating from HumPool's PPLNS is straightforward.",
  },
  {
    question: "How do I switch from HumPool to Kat Pool?",
    answer:
      "Point your miner at kas.katpool.com on any port from 1111 to 8888, set the username to your Kaspa wallet address, and start hashing. No account is required, and your address appears on the Kat Pool dashboard within a minute.",
  },
  {
    question: "What does Kat Pool offer that HumPool does not?",
    answer:
      "An open-source, auditable stack; NACHO (KRC-20) fee rebates that can take the effective fee to 0% for NACHO token and NFT holders; a global anycast stratum across seven regions; and a lower 10 KAS minimum payout.",
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
    headline: "Kat Pool vs HumPool: Kaspa Mining Pool Comparison",
    description: DESCRIPTION,
    inLanguage: "en",
    mainEntityOfPage: `${SITE_URL}/vs/humpool`,
    ...articleByline(),
  };
}

export default function VsHumpool() {
  const cfg = miningConfig();

  return (
    <PageShell>
      <JsonLd
        data={[
          articleLd(),
          faqLd(),
          breadcrumbLd([{ name: "Kat Pool vs HumPool", path: "/vs/humpool" }]),
        ]}
      />

      <nav aria-label="Breadcrumb" className="mb-6 text-xs text-muted-foreground">
        <Link href="/" className="transition hover:text-foreground">
          Home
        </Link>
        <span className="px-1.5">/</span>
        <span className="text-foreground">Kat Pool vs HumPool</span>
      </nav>

      <article className="space-y-10">
        <header className="space-y-4">
          <h1 className="text-3xl font-semibold tracking-tight sm:text-4xl">
            Kat Pool vs <span className="text-grad">HumPool</span>
          </h1>
          <p className="text-lg leading-relaxed text-muted-foreground">
            Kat Pool is a strong HumPool alternative for Kaspa (KAS) mining: it is open source,
            charges a lower effective fee (about 0.5%, or 0% for NACHO/NFT holders versus HumPool&apos;s
            1%), rebates NACHO on top of KAS, and runs a 7-region anycast stratum. The table below
            compares both pools point by point, and switching takes about two minutes.
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

        <section className="overflow-hidden rounded-2xl border border-border">
          <table className="w-full border-collapse text-sm">
            <caption className="sr-only">Feature comparison of Kat Pool and HumPool</caption>
            <thead>
              <tr className="bg-elevated/60 text-left">
                <th scope="col" className="px-4 py-3 font-medium text-muted-foreground">Feature</th>
                <th scope="col" className="px-4 py-3 font-semibold text-foreground">Kat Pool</th>
                <th scope="col" className="px-4 py-3 font-medium text-muted-foreground">HumPool</th>
              </tr>
            </thead>
            <tbody>
              {ROWS.map((row) => (
                <tr key={row.label} className="border-t border-border align-top">
                  <th scope="row" className="px-4 py-3 text-left font-medium text-foreground">
                    {row.label}
                  </th>
                  <td className="px-4 py-3 text-muted-foreground">
                    <span className="inline-flex items-start gap-1.5">
                      {row.katWin ? (
                        <Check className="mt-0.5 size-4 shrink-0 text-success" aria-hidden />
                      ) : (
                        <Minus className="mt-0.5 size-4 shrink-0 text-muted-foreground/60" aria-hidden />
                      )}
                      <span>{row.kat}</span>
                    </span>
                  </td>
                  <td className="px-4 py-3 text-muted-foreground">{row.hum}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>

        <p className="text-xs text-muted-foreground">
          HumPool figures reflect its publicly published terms and may change; always confirm current
          terms on each pool&apos;s own site.
        </p>

        <section className="space-y-3">
          <h2 className="text-xl font-semibold tracking-tight">How do I switch from HumPool to Kat Pool?</h2>
          <p className="text-muted-foreground">
            Repoint your miner to{" "}
            <code className="rounded bg-background/60 px-1.5 py-0.5 font-mono text-sm text-foreground">
              stratum+tcp://{cfg.host}
            </code>{" "}
            on any port from {cfg.ports[0].port} to {cfg.ports[cfg.ports.length - 1].port}, set the
            username to your Kaspa wallet address, and save. No account, no signup — your hashrate
            shows up on the Kat Pool dashboard within a minute.
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
            <h2 className="text-lg font-semibold tracking-tight">Make the switch</h2>
            <p className="text-sm text-muted-foreground">
              Open the dashboard, or read the full mining guide first.
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
              href={`${APP_URL}/start`}
              className="inline-flex items-center gap-1.5 rounded-full bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:brightness-110"
            >
              Start mining
              <ArrowRight className="size-3.5" />
            </a>
          </div>
        </section>
      </article>
    </PageShell>
  );
}
