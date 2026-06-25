"use client";

import { BadgeCheck } from "lucide-react";
import { AddressDisplay } from "@/components/dashboard/address-display";
import { Badge } from "@/components/ui/badge";
import { Skeleton } from "@/components/ui/skeleton";
import { useFullRebate, useMinerProfile } from "@/lib/api/hooks";
import { formatDateTime, formatRelative } from "@/lib/format";
import { SetAsMineToggle } from "./set-as-mine-toggle";

/** Miner page header: address, network, rebate tier, first/last seen. */
export function MinerHeader({ address }: { address: string }) {
  const { data, isLoading } = useMinerProfile(address);
  const rebate = useFullRebate(address);

  return (
    <div className="flex flex-wrap items-start justify-between gap-4">
      <div className="min-w-0 space-y-2">
        <div className="flex flex-wrap items-center gap-2">
          <h1 className="text-2xl font-semibold tracking-tight">Miner</h1>
          {data ? <Badge variant="outline">{data.network}</Badge> : null}
          {rebate.data?.full_rebate ? (
            <Badge variant="success">
              <BadgeCheck className="size-3" /> Elite rebate
            </Badge>
          ) : rebate.data?.tier === "standard" ? (
            <Badge variant="secondary">Standard rebate</Badge>
          ) : null}
        </div>
        <AddressDisplay address={address} full className="break-all text-sm" />
      </div>
      <div className="flex flex-col items-end gap-2">
        <SetAsMineToggle address={address} />
        <div className="text-right text-xs text-muted-foreground">
          {isLoading || !data ? (
            <Skeleton className="h-4 w-32" />
          ) : (
            <>
              <p>
                First seen{" "}
                <span title={formatDateTime(data.first_seen_at)}>{formatRelative(data.first_seen_at)}</span>
              </p>
              <p>
                Last seen{" "}
                <span title={formatDateTime(data.last_seen_at)}>{formatRelative(data.last_seen_at)}</span>
              </p>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
