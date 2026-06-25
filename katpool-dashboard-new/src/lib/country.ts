/**
 * ISO-3166-1 alpha-2 helpers for the geo distribution view (ADR-0025).
 * Country codes come from the pool's aggregate `/pool/geo` endpoint.
 */

const REGION_NAMES =
  typeof Intl !== "undefined" && "DisplayNames" in Intl
    ? new Intl.DisplayNames(["en"], { type: "region" })
    : null;

/** Alpha-2 country code → English country name (falls back to the code). */
export function countryName(code: string): string {
  const cc = code.toUpperCase();
  if (!/^[A-Z]{2}$/.test(cc)) return cc;
  try {
    return REGION_NAMES?.of(cc) ?? cc;
  } catch {
    return cc;
  }
}

/** Alpha-2 country code → flag emoji (regional-indicator pair). */
export function flagEmoji(code: string): string {
  const cc = code.toUpperCase();
  if (!/^[A-Z]{2}$/.test(cc)) return "\u{1F3F3}\u{FE0F}";
  const base = 0x1f1e6;
  return String.fromCodePoint(
    base + (cc.charCodeAt(0) - 65),
    base + (cc.charCodeAt(1) - 65),
  );
}
