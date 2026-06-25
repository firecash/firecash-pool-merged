import type { Metadata } from "next";
import Link from "next/link";
import { ArrowRight } from "lucide-react";
import { PageShell } from "@/components/site/page-shell";
import { JsonLd } from "@/components/json-ld";
import { MiningCalculator } from "@/components/calculator/mining-calculator";
import { articleByline, breadcrumbLd, CONTENT_UPDATED, MAINTAINER } from "@/lib/structured-data";
import { miningConfig, APP_URL } from "@/lib/mining";
import { fetchKaspaNetwork, type KaspaNetwork } from "@/lib/kaspa-network";

const SITE_URL = process.env.NEXT_PUBLIC_SITE_URL ?? "https://katpool.com";
const TITLE = "Kaspa Mining Calculator — Estimate KAS Earnings | Kat Pool";
const DESCRIPTION =
  "Free Kaspa (KAS) mining calculator. Enter your ASIC hashrate, power draw and electricity cost to estimate daily and monthly KAS earnings and profit, using live network hashrate, block reward and KAS price.";

export const metadata: Metadata = {
  title: TITLE,
  description: DESCRIPTION,
  alternates: { canonical: "/kaspa-mining-calculator" },
  openGraph: { title: TITLE, description: DESCRIPTION, url: "/kaspa-mining-calculator", type: "website" },
  twitter: { card: "summary_large_image", title: TITLE, description: DESCRIPTION },
};

export const revalidate = 300;

const FAQ = [
  {
    question: "How is Kaspa mining profitability calculated?",
    answer:
      "Your share of the network is your hashrate divided by the total Kaspa network hashrate. Multiply that share by the daily network emission (block reward × 10 blocks per second × 86,400 seconds), subtract the pool fee, then multiply by the KAS price for a USD estimate. Subtract your electricity cost (power in kW × 24 hours × cost per kWh) to get net profit.",
  },
  {
    question: "Is this Kaspa mining calculator accurate?",
    answer:
      "It uses live network hashrate, block reward and KAS price from the official Kaspa API, so the estimate reflects current conditions. Actual earnings vary with network difficulty, price movements, pool luck and uptime, so treat the result as an estimate, not a guarantee.",
  },
  {
    question: "What hashrate do Kaspa ASICs produce?",
    answer:
      "Kaspa kHeavyHash ASICs range from entry-level IceRiver KS0/KS1 units up to multi-terahash models like the IceRiver KS5 series and Bitmain KS5. Check your miner's published specification for its exact TH/s, then enter that value above.",
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

function appLd() {
  return {
    "@context": "https://schema.org",
    "@type": "WebApplication",
    name: "Kaspa Mining Calculator",
    url: `${SITE_URL}/kaspa-mining-calculator`,
    applicationCategory: "FinanceApplication",
    operatingSystem: "Any",
    description: DESCRIPTION,
    offers: { "@type": "Offer", price: "0", priceCurrency: "USD" },
    ...articleByline(),
  };
}

export default async function CalculatorPage() {
  const cfg = miningConfig();
  let net: KaspaNetwork | null = null;
  try {
    net = await fetchKaspaNetwork();
  } catch {
    // Calculator renders with a graceful "data unavailable" state.
  }

  const priceFmt = net
    ? new Intl.NumberFormat("en-US", { style: "currency", currency: "USD", maximumFractionDigits: 6 }).format(
        net.priceUsd,
      )
    : null;

  return (
    <PageShell>
      <JsonLd
        data={[
          appLd(),
          faqLd(),
          breadcrumbLd([{ name: "Kaspa mining calculator", path: "/kaspa-mining-calculator" }]),
        ]}
      />

      <nav aria-label="Breadcrumb" className="mb-6 text-xs text-muted-foreground">
        <Link href="/" className="transition hover:text-foreground">
          Home
        </Link>
        <span className="px-1.5">/</span>
        <span className="text-foreground">Kaspa mining calculator</span>
      </nav>

      <article className="space-y-10">
        <header className="space-y-4">
          <h1 className="text-3xl font-semibold tracking-tight sm:text-4xl">
            Kaspa mining <span className="text-grad">calculator</span>
          </h1>
          <p className="text-lg leading-relaxed text-muted-foreground">
            Estimate your Kaspa (KAS) mining earnings: enter your ASIC hashrate, power draw and
            electricity cost, and this calculator returns daily and monthly KAS and profit using the
            live network hashrate, block reward and KAS price. Defaults reflect Kat Pool&apos;s{" "}
            {cfg.toplineFeePercent}% fee.
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
            {priceFmt ? ` · Live KAS price ${priceFmt}` : ""}
          </p>
        </header>

        <section className="glass-panel rounded-2xl p-5 sm:p-6">
          <MiningCalculator net={net} poolFeePercent={cfg.toplineFeePercent} available={net !== null} />
        </section>

        <section className="space-y-3">
          <h2 className="text-xl font-semibold tracking-tight">How is Kaspa mining profitability calculated?</h2>
          <p className="text-muted-foreground">
            Your earnings equal your share of the network times what the network pays out. Your share
            is your hashrate divided by the total Kaspa network hashrate. The network emits{" "}
            <strong>block reward × 10 blocks per second × 86,400 seconds</strong> of KAS each day; your
            share of that, minus the pool fee, is your gross KAS. Multiply by the KAS price for a USD
            figure, then subtract electricity (power in kW × 24 × cost per kWh) for net profit. On Kat
            Pool, NACHO rebates cut the effective fee below the {cfg.toplineFeePercent}% topline — as
            low as 0% for NACHO and NFT holders.
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
            <h2 className="text-lg font-semibold tracking-tight">Ready to mine?</h2>
            <p className="text-sm text-muted-foreground">
              Point your miner at Kat Pool — see the guide or supported hardware.
            </p>
          </div>
          <div className="flex gap-3">
            <Link
              href="/kaspa-asic-miners"
              className="inline-flex items-center gap-1.5 rounded-full border border-border px-4 py-2 text-sm text-muted-foreground transition hover:border-primary/40 hover:text-foreground"
            >
              ASIC guide
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
