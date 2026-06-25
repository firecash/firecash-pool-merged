import "server-only";

/** A fetch that times out, so a slow upstream can't pin a BFF request open. */
export async function fetchJson<T>(
  url: string,
  init: RequestInit & { revalidate?: number; noStore?: boolean; timeoutMs?: number } = {},
): Promise<T> {
  const { revalidate, noStore, timeoutMs = 8000, ...rest } = init;
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  try {
    const res = await fetch(url, {
      ...rest,
      signal: controller.signal,
      headers: { accept: "application/json", ...(rest.headers ?? {}) },
      ...(noStore
        ? { cache: "no-store" as const }
        : revalidate != null
          ? { next: { revalidate } }
          : {}),
    });
    if (!res.ok) {
      throw new UpstreamError(
        `upstream ${res.status} for ${safeUrl(url)}`,
        res.status,
        res.headers.get("retry-after") ?? undefined,
      );
    }
    return (await res.json()) as T;
  } finally {
    clearTimeout(timer);
  }
}

/** An error carrying the upstream HTTP status, for accurate BFF mapping. */
export class UpstreamError extends Error {
  constructor(
    message: string,
    readonly status: number,
    /** Upstream `Retry-After` (seconds or HTTP-date), preserved for 429 passthrough. */
    readonly retryAfter?: string,
  ) {
    super(message);
    this.name = "UpstreamError";
  }
}

/** Strip query/credentials from a URL before logging. */
export function safeUrl(url: string): string {
  try {
    const u = new URL(url);
    return `${u.origin}${u.pathname}`;
  } catch {
    return "invalid-url";
  }
}
