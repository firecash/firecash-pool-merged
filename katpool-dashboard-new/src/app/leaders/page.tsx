import type { Metadata } from "next";
import Link from "next/link";
import { ArrowRight, Gauge, Layers, Trophy } from "lucide-react";
import { PageHeader } from "@/components/dashboard/page-header";
import { LeaderboardTable } from "@/features/leaders/leaderboard-table";
import { Card } from "@/components/ui/card";
import { Button } from "@/components/ui/button";

export const metadata: Metadata = {
  title: "Leaderboard",
  description:
    "The top miners on katpool ranked by estimated Kaspa hashrate and pool share over your selected window.",
  alternates: { canonical: "/leaders" },
};

export default function LeadersPage() {
  return (
    <div className="space-y-6">
      <PageHeader
        title="Leaderboard"
        description="The pool's top contributing miners, ranked by hashrate over your selected window."
      />
      <LeaderboardTable limit={100} />

      <div className="grid grid-cols-1 gap-6 lg:grid-cols-3">
        <Card className="p-5 lg:col-span-2">
          <h3 className="text-sm font-semibold tracking-tight">How ranking works</h3>
          <ul className="mt-3 space-y-3 text-sm text-muted-foreground">
            <li className="flex items-start gap-3">
              <Gauge className="mt-0.5 size-4 shrink-0 text-primary" />
              <span>
                Miners are ranked by{" "}
                <span className="text-foreground">estimated hashrate</span> over the selected window,
                derived from accepted share difficulty — not raw share count.
              </span>
            </li>
            <li className="flex items-start gap-3">
              <Layers className="mt-0.5 size-4 shrink-0 text-primary" />
              <span>
                <span className="text-foreground">Pool share</span> is each miner&apos;s portion of total
                pool hashrate in that window. Switch the range to compare short bursts against
                sustained contribution.
              </span>
            </li>
            <li className="flex items-start gap-3">
              <Trophy className="mt-0.5 size-4 shrink-0 text-primary" />
              <span>
                Addresses are shown truncated for privacy. Select any miner to open its full live
                dashboard.
              </span>
            </li>
          </ul>
        </Card>

        <Card className="relative flex flex-col justify-between gap-4 overflow-hidden p-5">
          <div className="pointer-events-none absolute -right-16 -top-16 size-44 rounded-full bg-primary/15 blur-3xl" />
          <div className="relative">
            <h3 className="text-base font-semibold tracking-tight">Climb the leaderboard</h3>
            <p className="mt-1 text-sm text-muted-foreground">
              Point a rig at katpool and your address appears here within a minute.
            </p>
          </div>
          <Button asChild className="relative w-full">
            <Link href="/start">
              Start mining <ArrowRight className="size-4" />
            </Link>
          </Button>
        </Card>
      </div>
    </div>
  );
}
