import { NextResponse, type NextRequest } from "next/server";

/**
 * Canonical-host redirect.
 *
 * Both `katpool.com` and `katpool.xyz` are attached to the Railway service, so
 * Railway serves the app on every alias without redirecting between them. To
 * consolidate SEO authority (and because `.com` reads as more trustworthy) we
 * 308-redirect the alias hosts to the single canonical origin derived from
 * `NEXT_PUBLIC_SITE_URL`, preserving the path and query string.
 *
 * Only the explicit alias hosts are redirected — Railway's internal
 * `*.up.railway.app` health-check host and `localhost` are left untouched so
 * deploy health checks (which do not follow redirects) keep passing.
 *
 * IMPORTANT: any registrar/Railway-level rule that currently forwards
 * `katpool.com → katpool.xyz` must be removed, otherwise it will fight this
 * redirect and create a loop.
 */

const SITE_URL = process.env.NEXT_PUBLIC_SITE_URL ?? "https://katpool.com";
const CANONICAL_HOST = new URL(SITE_URL).hostname;

// Comma-separated override; defaults to the known `.xyz` + `www` aliases.
const ALIAS_HOSTS = (
  process.env.NEXT_PUBLIC_CANONICAL_REDIRECT_HOSTS ??
  "katpool.xyz,www.katpool.xyz,www.katpool.com"
)
  .split(",")
  .map((h) => h.trim().toLowerCase())
  .filter(Boolean);

export function middleware(request: NextRequest) {
  const host = (request.headers.get("host") ?? "").split(":")[0].toLowerCase();

  if (host && host !== CANONICAL_HOST && ALIAS_HOSTS.includes(host)) {
    const target = new URL(request.nextUrl.pathname + request.nextUrl.search, SITE_URL);
    return NextResponse.redirect(target, 308);
  }

  return NextResponse.next();
}

export const config = {
  // Skip Next internals and static assets; run on pages + route handlers so the
  // alias hosts redirect everywhere a human or crawler might land.
  matcher: ["/((?!_next/static|_next/image|favicon.ico).*)"],
};
