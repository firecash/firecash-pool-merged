import type { ReactNode } from "react";
import Image from "next/image";
import Link from "next/link";
import { ArrowRight, Github } from "lucide-react";
import { APP_URL, GITHUB_URL, TWITTER_URL } from "@/lib/mining";
import { XIcon } from "@/components/x-icon";

/**
 * Server-rendered chrome for static content/SEO pages (guide, comparison).
 *
 * The global `body` is `overflow:hidden` for the scroll-jack landing SPA, so
 * content pages scroll inside their own full-height container — the same
 * pattern the SPA's scene panel uses — rather than mutating global CSS.
 */
export function PageShell({ children }: { children: ReactNode }) {
  return (
    <div className="landing-aurora h-[100dvh] w-full overflow-y-auto overflow-x-hidden bg-brand-bg text-foreground [-webkit-overflow-scrolling:touch]">
      <header className="sticky top-0 z-40 border-b border-white/[0.06] bg-brand-bg/95 backdrop-blur">
        <div className="mx-auto flex h-16 max-w-5xl items-center justify-between px-5 sm:h-[4.5rem] sm:px-6">
          <Link href="/" className="flex items-center" aria-label="Kat Pool home">
            <Image
              src="/katpool-wordmark.png"
              alt="Kat Pool — Kaspa mining pool"
              width={2500}
              height={800}
              priority
              className="h-8 w-auto select-none sm:h-9"
            />
          </Link>
          <div className="flex items-center gap-2 sm:gap-3">
            <a
              href={TWITTER_URL}
              target="_blank"
              rel="noopener noreferrer"
              aria-label="Kat Pool on X"
              className="inline-flex size-9 items-center justify-center rounded-full border border-border text-muted-foreground transition hover:border-primary/40 hover:text-foreground"
            >
              <XIcon className="size-3.5" />
            </a>
            <a
              href={APP_URL}
              className="inline-flex items-center gap-1.5 rounded-full bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:brightness-110"
            >
              Open dashboard
              <ArrowRight className="size-3.5" />
            </a>
          </div>
        </div>
      </header>

      <main className="mx-auto max-w-3xl px-5 py-12 sm:px-6 sm:py-16">{children}</main>

      <footer className="border-t border-white/[0.06] bg-brand-bg/80">
        <div className="mx-auto flex max-w-5xl flex-col gap-4 px-5 py-10 sm:px-6 sm:flex-row sm:items-center sm:justify-between">
          <nav className="flex flex-wrap gap-x-5 gap-y-2 text-sm text-muted-foreground">
            <Link href="/" className="transition hover:text-foreground">
              Home
            </Link>
            <Link href="/kaspa-mining-pool" className="transition hover:text-foreground">
              Mining guide
            </Link>
            <Link href="/compare" className="transition hover:text-foreground">
              Compare pools
            </Link>
            <Link href="/kaspa-mining-calculator" className="transition hover:text-foreground">
              Calculator
            </Link>
            <Link href="/kaspa-asic-miners" className="transition hover:text-foreground">
              ASIC miners
            </Link>
            <Link href="/vs/humpool" className="transition hover:text-foreground">
              vs HumPool
            </Link>
            <Link href="/stats" className="transition hover:text-foreground">
              Live stats
            </Link>
            <Link href="/blog" className="transition hover:text-foreground">
              Blog
            </Link>
            <Link href="/about" className="transition hover:text-foreground">
              About
            </Link>
            <a href={APP_URL} className="transition hover:text-foreground">
              Dashboard
            </a>
          </nav>
          <div className="flex items-center gap-3 text-muted-foreground">
            <a
              href={TWITTER_URL}
              target="_blank"
              rel="noopener noreferrer"
              aria-label="Kat Pool on X"
              className="transition hover:text-foreground"
            >
              <XIcon className="size-4" />
            </a>
            <a
              href={GITHUB_URL}
              target="_blank"
              rel="noopener noreferrer"
              aria-label="Kat Pool on GitHub"
              className="transition hover:text-foreground"
            >
              <Github className="size-4" />
            </a>
            <span className="text-xs">Open source · Kaspa mainnet</span>
          </div>
        </div>
      </footer>
    </div>
  );
}
