"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { ArrowRight } from "lucide-react";
import { NAV_ITEMS } from "./nav";
import { cn } from "@/lib/utils";

/** Vertical nav links with active highlighting (shared desktop + mobile). */
export function SidebarNav({ onNavigate }: { onNavigate?: () => void }) {
  const pathname = usePathname();

  return (
    <nav className="flex flex-col gap-1">
      {NAV_ITEMS.map((item) => {
        const active = item.exact
          ? pathname === item.href
          : pathname === item.href || pathname.startsWith(`${item.href}/`);
        const Icon = item.icon;

        if (item.cta) {
          return (
            <Link
              key={item.href}
              href={item.href}
              onClick={onNavigate}
              aria-current={active ? "page" : undefined}
              className={cn(
                "group mb-2 flex items-center gap-2.5 rounded-lg border px-3 py-2 text-sm font-semibold transition-all duration-200",
                active
                  ? "border-primary/60 bg-primary text-primary-foreground shadow-[0_0_18px_-2px_var(--primary)]"
                  : "border-primary/40 bg-primary/10 text-primary hover:bg-primary/15 hover:shadow-[0_0_18px_-4px_var(--primary)]",
              )}
            >
              <Icon className="size-4 shrink-0" />
              {item.label}
              <ArrowRight className="ml-auto size-3.5 transition-transform duration-200 group-hover:translate-x-0.5" />
            </Link>
          );
        }

        return (
          <Link
            key={item.href}
            href={item.href}
            onClick={onNavigate}
            aria-current={active ? "page" : undefined}
            className={cn(
              "group relative flex items-center gap-3 rounded-lg px-3 py-2 text-sm font-medium transition-all duration-200",
              active
                ? "bg-gradient-to-r from-primary/[0.14] to-primary/[0.03] text-foreground"
                : "text-muted-foreground hover:bg-muted/70 hover:text-foreground",
            )}
          >
            {active ? (
              <span className="absolute inset-y-1.5 left-0 w-0.5 rounded-full bg-primary shadow-[0_0_8px_var(--primary)]" />
            ) : null}
            <Icon
              className={cn(
                "size-4 transition-colors",
                active ? "text-primary" : "text-muted-foreground group-hover:text-foreground",
              )}
            />
            {item.label}
          </Link>
        );
      })}
    </nav>
  );
}
