const BASE = (
  process.env.NEXT_PUBLIC_EXPLORER_BASE_URL ?? "https://explorer.kaspa.org"
).replace(/\/+$/, "");

/** Explorer deep-link for an address. */
export function explorerAddress(address: string): string {
  return `${BASE}/addresses/${encodeURIComponent(address)}`;
}

/** Explorer deep-link for a block hash. */
export function explorerBlock(hash: string): string {
  return `${BASE}/blocks/${encodeURIComponent(hash)}`;
}

/** Explorer deep-link for a transaction hash. */
export function explorerTx(hash: string): string {
  return `${BASE}/txs/${encodeURIComponent(hash)}`;
}

/**
 * kaspa.stream address deep-link (used for the public treasury reserves link).
 * kaspa.stream surfaces KRC-20 holdings alongside the KAS balance, which suits
 * the treasury view. Overridable via `NEXT_PUBLIC_STREAM_EXPLORER_BASE_URL`.
 */
const STREAM_BASE = (
  process.env.NEXT_PUBLIC_STREAM_EXPLORER_BASE_URL ?? "https://kaspa.stream"
).replace(/\/+$/, "");

/** kaspa.stream deep-link for an address. */
export function streamAddress(address: string): string {
  return `${STREAM_BASE}/addresses/${encodeURIComponent(address)}`;
}
