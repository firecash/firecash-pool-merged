/**
 * Connection facts for the "Start mining" guide.
 *
 * Defaults mirror the verified production topology in
 * `docs/cutover-stratum-compatibility.md` (origin + 7-region fly.io anycast
 * edge; eight stratum ports each with a starting-difficulty *seed*; vardiff on
 * every port). Every value is overridable via `NEXT_PUBLIC_*` env so a given
 * deployment (e.g. the tn10 staging pool on a single port) renders its own
 * truth instead of a hard-coded guess.
 */

/** Network the pool mines; drives the example address prefix + faucet copy. */
export type PoolNetwork = "mainnet" | "testnet-10";

export interface StratumPort {
  /** TCP port number. */
  port: number;
  /** Starting difficulty seed sent at authorize (vardiff moves from here). */
  seed: number;
}

export interface StratumRegion {
  /** Human label, e.g. "Europe". */
  label: string;
  /** Fully-qualified host, e.g. "eu.katpool.com". */
  host: string;
  /** True for the primary/anycast origin host. */
  primary?: boolean;
}

/** Verified port → starting-difficulty seed (cutover-stratum-compatibility §1). */
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

/** Region subdomain prefixes (cutover-stratum-compatibility §1 inventory). */
const REGION_PREFIXES: { label: string; prefix: string }[] = [
  { label: "San Francisco", prefix: "na-west" },
  { label: "New York City", prefix: "na-east" },
  { label: "Europe", prefix: "eu" },
  { label: "Singapore", prefix: "ap" },
  { label: "Hong Kong", prefix: "hkg" },
  { label: "São Paulo", prefix: "sa" },
  { label: "Sydney", prefix: "au" },
];

const DEFAULT_HOST = "kas.katpool.com";

/**
 * Trim an env value to `undefined` when empty.
 *
 * IMPORTANT: callers must pass `process.env.NEXT_PUBLIC_*` as a **static**
 * member access. Next.js only inlines `NEXT_PUBLIC_*` vars into the client
 * bundle for static accesses — a dynamic `process.env[key]` lookup is NOT
 * replaced at build time and reads `undefined` in the browser (which silently
 * hid the treasury address and made the other vars fall back to defaults).
 */
function clean(v: string | undefined): string | undefined {
  return v && v.trim() !== "" ? v.trim() : undefined;
}

/** Strip a leading single-label prefix (e.g. "kas.") to get the base domain. */
function baseDomain(host: string): string {
  const parts = host.split(".");
  return parts.length > 2 ? parts.slice(1).join(".") : host;
}

/** Parse a "port:seed,port:seed" override into a sorted port list. */
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
  network: PoolNetwork;
  /** Primary host miners should use (anycast routes to the nearest edge). */
  host: string;
  /** Regional endpoints (primary first); single entry when no edge is set. */
  regions: StratumRegion[];
  /** Primary/anycast region (always present). */
  primary: StratumRegion;
  ports: StratumPort[];
  /** Default port surfaced in examples (always present). */
  recommended: StratumPort;
  /** Example address prefix for the active network. */
  addressPrefix: string;
  /** Pool fee, percent (display only). */
  feePercent: number;
  /** Minimum KAS balance before a payout cycle includes a wallet. */
  minPayoutKas: number;
  /**
   * Public treasury address, for the on-chain reserves link on the Status page.
   * Operator-set per deployment (`NEXT_PUBLIC_TREASURY_ADDRESS`); omitted until
   * configured so nothing misleading is shown.
   */
  treasuryAddress?: string;
}

const FALLBACK_PORT: StratumPort = { port: 3333, seed: 4096 };

/** Resolve the mining guide configuration from env + verified defaults. */
export function miningConfig(): MiningConfig {
  const network: PoolNetwork =
    clean(process.env.NEXT_PUBLIC_POOL_NETWORK) === "testnet-10" ? "testnet-10" : "mainnet";
  const host = clean(process.env.NEXT_PUBLIC_STRATUM_HOST) ?? DEFAULT_HOST;
  const domain = baseDomain(host);

  const portsRaw = clean(process.env.NEXT_PUBLIC_STRATUM_PORTS);
  const ports = portsRaw ? parsePorts(portsRaw) : DEFAULT_PORTS;

  // A geo edge only exists on the multi-region production topology. When the
  // deployment overrides the host (e.g. a single-port staging pool) we don't
  // fabricate regional names — show just the configured host.
  const showRegions = host === DEFAULT_HOST && portsRaw === undefined;
  const primary: StratumRegion = { label: "Global (anycast)", host, primary: true };
  const regions: StratumRegion[] = [primary];
  if (showRegions) {
    for (const r of REGION_PREFIXES) regions.push({ label: r.label, host: `${r.prefix}.${domain}` });
  }

  const recommended = ports.find((p) => p.port === 3333) ?? ports[0] ?? FALLBACK_PORT;

  return {
    network,
    host,
    regions,
    primary,
    ports,
    recommended,
    addressPrefix: network === "testnet-10" ? "kaspatest" : "kaspa",
    feePercent: 0.75,
    minPayoutKas: 10,
    treasuryAddress: clean(process.env.NEXT_PUBLIC_TREASURY_ADDRESS),
  };
}
