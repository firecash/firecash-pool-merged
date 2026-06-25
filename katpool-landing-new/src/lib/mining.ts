export interface StratumPort {
  port: number;
  seed: number;
}

export interface StratumRegion {
  label: string;
  host: string;
  primary?: boolean;
}

const DEFAULT_PORTS: StratumPort[] = [
  { port: 1111, seed: 256 },
  { port: 2222, seed: 1024 },
  { port: 3333, seed: 4096 },
  { port: 4444, seed: 8192 },
  { port: 5555, seed: 16384 },
  { port: 6666, seed: 32768 },
  { port: 7777, seed: 65536 },
  { port: 8888, seed: 2048 },
];

const REGION_PREFIXES: { label: string; prefix: string }[] = [
  { label: "Anycast", prefix: "anycast" },
  { label: "San Francisco", prefix: "na-west" },
  { label: "New York", prefix: "na-east" },
  { label: "Europe", prefix: "eu" },
  { label: "Singapore", prefix: "ap" },
  { label: "Hong Kong", prefix: "hkg" },
  { label: "São Paulo", prefix: "sa" },
  { label: "Sydney", prefix: "au" },
];

const DEFAULT_HOST = "kas.katpool.com";

function clean(v: string | undefined): string | undefined {
  return v && v.trim() !== "" ? v.trim() : undefined;
}

function baseDomain(host: string): string {
  const parts = host.split(".");
  return parts.length > 2 ? parts.slice(1).join(".") : host;
}

function parsePorts(raw: string): StratumPort[] {
  const out: StratumPort[] = [];
  for (const pair of raw.split(",")) {
    const [p, s] = pair.split(":");
    const port = Number(p);
    const seed = Number(s);
    if (Number.isInteger(port) && port > 0 && Number.isFinite(seed) && seed > 0) {
      out.push({ port, seed });
    }
  }
  return out.length ? out : DEFAULT_PORTS;
}

export interface MiningConfig {
  host: string;
  regions: StratumRegion[];
  ports: StratumPort[];
  recommended: StratumPort;
  minPayoutKas: number;
  toplineFeePercent: number;
  displayFeePercent: number;
}

export function miningConfig(): MiningConfig {
  const host = clean(process.env.NEXT_PUBLIC_STRATUM_HOST) ?? DEFAULT_HOST;
  const domain = baseDomain(host);
  const portsRaw = clean(process.env.NEXT_PUBLIC_STRATUM_PORTS);
  const ports = portsRaw ? parsePorts(portsRaw) : DEFAULT_PORTS;
  const showRegions = host === DEFAULT_HOST && portsRaw === undefined;

  const primary: StratumRegion = { label: "Global anycast", host, primary: true };
  const regions: StratumRegion[] = [primary];
  if (showRegions) {
    for (const r of REGION_PREFIXES) {
      regions.push({ label: r.label, host: `${r.prefix}.${domain}` });
    }
  }

  const recommended = ports.find((p) => p.port === 3333) ?? ports[0] ?? { port: 3333, seed: 4096 };

  return {
    host,
    regions,
    ports,
    recommended,
    minPayoutKas: 10,
    toplineFeePercent: 0.75,
    displayFeePercent: 0.5,
  };
}

export const APP_URL = process.env.NEXT_PUBLIC_APP_URL ?? "https://app.katpool.com";
export const GITHUB_URL = process.env.NEXT_PUBLIC_GITHUB_URL ?? "https://github.com/Nacho-the-Kat/katpool";
export const TWITTER_URL = process.env.NEXT_PUBLIC_TWITTER_URL ?? "https://x.com/Katpool_Mining";
/** X / Twitter handle (with leading @) derived from the profile URL. */
export const TWITTER_HANDLE = `@${TWITTER_URL.replace(/\/+$/, "").split("/").pop()}`;
