import { Github } from "lucide-react";
import { Brand } from "./brand";
import { SidebarNav } from "./sidebar-nav";
import { PoolPulse } from "./pool-pulse";
import { MinerPulse } from "./miner-pulse";
import { XIcon } from "@/components/x-icon";
import { GITHUB_URL, TWITTER_URL, SITE_URL } from "@/lib/brand";

const RESOURCE_LINKS: { label: string; href: string }[] = [
  { label: "Mining guide", href: `${SITE_URL}/kaspa-mining-pool` },
  { label: "Compare pools", href: `${SITE_URL}/compare` },
  { label: "Mining calculator", href: `${SITE_URL}/kaspa-mining-calculator` },
  { label: "Blog", href: `${SITE_URL}/blog` },
];

/** Fixed desktop sidebar (lg+). */
export function Sidebar() {
  return (
    <aside className="dark sticky top-0 hidden h-screen w-64 shrink-0 flex-col border-r border-border bg-brand-bg px-4 py-5 lg:flex">
      <Brand className="px-1.5" />
      <div className="mt-8 flex-1">
        <p className="px-3 pb-2 text-[0.6875rem] font-medium uppercase tracking-[0.12em] text-muted-foreground/70">
          Navigation
        </p>
        <SidebarNav />
      </div>
      <PoolPulse />
      <MinerPulse />
      <nav aria-label="Resources" className="mt-4 px-1.5">
        <p className="px-1.5 pb-1.5 text-[0.6875rem] font-medium uppercase tracking-[0.12em] text-muted-foreground/70">
          Resources
        </p>
        <ul className="space-y-0.5">
          {RESOURCE_LINKS.map((item) => (
            <li key={item.href}>
              <a
                href={item.href}
                className="block rounded-lg px-1.5 py-1 text-xs text-muted-foreground transition hover:text-foreground"
              >
                {item.label}
              </a>
            </li>
          ))}
        </ul>
      </nav>
      <div className="mt-4 flex items-center gap-2 px-1.5">
        <a
          href={TWITTER_URL}
          target="_blank"
          rel="noopener noreferrer"
          aria-label="Kat Pool on X"
          className="inline-flex size-8 items-center justify-center rounded-lg border border-border text-muted-foreground transition hover:border-primary/40 hover:text-foreground"
        >
          <XIcon className="size-3.5" />
        </a>
        <a
          href={GITHUB_URL}
          target="_blank"
          rel="noopener noreferrer"
          aria-label="Kat Pool on GitHub — open source"
          className="inline-flex size-8 items-center justify-center rounded-lg border border-border text-muted-foreground transition hover:border-primary/40 hover:text-foreground"
        >
          <Github className="size-3.5" />
        </a>
      </div>
    </aside>
  );
}
