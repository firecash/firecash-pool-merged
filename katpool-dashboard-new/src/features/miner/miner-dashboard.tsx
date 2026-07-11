"use client";

import Link from "next/link";
import { SearchX } from "lucide-react";
import { Card } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { ErrorState } from "@/components/dashboard/states";
import { DashboardApiError } from "@/lib/api/client";
import { useMinerProfile } from "@/lib/api/hooks";
import { MinerHeader } from "./miner-header";
import { MinerKpis } from "./miner-kpis";
import { MinerHashrate } from "./miner-hashrate";
import { WorkersTable } from "./workers-table";
import { WorkerDistribution } from "./worker-distribution";
import { MinerEarnings } from "./miner-earnings";
import { RejectsPanel } from "./rejects-panel";
import { MinerPayouts } from "./miner-payouts";

/** Full per-miner dashboard, gated on the profile lookup. */
export function MinerDashboard({ address }: { address: string }) {
  const profile = useMinerProfile(address);

  if (profile.isLoading) {
    return (
      <div className="space-y-6">
        <Skeleton className="h-10 w-64" />
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-3">
          {Array.from({ length: 6 }).map((_, i) => (
            <Skeleton key={i} className="h-32 w-full rounded-2xl" />
          ))}
        </div>
      </div>
    );
  }

  if (profile.isError) {
    const notFound =
      profile.error instanceof DashboardApiError && profile.error.status === 404;
    if (notFound) {
      return (
        <Card className="flex flex-col items-center gap-4 p-10 text-center">
          <span className="flex size-12 items-center justify-center rounded-xl bg-muted text-muted-foreground">
            <SearchX className="size-6" />
          </span>
          <div>
            <h2 className="text-lg font-semibold">Miner not found</h2>
            <p className="mt-1 max-w-sm text-sm text-muted-foreground">
              We have no record of this address mining on the pool. Check the address and try again.
            </p>
          </div>
          <Button asChild variant="outline">
            <Link href="/">Back to overview</Link>
          </Button>
        </Card>
      );
    }
    return <ErrorState message={profile.error.message} onRetry={() => void profile.refetch()} />;
  }

  return (
    <div className="space-y-6">
      <MinerHeader address={address} />
      <MinerKpis address={address} />
      <MinerHashrate address={address} />
      <div className="grid grid-cols-1 items-stretch gap-6 xl:grid-cols-3">
        <div className="xl:col-span-2">
          <WorkersTable address={address} />
        </div>
        <WorkerDistribution address={address} />
      </div>
      <div className="grid grid-cols-1 items-stretch gap-6 lg:grid-cols-2">
        <MinerEarnings address={address} />
        <RejectsPanel address={address} />
      </div>
      <MinerPayouts address={address} />
    </div>
  );
}
