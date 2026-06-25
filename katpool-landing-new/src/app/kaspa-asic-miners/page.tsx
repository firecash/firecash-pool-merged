import type { Metadata } from "next";
import Link from "next/link";
import { ArrowRight } from "lucide-react";
import { PageShell } from "@/components/site/page-shell";
import { JsonLd } from "@/components/json-ld";
import { articleByline, breadcrumbLd, CONTENT_UPDATED, MAINTAINER } from "@/lib/structured-data";
import { miningConfig, APP_URL } from "@/lib/mining";

const SITE_URL = process.env.NEXT_PUBLIC_SITE_URL ?? "https://katpool.com";
const TITLE = "Kaspa ASIC Miners — Supported Hardware & Setup | Kat Pool";
const DESCRIPTION =
  "Which ASIC miners work for Kaspa (KAS) and how to configure them on Kat Pool: IceRiver KS series, Bitmain KS/KA series and Goldshell KA — stratum host, ports, vardiff and wallet setup.";

export const metadata: Metadata = {
  title: TITLE,
  description: DESCRIPTION,
  alternates: { canonical: "/kaspa-asic-miners" },
  openGraph: { title: TITLE, description: DESCRIPTION, url: "/kaspa-asic-miners", type: "article" },
  twitter: { card: "summary_large_image", title: TITLE, description: DESCRIPTION },
};

interface Vendor {
  name: string;
  models: string;
  note: string;
}

const VENDORS: Vendor[] = [
  {
    name: "IceRiver KS series",
    models: "KS0, KS0 Pro, KS1, KS2, KS3/KS3L/KS3M, KS5/KS5L/KS5M, KS7",
    note: "The most common Kaspa ASIC line, from low-power desktop units to high-terahash datacenter models. All connect over standard Stratum.",
  },
  {
    name: "Bitmain Antminer KS/KA series",
    models: "KS3, KS5, KS5 Pro, KA3",
    note: "Bitmain's kHeavyHash range. Configure the pool in the Antminer web UI under Miner Configuration.",
  },
  {
    name: "Goldshell KA series",
    models: "KA Box, KA Box Pro, E-KA1M",
    note: "Compact, quieter units suited to home setups; configured from the Goldshell dashboard.",
  },
];

const FAQ = [
  {
    question: "Which ASIC miners work with Kaspa?",
    answer:
      "Any ASIC built for Kaspa's kHeavyHash algorithm works, including the IceRiver KS series (KS0 through KS7), Bitmain Antminer KS/KA series (KS3, KS5, KS5 Pro, KA3) and Goldshell KA units (KA Box, KA Box Pro, E-KA1M). GPUs can technically mine Kaspa but are no longer competitive against ASICs.",
  },
  {
    question: "How do I connect my Kaspa ASIC to Kat Pool?",
    answer:
      "In your miner's web interface, set the pool URL to stratum+tcp://kas.katpool.com on a port from 1111 to 8888 (vardiff is enabled on every port), set the worker username to your Kaspa wallet address, leave the password blank or as 'x', and save. No account is required.",
  },
  {
    question: "Which port should I use on Kat Pool?",
    answer:
      "Every port from 1111 to 8888 uses variable difficulty (vardiff), so any port auto-adjusts to your hardware. Port 3333 is a solid default for most ASICs; larger farms can pick a higher-difficulty starting port to reduce share spam.",
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
    "@type": "TechArticle",
    headline: "Kaspa ASIC Miners: Supported Hardware & Setup",
    description: DESCRIPTION,
    about: "Kaspa mining hardware",
    inLanguage: "en",
    mainEntityOfPage: `${SITE_URL}/kaspa-asic-miners`,
    ...articleByline(),
  };
}

export default function AsicMinersPage() {
  const cfg = miningConfig();
  const firstPort = cfg.ports[0].port;
  const lastPort = cfg.ports[cfg.ports.length - 1].port;

  return (
    <PageShell>
      <JsonLd
        data={[
          articleLd(),
          faqLd(),
          breadcrumbLd([{ name: "Kaspa ASIC miners", path: "/kaspa-asic-miners" }]),
        ]}
      />

      <nav aria-label="Breadcrumb" className="mb-6 text-xs text-muted-foreground">
        <Link href="/" className="transition hover:text-foreground">
          Home
        </Link>
        <span className="px-1.5">/</span>
        <span className="text-foreground">Kaspa ASIC miners</span>
      </nav>

      <article className="space-y-10">
        <header className="space-y-4">
          <h1 className="text-3xl font-semibold tracking-tight sm:text-4xl">
            Kaspa <span className="text-grad">ASIC miners</span>: supported hardware &amp; setup
          </h1>
          <p className="text-lg leading-relaxed text-muted-foreground">
            Kaspa is mined with kHeavyHash ASICs, and Kat Pool supports every major model. This guide
            lists the supported IceRiver, Bitmain and Goldshell hardware and shows exactly how to point
            any of them at the pool — set the stratum host, pick a port, use your wallet address as the
            username, and save.
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
          <h2 className="text-xl font-semibold tracking-tight">Which ASICs work with Kaspa?</h2>
          <p className="text-muted-foreground">
            Any miner built for Kaspa&apos;s kHeavyHash algorithm works on Kat Pool. The three main
            vendors are below; check each model&apos;s published specification for its exact hashrate and
            power draw, then estimate returns with the{" "}
            <Link href="/kaspa-mining-calculator" className="text-primary underline-offset-2 hover:underline">
              Kaspa mining calculator
            </Link>
            .
          </p>
          <div className="overflow-x-auto rounded-2xl border border-border">
            <table className="w-full border-collapse text-sm">
              <caption className="sr-only">Supported Kaspa ASIC miners by vendor</caption>
              <thead>
                <tr className="bg-elevated/60 text-left">
                  <th scope="col" className="px-4 py-3 font-medium text-muted-foreground">Vendor / series</th>
                  <th scope="col" className="px-4 py-3 font-medium text-muted-foreground">Models</th>
                  <th scope="col" className="px-4 py-3 font-medium text-muted-foreground">Notes</th>
                </tr>
              </thead>
              <tbody>
                {VENDORS.map((v) => (
                  <tr key={v.name} className="border-t border-border align-top">
                    <th scope="row" className="px-4 py-3 text-left font-semibold text-foreground">
                      {v.name}
                    </th>
                    <td className="px-4 py-3 text-muted-foreground">{v.models}</td>
                    <td className="px-4 py-3 text-muted-foreground">{v.note}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </section>

        <section className="space-y-3">
          <h2 className="text-xl font-semibold tracking-tight">How do I configure my ASIC for Kat Pool?</h2>
          <p className="text-muted-foreground">
            The setup is identical across vendors. In your miner&apos;s web interface, open the pool
            configuration and enter:
          </p>
          <ol className="list-decimal space-y-2 pl-5 text-muted-foreground">
            <li>
              Pool URL:{" "}
              <code className="rounded bg-background/60 px-1.5 py-0.5 font-mono text-sm text-foreground">
                stratum+tcp://{cfg.host}
              </code>{" "}
              on any port from {firstPort} to {lastPort} (vardiff on every port; {cfg.recommended.port}{" "}
              is a solid default).
            </li>
            <li>
              Worker / username: your Kaspa wallet address (
              <code className="rounded bg-background/60 px-1.5 py-0.5 font-mono text-sm text-foreground">kaspa:…</code>
              ). Optionally append a worker name after a dot.
            </li>
            <li>
              Password: leave blank or set to{" "}
              <code className="rounded bg-background/60 px-1.5 py-0.5 font-mono text-sm text-foreground">x</code>. Save
              and reboot if prompted.
            </li>
          </ol>
          <p className="text-muted-foreground">
            For full fee, reward and payout detail, see the{" "}
            <Link href="/kaspa-mining-pool" className="text-primary underline-offset-2 hover:underline">
              Kaspa mining pool guide
            </Link>
            .
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
            <h2 className="text-lg font-semibold tracking-tight">Point your miner at Kat Pool</h2>
            <p className="text-sm text-muted-foreground">Estimate earnings or open the dashboard.</p>
          </div>
          <div className="flex gap-3">
            <Link
              href="/kaspa-mining-calculator"
              className="inline-flex items-center gap-1.5 rounded-full border border-border px-4 py-2 text-sm text-muted-foreground transition hover:border-primary/40 hover:text-foreground"
            >
              Calculator
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
