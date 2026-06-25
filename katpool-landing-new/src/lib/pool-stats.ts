export interface MiningPoolStats {
  coin_mined: string;
  pool_name: string;
  url: string;
  poolFee: number;
  current_hashRate: string;
  totalBlocksCount: number;
  minPay: number;
  feeType: string;
  lastblock: string;
  lastblocktime: string;
  country: string;
}

const DEFAULT_API = "https://api.katpool.com";

export function poolApiBase(): string {
  return (process.env.NEXT_PUBLIC_POOL_API_URL ?? DEFAULT_API).replace(/\/$/, "");
}

export async function fetchPoolStats(): Promise<MiningPoolStats> {
  const res = await fetch(`${poolApiBase()}/api/pool/miningPoolStats`, {
    next: { revalidate: 30 },
  });
  if (!res.ok) throw new Error(`pool stats ${res.status}`);
  return res.json() as Promise<MiningPoolStats>;
}

/** Parse compact hashrate strings like "591.73TH/s" into a display number + unit. */
export function parseHashRate(raw: string): { value: string; unit: string } {
  const m = raw.match(/^([\d.]+)([A-Za-z/]+)$/);
  if (!m) return { value: raw, unit: "" };
  return { value: m[1], unit: m[2] };
}

export function formatRelativeTime(iso: string, nowMs = Date.now()): string {
  const then = new Date(iso).getTime();
  if (Number.isNaN(then)) return "—";
  const sec = Math.max(0, Math.floor((nowMs - then) / 1000));
  if (sec < 60) return `${sec}s ago`;
  if (sec < 3600) return `${Math.floor(sec / 60)}m ago`;
  if (sec < 86400) return `${Math.floor(sec / 3600)}h ago`;
  return `${Math.floor(sec / 86400)}d ago`;
}

export function formatBlockCount(n: number): string {
  return new Intl.NumberFormat("en-US").format(Math.round(n));
}
