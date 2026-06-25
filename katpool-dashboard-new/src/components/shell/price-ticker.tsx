"use client";

import { useNetworkContext } from "@/lib/api/hooks";
import { ECOSYSTEM } from "@/lib/ecosystem";
import { ExtLink } from "@/components/ext-link";
import { formatUsdPrice } from "@/lib/format";
import { DeltaChip } from "@/components/dashboard/delta-chip";
import { Skeleton } from "@/components/ui/skeleton";

function PricePill({
  label,
  href,
  price,
  change,
}: {
  label: string;
  href: string;
  price: number;
  change: number | null | undefined;
}) {
  return (
    <div className="hidden items-center gap-2 rounded-full border border-border bg-muted/30 px-3 py-1 sm:flex">
      <ExtLink href={href} className="text-xs font-medium text-muted-foreground">
        {label}
      </ExtLink>
      <span className="text-sm font-semibold tnum">{formatUsdPrice(price)}</span>
      <DeltaChip value={change} />
    </div>
  );
}

/** Live KAS + NACHO prices with 24h change for the top bar. */
export function PriceTicker() {
  const { data, isLoading } = useNetworkContext();
  const kas = data?.prices.kas_usd ?? null;
  const nacho = data?.prices.nacho_usd ?? null;

  if (isLoading) {
    return (
      <div className="hidden items-center gap-2 sm:flex">
        <Skeleton className="h-7 w-28 rounded-full" />
        <Skeleton className="hidden h-7 w-28 rounded-full md:block" />
      </div>
    );
  }
  if (kas == null && nacho == null) return null;

  return (
    <div className="flex items-center gap-2">
      {kas != null ? (
        <PricePill
          label="KAS"
          href={ECOSYSTEM.kaspa}
          price={kas}
          change={data?.prices.kas_change_24h}
        />
      ) : null}
      {nacho != null ? (
        <PricePill
          label="NACHO"
          href={ECOSYSTEM.nacho}
          price={nacho}
          change={data?.prices.nacho_change_24h}
        />
      ) : null}
    </div>
  );
}
