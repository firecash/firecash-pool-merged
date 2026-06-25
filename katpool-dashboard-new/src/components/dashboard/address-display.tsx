"use client";

import { ExternalLink } from "lucide-react";
import { truncateMiddle } from "@/lib/format";
import { explorerAddress } from "@/lib/explorer";
import { CopyButton } from "./copy-button";
import { cn } from "@/lib/utils";

/** Render an address as truncated mono text with copy + explorer affordances. */
export function AddressDisplay({
  address,
  className,
  full = false,
  link = true,
}: {
  address: string;
  className?: string;
  full?: boolean;
  link?: boolean;
}) {
  return (
    <span className={cn("inline-flex items-center gap-1 font-mono text-sm", className)}>
      <span title={address}>{full ? address : truncateMiddle(address)}</span>
      <CopyButton value={address} label="Copy address" />
      {link ? (
        <a
          href={explorerAddress(address)}
          target="_blank"
          rel="noopener noreferrer"
          aria-label="View on explorer"
          className="inline-flex size-8 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring sm:size-6"
        >
          <ExternalLink className="size-3.5" />
        </a>
      ) : null}
    </span>
  );
}
