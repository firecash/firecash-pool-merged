/**
 * Presentation formatters. All inputs are treated as exact: money arrives as
 * decimal strings (never floats) and is scaled with BigInt; hashrate is a
 * rate (lossy by definition) and uses Number.
 */

const HASH_UNITS = ["H/s", "KH/s", "MH/s", "GH/s", "TH/s", "PH/s", "EH/s", "ZH/s"] as const;

/** Format a hashrate given in H/s, auto-scaling to the largest sensible unit. */
export function formatHashrate(hs: number, digits = 2): string {
  if (!Number.isFinite(hs) || hs < 0) return "—";
  if (hs === 0) return `0 ${HASH_UNITS[0]}`;
  const exp = Math.min(Math.floor(Math.log10(hs) / 3), HASH_UNITS.length - 1);
  const scaled = hs / 1000 ** exp;
  return `${trimZeros(scaled.toFixed(digits))} ${HASH_UNITS[exp]}`;
}

/** Convert a Kaspa-API hashrate (TH/s) to H/s for unit-consistent display. */
export function networkHashrateToHs(teraHashes: number): number {
  return teraHashes * 1e12;
}

/** Compact integer/decimal with thousands separators. */
export function formatNumber(value: number, digits = 0): string {
  if (!Number.isFinite(value)) return "—";
  return value.toLocaleString("en-US", {
    minimumFractionDigits: 0,
    maximumFractionDigits: digits,
  });
}

/** Abbreviate large counts: 1.2K, 3.4M, 5.6B. */
export function formatCompact(value: number, digits = 1): string {
  if (!Number.isFinite(value)) return "—";
  return value.toLocaleString("en-US", {
    notation: "compact",
    maximumFractionDigits: digits,
  });
}

/** Format a KAS decimal string (e.g. "1739.516") with grouped thousands. */
export function formatKas(kas: string, maxDigits = 4): string {
  const n = Number(kas);
  if (!Number.isFinite(n)) return kas;
  return `${n.toLocaleString("en-US", { maximumFractionDigits: maxDigits })} KAS`;
}

/** Format NACHO base units (8 decimals) as a human token amount. */
export function formatNacho(baseUnits: string | number, maxDigits = 4): string {
  let raw: bigint;
  try {
    raw = typeof baseUnits === "string" ? BigInt(baseUnits) : BigInt(baseUnits);
  } catch {
    return "—";
  }
  const negative = raw < 0n;
  if (negative) raw = -raw;
  const whole = raw / 100_000_000n;
  const frac = raw % 100_000_000n;
  const fracStr = frac.toString().padStart(8, "0").replace(/0+$/, "");
  const display =
    fracStr.length > 0
      ? `${whole.toString()}.${fracStr.slice(0, maxDigits)}`
      : whole.toString();
  const n = Number(display);
  if (Number.isFinite(n) && whole < 1_000_000n) {
    return `${negative ? "-" : ""}${n.toLocaleString("en-US", { maximumFractionDigits: maxDigits })} NACHO`;
  }
  return `${negative ? "-" : ""}${display} NACHO`;
}

/** Format a USD price; picks precision by magnitude (handles sub-cent tokens). */
export function formatUsd(value: number | null | undefined): string {
  if (value == null || !Number.isFinite(value)) return "—";
  if (value !== 0 && Math.abs(value) < 0.01) {
    return `$${value.toLocaleString("en-US", { maximumSignificantDigits: 3 })}`;
  }
  return value.toLocaleString("en-US", { style: "currency", currency: "USD" });
}

const SUBSCRIPT_DIGITS = ["₀", "₁", "₂", "₃", "₄", "₅", "₆", "₇", "₈", "₉"] as const;

function toSubscript(n: number): string {
  return String(n)
    .split("")
    .map((d) => SUBSCRIPT_DIGITS[Number(d)] ?? d)
    .join("");
}

/**
 * USD price formatter tuned for tokens spanning many orders of magnitude.
 * - ≥ $1: two decimals.
 * - $0.01–$1: up to four decimals (trailing zeros trimmed).
 * - < $0.01: compact subscript-zero notation, e.g. `$0.0₄1234` (= $0.00001234),
 *   the convention used across crypto price UIs — keeps a fixed footprint while
 *   preserving significant figures no matter how small the price gets.
 */
export function formatUsdPrice(value: number | null | undefined, sig = 4): string {
  if (value == null || !Number.isFinite(value)) return "—";
  if (value === 0) return "$0.00";
  const abs = Math.abs(value);
  const sign = value < 0 ? "-" : "";

  if (abs >= 0.01) {
    return `${sign}${abs.toLocaleString("en-US", {
      style: "currency",
      currency: "USD",
      minimumFractionDigits: 2,
      maximumFractionDigits: abs >= 1 ? 2 : 4,
    })}`;
  }

  // Sub-cent: render as $0.0<sub-zero-count><significant digits>.
  let exp = Math.floor(Math.log10(abs));
  let significand = Math.round((abs / 10 ** exp) * 10 ** (sig - 1));
  // Rounding can carry into the next power of ten (e.g. 9.999 → 10.00).
  if (significand >= 10 ** sig) {
    significand = Math.round(significand / 10);
    exp += 1;
  }
  const zeros = -exp - 1;
  const digits = String(significand).replace(/0+$/, "") || "0";
  return `${sign}$0.0${toSubscript(zeros)}${digits}`;
}

/** Signed percentage with one decimal, for delta chips. */
export function formatPercent(value: number | null | undefined, digits = 1): string {
  if (value == null || !Number.isFinite(value)) return "—";
  const sign = value > 0 ? "+" : "";
  return `${sign}${value.toFixed(digits)}%`;
}

/** USD value of a sompi amount (string) at a KAS/USD price. */
export function sompiToUsd(sompi: string, kasUsd: number | null): number | null {
  if (kasUsd == null) return null;
  try {
    const kas = Number(BigInt(sompi)) / 1e8;
    return kas * kasUsd;
  } catch {
    return null;
  }
}

const RTF = new Intl.RelativeTimeFormat("en", { numeric: "auto" });

/** Relative time like "3 minutes ago" / "in 2 days". */
export function formatRelative(iso: string | number | Date, nowMs = Date.now()): string {
  const then = new Date(iso).getTime();
  if (Number.isNaN(then)) return "—";
  const deltaSec = Math.round((then - nowMs) / 1000);
  const abs = Math.abs(deltaSec);
  const table: [number, Intl.RelativeTimeFormatUnit][] = [
    [60, "second"],
    [3600, "minute"],
    [86400, "hour"],
    [604800, "day"],
    [2629800, "week"],
    [31557600, "month"],
    [Infinity, "year"],
  ];
  let unitSec = 1;
  let unit: Intl.RelativeTimeFormatUnit = "second";
  for (const [limit, u] of table) {
    if (abs < limit) {
      unit = u;
      break;
    }
    unitSec = limit;
  }
  return RTF.format(Math.round(deltaSec / unitSec), unit);
}

/**
 * Absolute timestamp in the viewer's local timezone, compact. The trailing
 * `timeZoneName: "short"` makes the zone explicit (e.g. "PDT") so a local time
 * is never mistaken for UTC.
 */
export function formatDateTime(iso: string | number | Date): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "—";
  return d.toLocaleString("en-US", {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    timeZoneName: "short",
  });
}

/** Middle-truncate a long hash/address: kaspa:qq…h4x9. */
export function truncateMiddle(value: string, head = 10, tail = 6): string {
  if (value.length <= head + tail + 1) return value;
  return `${value.slice(0, head)}…${value.slice(-tail)}`;
}

/** Human-readable duration from seconds. */
export function formatDuration(totalSec: number): string {
  if (!Number.isFinite(totalSec) || totalSec < 0) return "—";
  const d = Math.floor(totalSec / 86400);
  const h = Math.floor((totalSec % 86400) / 3600);
  const m = Math.floor((totalSec % 3600) / 60);
  if (d > 0) return `${d}d ${h}h`;
  if (h > 0) return `${h}h ${m}m`;
  if (m > 0) return `${m}m`;
  return `${Math.floor(totalSec)}s`;
}

function trimZeros(s: string): string {
  return s.includes(".") ? s.replace(/\.?0+$/, "") : s;
}
