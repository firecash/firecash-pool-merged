import Link from "next/link";
import {
  ArrowRight,
  Cpu,
  Gauge,
  Globe2,
  KeyRound,
  LineChart,
  Plug,
  ShieldCheck,
  Wallet,
} from "lucide-react";
import { Card } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Panel } from "@/components/dashboard/panel";
import { CopyButton } from "@/components/dashboard/copy-button";
import { miningConfig } from "@/lib/mining";
import { ECOSYSTEM } from "@/lib/ecosystem";
import { ExtLink } from "@/components/ext-link";
import { formatNumber } from "@/lib/format";

/** Brand families surfaced as quick badges on the connect card. */
const ASIC_BRANDS = ["IceRiver KS-series", "Bitmain Antminer KS-series", "Goldshell KA BOX"] as const;

/**
 * The full kHeavyHash (Kaspa) ASIC lineup, grouped by the stratum port whose
 * starting difficulty best fits each model's hashrate. Vardiff fine-tunes from
 * there — the only goal is to start close enough that the first shares validate.
 * Starting far too high makes a small rig submit only rejects and reconnect
 * before vardiff can settle, so when unsure, start lower (3333). Hashrates are
 * per-manufacturer rated specs; the port mapping targets the pool's ~20
 * shares/min vardiff setpoint.
 */
const PORT_GUIDE: {
  port: number;
  fits: string;
  recommended?: boolean;
  models: string[];
}[] = [
  {
    port: 1111,
    fits: "up to ~0.5 TH/s",
    models: ["IceRiver KS0", "IceRiver KS0 Pro", "IceRiver KS0 Ultra"],
  },
  {
    port: 2222,
    fits: "~1–2 TH/s",
    models: ["IceRiver KS1", "IceRiver KS2", "Goldshell KA BOX", "Goldshell KA BOX Pro"],
  },
  {
    port: 3333,
    fits: "~5–9 TH/s",
    recommended: true,
    models: ["IceRiver KS3L", "IceRiver KS3M", "IceRiver KS3", "Bitmain Antminer KS3"],
  },
  {
    port: 4444,
    fits: "~12–21 TH/s",
    models: ["IceRiver KS5L", "IceRiver KS5M", "Bitmain Antminer KS5", "Bitmain Antminer KS5 Pro"],
  },
  {
    port: 5555,
    fits: "~25 TH/s and up",
    models: ["IceRiver KS7", "Several rigs on one connection"],
  },
];

/** Short per-port "best for" hint, keyed by port, for the ports table. */
const PORT_FITS: Record<number, string> = {
  1111: "Small — KS0 family",
  2222: "Entry — KS1 / KS2 / KA BOX",
  3333: "Recommended — KS3-class",
  4444: "Large — KS5-class",
  5555: "Very large — KS7",
  6666: "Farm / multi-rig",
  7777: "Farm / multi-rig",
  8888: "Entry (alternate)",
};

/** A labelled, copyable monospace field (connection settings). */
function CopyField({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between gap-3 rounded-lg border border-border bg-background/60 px-3 py-2">
      <div className="min-w-0">
        <p className="text-[0.6875rem] uppercase tracking-[0.1em] text-muted-foreground">{label}</p>
        <p className="truncate font-mono text-sm text-foreground" title={value}>
          {value}
        </p>
      </div>
      <CopyButton value={value} label={`Copy ${label}`} />
    </div>
  );
}

function Step({
  n,
  icon: Icon,
  title,
  children,
}: {
  n: number;
  icon: typeof Wallet;
  title: string;
  children: React.ReactNode;
}) {
  return (
    <Card className="flex h-full flex-col gap-3 p-5">
      <div className="flex items-center gap-3">
        <span className="flex size-9 items-center justify-center rounded-xl border border-primary/30 bg-primary/10 text-primary">
          <Icon className="size-5" />
        </span>
        <span className="text-[0.6875rem] font-medium uppercase tracking-[0.14em] text-muted-foreground">
          Step {n}
        </span>
      </div>
      <h3 className="text-base font-semibold tracking-tight">{title}</h3>
      <div className="text-sm leading-relaxed text-muted-foreground [&_a]:text-primary [&_a:hover]:underline">
        {children}
      </div>
    </Card>
  );
}

/**
 * "Start mining" — the flagship onboarding guide. Connection facts come from
 * {@link miningConfig} (env-overridable; defaults from the verified cutover
 * topology), so the page is always accurate for the deployment it ships in.
 */
export function StartGuide() {
  const cfg = miningConfig();
  const primary = cfg.primary;
  const recommendedPort = cfg.recommended;
  const addressExample = `${cfg.addressPrefix}:your-wallet-address`;
  const userExample = `${addressExample}.rig1`;
  const stratumUrl = `stratum+tcp://${primary.host}:${recommendedPort.port}`;
  const isTestnet = cfg.network === "testnet-10";

  return (
    <div className="space-y-6">
      {/* CTA hero */}
      <Card className="relative overflow-hidden">
        <div className="pointer-events-none absolute inset-0 app-aurora opacity-80" />
        <div className="pointer-events-none absolute -right-24 -top-24 size-72 rounded-full bg-primary/15 blur-3xl" />
        <div className="relative flex flex-col gap-6 p-6 sm:p-8 lg:flex-row lg:items-center lg:justify-between">
          <div className="max-w-2xl">
            <Badge variant="success" className="mb-3">
              <span className="size-1.5 rounded-full bg-success live-dot" /> Accepting miners now
            </Badge>
            <h2 className="text-2xl font-semibold tracking-tight sm:text-3xl">
              Point your rig at <span className="text-grad">katpool</span> in under two minutes
            </h2>
            <p className="mt-2 text-sm text-muted-foreground sm:text-base">
              Low {cfg.feePercent}% fee with a <ExtLink href={ECOSYSTEM.nacho}>NACHO</ExtLink> rebate,
              variable difficulty on every port, and a global anycast edge that routes you to the
              nearest server automatically.
            </p>
            <div className="mt-5 flex flex-wrap gap-2">
              <Badge variant="outline">
                <Gauge className="size-3.5" /> {cfg.feePercent}% fee +{" "}
                <ExtLink href={ECOSYSTEM.nacho}>NACHO</ExtLink> rebate
              </Badge>
              <Badge variant="outline">
                <Cpu className="size-3.5" /> Variable difficulty
              </Badge>
              <Badge variant="outline">
                <Globe2 className="size-3.5" /> {cfg.regions.length > 1 ? "7-region edge" : "Anycast edge"}
              </Badge>
              <Badge variant="outline">
                <Wallet className="size-3.5" /> {cfg.minPayoutKas}{" "}
                <ExtLink href={ECOSYSTEM.kaspa}>KAS</ExtLink> min payout
              </Badge>
            </div>
          </div>
          <div className="flex shrink-0 flex-col gap-2 sm:flex-row lg:flex-col">
            <Button asChild size="lg">
              <a href="#connect">
                Connection settings <ArrowRight className="size-4" />
              </a>
            </Button>
            <Button asChild variant="outline" size="lg">
              <Link href="/">Pool overview</Link>
            </Button>
          </div>
        </div>
      </Card>

      {/* Steps */}
      <div className="grid grid-cols-1 gap-4 md:grid-cols-3">
        <Step n={1} icon={Wallet} title="Get a Kaspa address">
          You&apos;re paid directly to your own wallet — katpool never holds your coins. Create a{" "}
          {isTestnet ? (
            <>
              testnet wallet and fund it from the{" "}
              <a href="https://faucet-tn10.kaspanet.io" target="_blank" rel="noreferrer">
                tn10 faucet
              </a>
            </>
          ) : (
            <>
              wallet (e.g.{" "}
              <ExtLink href={ECOSYSTEM.kaspium}>Kaspium</ExtLink> or the{" "}
              <ExtLink href={ECOSYSTEM.kaspa}>Kaspa</ExtLink> desktop wallet)
            </>
          )}
          , then copy your <span className="font-mono text-foreground">{cfg.addressPrefix}:</span>{" "}
          receiving address.
        </Step>
        <Step n={2} icon={Plug} title="Configure your miner">
          Set the pool URL, use your address (optionally <span className="font-mono">.worker</span>)
          as the username, and any value as the password. Full settings are{" "}
          <a href="#connect">below</a>.
        </Step>
        <Step n={3} icon={LineChart} title="Watch it live">
          Your rig appears within a minute. Paste your address into the search bar at the top to
          follow hashrate, workers, shares, balance and payouts in real time.
        </Step>
      </div>

      {/* Connection settings */}
      <div id="connect" className="scroll-mt-6 grid grid-cols-1 gap-6 lg:grid-cols-3">
        <Panel
          className="lg:col-span-2"
          eyebrow="Connect"
          title="Connection settings"
          description="Works with every kHeavyHash ASIC — IceRiver, Bitmain Antminer KS, and Goldshell."
        >
          <div className="space-y-3">
            <CopyField label="Stratum URL" value={stratumUrl} />
            <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
              <CopyField label="Username" value={userExample} />
              <CopyField label="Password" value="x" />
            </div>
            <p className="text-xs text-muted-foreground">
              Replace <span className="font-mono">your-wallet-address</span> with your real{" "}
              {cfg.addressPrefix}: address. The text after the dot
              (<span className="font-mono">.rig1</span>) is your worker name — pick anything. The
              password is ignored, so any value works.
            </p>

            <div className="space-y-2 pt-2">
              <p className="text-[0.6875rem] font-medium uppercase tracking-[0.12em] text-muted-foreground">
                On your ASIC
              </p>
              <p className="text-xs leading-relaxed text-muted-foreground">
                Open the miner&apos;s web dashboard, go to{" "}
                <span className="font-medium text-foreground">Settings → Pools</span>, and enter the
                values above as <span className="font-medium text-foreground">Pool 1</span> (URL,
                worker, password). Save — the rig reconnects to katpool automatically.
              </p>
              <div className="flex flex-wrap gap-1.5 pt-1">
                {ASIC_BRANDS.map((model) => (
                  <Badge key={model} variant="outline">
                    {model}
                  </Badge>
                ))}
              </div>
            </div>
          </div>
        </Panel>

        <Panel eyebrow="Choose a port" title="Ports & starting difficulty" description="Match your rig size — vardiff fine-tunes from there.">
          <div className="overflow-hidden rounded-xl border border-border">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-border bg-muted/40 text-left text-xs text-muted-foreground">
                  <th className="px-3 py-2.5 font-medium">Port</th>
                  <th className="px-3 py-2.5 font-medium">Best for</th>
                  <th className="px-3 py-2.5 text-right font-medium">Start diff</th>
                  <th className="px-3 py-2.5 text-right font-medium" />
                </tr>
              </thead>
              <tbody>
                {cfg.ports.map((p) => {
                  const isRec = p.port === recommendedPort.port;
                  return (
                    <tr
                      key={p.port}
                      className={`border-b border-border/60 last:border-0 ${isRec ? "bg-primary/5" : ""}`}
                    >
                      <td className="px-3 py-2.5 font-mono">
                        {p.port}
                        {isRec ? <span className="ml-1.5 align-middle text-[0.625rem] text-primary">★</span> : null}
                      </td>
                      <td className="px-3 py-2.5 text-xs text-muted-foreground">{PORT_FITS[p.port] ?? "—"}</td>
                      <td className="px-3 py-2.5 text-right tnum">{formatNumber(p.seed)}</td>
                      <td className="px-3 py-2.5 text-right">
                        <CopyButton value={`${primary.host}:${p.port}`} label={`Copy ${primary.host}:${p.port}`} />
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
          <p className="mt-3 text-xs text-muted-foreground">
            The number is a <span className="font-medium text-foreground">starting</span> difficulty —
            variable difficulty then converges you to a steady share rate. Picking one too high for a
            small rig is the usual cause of constant rejects, so{" "}
            <span className="font-medium text-foreground">when unsure, use {recommendedPort.port}</span>.
            Not sure which fits your miner? See the guide below.
          </p>
        </Panel>
      </div>

      {/* Which port for your miner */}
      <Panel
        eyebrow="Match your miner"
        title="Which port for your ASIC?"
        description="Every Kaspa miner runs kHeavyHash. Find your model, start on its port, and vardiff handles the rest."
      >
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {PORT_GUIDE.map((g) => (
            <div
              key={g.port}
              className={`flex flex-col gap-2.5 rounded-xl border p-4 ${
                g.recommended ? "border-primary/40 bg-primary/5" : "border-border bg-background/40"
              }`}
            >
              <div className="flex items-center justify-between gap-2">
                <div className="flex min-w-0 items-center gap-2">
                  <Cpu className="size-4 shrink-0 text-primary" />
                  <span className="font-mono text-sm font-semibold">{primary.host}:{g.port}</span>
                </div>
                <CopyButton value={`${primary.host}:${g.port}`} label={`Copy ${primary.host}:${g.port}`} />
              </div>
              <div className="flex flex-wrap items-center gap-2">
                <span className="text-xs text-muted-foreground">{g.fits}</span>
                {g.recommended ? <Badge variant="default">Start here if unsure</Badge> : null}
              </div>
              <div className="flex flex-wrap gap-1.5">
                {g.models.map((m) => (
                  <Badge key={m} variant="outline">
                    {m}
                  </Badge>
                ))}
              </div>
            </div>
          ))}
        </div>
        <p className="mt-3 text-xs text-muted-foreground">
          Hashrates are manufacturer specs (±10%). Don&apos;t see your exact model? Pick the closest
          size — the starting difficulty only needs to be in the right ballpark, and variable
          difficulty converges from there. Ports 6666 and 7777 seed even higher for large multi-rig
          connections.
        </p>
      </Panel>

      {/* Endpoints + fees */}
      <div className="grid grid-cols-1 gap-6 lg:grid-cols-2">
        <Panel
          eyebrow="Servers"
          title="Regional endpoints"
          description={
            cfg.regions.length > 1
              ? "Use the nearest host, or the global name to let anycast pick for you."
              : "Connect to the pool host below."
          }
        >
          <div className="divide-y divide-border/60">
            {cfg.regions.map((r) => (
              <div key={r.host} className="flex items-center justify-between gap-3 py-2.5">
                <div className="flex items-center gap-2">
                  <Globe2 className="size-4 text-muted-foreground" />
                  <span className="text-sm">{r.label}</span>
                  {r.primary ? <Badge variant="default">Recommended</Badge> : null}
                </div>
                <div className="flex items-center gap-1">
                  <span className="font-mono text-sm text-muted-foreground">{r.host}</span>
                  <CopyButton value={r.host} label={`Copy ${r.host}`} />
                </div>
              </div>
            ))}
          </div>
        </Panel>

        <Panel eyebrow="Economics" title="Fees & payouts" description="Transparent, miner-first economics.">
          <ul className="space-y-3 text-sm">
            <li className="flex items-start gap-3">
              <Gauge className="mt-0.5 size-4 shrink-0 text-primary" />
              <span>
                <span className="font-medium text-foreground">{cfg.feePercent}% topline fee</span> — among
                the lowest anywhere, taken only off block rewards you help find (PROP).
              </span>
            </li>
            <li className="flex items-start gap-3">
              <ShieldCheck className="mt-0.5 size-4 shrink-0 text-primary" />
              <span>
                <span className="font-medium text-foreground">
                  <ExtLink href={ECOSYSTEM.nacho}>NACHO</ExtLink> rebate
                </span>{" "}
                — Standard miners get 33% of the fee back as NACHO; Elite miners get 100%, paid
                automatically.
              </span>
            </li>
            <li className="flex items-start gap-3">
              <Wallet className="mt-0.5 size-4 shrink-0 text-primary" />
              <span>
                <span className="font-medium text-foreground">
                  {cfg.minPayoutKas} <ExtLink href={ECOSYSTEM.kaspa}>KAS</ExtLink> minimum
                </span>{" "}
                — automatic payouts run on a ~6-hour cycle straight to your wallet.
              </span>
            </li>
            <li className="flex items-start gap-3">
              <KeyRound className="mt-0.5 size-4 shrink-0 text-primary" />
              <span>
                <span className="font-medium text-foreground">Non-custodial</span> — rewards are sent to
                your address; the pool never holds miner funds.
              </span>
            </li>
          </ul>
        </Panel>
      </div>

      {/* FAQ */}
      <Panel eyebrow="Good to know" title="FAQ">
        <dl className="grid grid-cols-1 gap-x-8 gap-y-5 sm:grid-cols-2">
          <div>
            <dt className="text-sm font-medium text-foreground">Do I set a difficulty?</dt>
            <dd className="mt-1 text-sm text-muted-foreground">
              No. Variable difficulty adjusts automatically toward a steady share rate. The port you
              pick only sets the starting point.
            </dd>
          </div>
          <div>
            <dt className="text-sm font-medium text-foreground">Which miners are supported?</dt>
            <dd className="mt-1 text-sm text-muted-foreground">
              Every kHeavyHash ASIC — IceRiver KS-series, Bitmain Antminer KS-series, and Goldshell
              KA-series. <ExtLink href={ECOSYSTEM.kaspa}>Kaspa</ExtLink> is ASIC-only; CPU and GPU
              mining is no longer competitive.
            </dd>
          </div>
          <div>
            <dt className="text-sm font-medium text-foreground">How are workers named?</dt>
            <dd className="mt-1 text-sm text-muted-foreground">
              Append <span className="font-mono">.name</span> to your address in the username (e.g.{" "}
              <span className="font-mono">{cfg.addressPrefix}:…​.rig1</span>) to track rigs separately.
            </dd>
          </div>
          <div>
            <dt className="text-sm font-medium text-foreground">Seeing rejects at first?</dt>
            <dd className="mt-1 text-sm text-muted-foreground">
              A few are normal while difficulty converges. But if almost every share is rejected — or
              your miner keeps reconnecting — the starting difficulty is too high for your rig: switch
              to a <span className="font-medium text-foreground">lower</span> port (try{" "}
              {recommendedPort.port} or below) using the port guide above.
            </dd>
          </div>
        </dl>
      </Panel>
    </div>
  );
}
