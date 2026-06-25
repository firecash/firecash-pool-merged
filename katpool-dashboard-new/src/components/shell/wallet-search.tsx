"use client";

import { useRouter } from "next/navigation";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { Search } from "lucide-react";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import { useSearchFocus } from "./search-focus";

/** Global wallet search; routes to the miner page on submit. */
export function WalletSearch({ className }: { className?: string }) {
  const router = useRouter();
  const [value, setValue] = useState("");
  const [pinged, setPinged] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const { register } = useSearchFocus();

  useEffect(
    () =>
      register(() => {
        const el = inputRef.current;
        // offsetParent is null when the element (or an ancestor) is
        // `display:none` — i.e. the responsively-hidden instance. Skip it so
        // focus lands on the search box the user can actually see.
        if (!el || el.offsetParent === null) return false;
        el.focus();
        el.scrollIntoView({ block: "center", behavior: "smooth" });
        setPinged(true);
        window.setTimeout(() => setPinged(false), 1200);
        return true;
      }),
    [register],
  );

  function onSubmit(e: FormEvent) {
    e.preventDefault();
    const addr = value.trim();
    if (!addr) return;
    router.push(`/miners/${encodeURIComponent(addr)}`);
  }

  return (
    <form onSubmit={onSubmit} className={cn("relative w-full max-w-md", className)}>
      <Search className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
      <Input
        ref={inputRef}
        value={value}
        onChange={(e) => setValue(e.target.value)}
        placeholder="Search wallet address (kaspa:…)"
        spellCheck={false}
        autoComplete="off"
        className={cn(
          "pl-9 transition-shadow",
          pinged && "ring-2 ring-primary/70 ring-offset-2 ring-offset-background",
        )}
        aria-label="Search wallet address"
      />
    </form>
  );
}
