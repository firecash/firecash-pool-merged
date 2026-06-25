import Link from "next/link";
import { FAQ_ITEMS } from "@/lib/structured-data";
import { miningConfig, APP_URL } from "@/lib/mining";

/**
 * Server-rendered textual mirror of the homepage value proposition and FAQ.
 *
 * The landing experience is a wheel/scroll-jack SPA that only mounts the active
 * scene, so search-engine and AI crawlers (and screen readers) see almost no
 * homepage prose in the initial HTML. This block ships the same facts the
 * scenes present as real, crawlable, accessible content — visually hidden with
 * `sr-only` so it does not alter the SPA visuals, but fully present in the DOM.
 * It is not cloaking: it restates information the sighted UI already conveys.
 */
export function HomeSeoContent() {
  const cfg = miningConfig();
  const ports = cfg.ports.map((p) => p.port).join(", ");

  return (
    <section className="sr-only" aria-label="About Kat Pool">
      <h1>Kat Pool — open-source Kaspa (KAS) mining pool</h1>
      <p>
        Kat Pool is a fully open-source Kaspa mining pool. It runs a globally distributed anycast
        stratum across seven regions, pays miners on a transparent PROP (proportional) reward scheme,
        and rebates part of the {cfg.toplineFeePercent}% pool fee as NACHO — an effective fee of about{" "}
        {cfg.displayFeePercent}%, and 0% for NACHO token and Nacho Kats / KATCLAIM NFT holders. The
        minimum payout is {cfg.minPayoutKas} KAS.
      </p>

      <h2>How to start mining Kaspa on Kat Pool</h2>
      <p>
        Point your ASIC at stratum+tcp://{cfg.host} on a port from {ports} (vardiff on every port;{" "}
        {cfg.recommended.port} is a solid default), set the worker username to your Kaspa wallet
        address, and start hashing. No account or signup is required. Anycast routes you to the nearest
        region and your address appears on the dashboard within a minute.
      </p>

      <h2>Why mine Kaspa on Kat Pool</h2>
      <ul>
        <li>100% open source — the full stack is public on GitHub and payouts are auditable.</li>
        <li>
          Lowest effective fees — {cfg.toplineFeePercent}% topline, ~{cfg.displayFeePercent}% after
          NACHO rebates, 0% for NACHO/NFT holders.
        </li>
        <li>Global anycast stratum across seven regions for minimal stale shares.</li>
        <li>Dual rewards — earn KAS plus NACHO (KRC-20) rebates each payout cycle.</li>
        <li>Supports all Kaspa kHeavyHash ASICs (IceRiver KS series, Bitmain KS/KA series).</li>
      </ul>

      <h2>Frequently asked questions</h2>
      <dl>
        {FAQ_ITEMS.map((item) => (
          <div key={item.question}>
            <dt>{item.question}</dt>
            <dd>{item.answer}</dd>
          </div>
        ))}
      </dl>

      <nav aria-label="Kat Pool resources">
        <ul>
          <li>
            <Link href="/kaspa-mining-pool">Kaspa mining pool guide</Link>
          </li>
          <li>
            <Link href="/kaspa-mining-calculator">Kaspa mining calculator</Link>
          </li>
          <li>
            <Link href="/kaspa-asic-miners">Kaspa ASIC miner guide</Link>
          </li>
          <li>
            <Link href="/compare">Best Kaspa mining pools compared</Link>
          </li>
          <li>
            <Link href="/vs/humpool">Kat Pool vs HumPool</Link>
          </li>
          <li>
            <Link href="/stats">Live Kat Pool stats</Link>
          </li>
          <li>
            <Link href="/blog">Kat Pool blog</Link>
          </li>
          <li>
            <Link href="/about">About Kat Pool</Link>
          </li>
          <li>
            <a href={APP_URL}>Live dashboard</a>
          </li>
        </ul>
      </nav>
    </section>
  );
}
