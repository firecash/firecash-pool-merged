"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { NAV_ITEMS } from "./nav";
import { cn } from "@/lib/utils";

/** Fixed bottom navigation for mobile (primary destinations only). */
export function BottomNav() {
  const pathname = usePathname();
  // Quick-access bar: the CTA plus the destinations miners hit most.
  // Status stays one tap away in the drawer menu.
  const HIDDEN = new Set(["/status"]);
  const items = NAV_ITEMS.filter((i) => !HIDDEN.has(i.href)).slice(0, 5);

  return (
    <nav className="glass fixed inset-x-0 bottom-0 z-40 flex items-center justify-around border-t border-border px-2 py-1.5 lg:hidden">
      {items.map((item) => {
        const active = item.exact
          ? pathname === item.href
          : pathname === item.href || pathname.startsWith(`${item.href}/`);
        const Icon = item.icon;
        return (
          <Link
            key={item.href}
            href={item.href}
            aria-current={active ? "page" : undefined}
            className={cn(
              "flex flex-1 flex-col items-center gap-0.5 rounded-md py-1 text-[10px] font-medium transition-colors",
              active || item.cta ? "text-primary" : "text-muted-foreground",
            )}
          >
            {/* Reserved-height bar so the active marker doesn't shift layout
                and state isn't communicated by colour alone. */}
            <span
              className={cn(
                "h-0.5 w-5 rounded-full transition-colors",
                active ? "bg-primary" : "bg-transparent",
              )}
            />
            <Icon className="size-5" />
            {item.shortLabel ?? item.label.split(" ")[0]}
          </Link>
        );
      })}
    </nav>
  );
}
