import type { ApiErrorBody } from "./types";

/** A client-safe error raised when a BFF call fails. */
export class DashboardApiError extends Error {
  constructor(
    message: string,
    readonly status: number,
    readonly code: string,
  ) {
    super(message);
    this.name = "DashboardApiError";
  }
}

/** Build a same-origin BFF URL with bounded query params. */
export function bffUrl(path: string, params?: Record<string, string | number | undefined>): string {
  const qs = new URLSearchParams();
  if (params) {
    for (const [k, v] of Object.entries(params)) {
      if (v !== undefined && v !== null && v !== "") qs.set(k, String(v));
    }
  }
  const query = qs.toString();
  return query ? `${path}?${query}` : path;
}

/** Fetch + parse JSON from the same-origin BFF, mapping the error envelope. */
export async function fetchBff<T>(url: string, signal?: AbortSignal): Promise<T> {
  // `no-store` bypasses the browser HTTP cache so a React Query refetch always
  // reaches the BFF instead of being answered from a `stale-while-revalidate`
  // disk-cache entry. Without this, status transitions (e.g. a payout going
  // submitted → confirmed) appear frozen until the user clears their cache; the
  // BFF's own short server-side revalidate still shields the upstream API.
  const res = await fetch(url, {
    signal,
    cache: "no-store",
    headers: { accept: "application/json" },
  });
  if (!res.ok) {
    let code = "error";
    let message = `request failed (${res.status})`;
    try {
      const body = (await res.json()) as ApiErrorBody;
      if (body?.error) {
        code = body.error.code ?? code;
        message = body.error.message ?? message;
      }
    } catch {
      /* non-JSON error body */
    }
    throw new DashboardApiError(message, res.status, code);
  }
  return (await res.json()) as T;
}
