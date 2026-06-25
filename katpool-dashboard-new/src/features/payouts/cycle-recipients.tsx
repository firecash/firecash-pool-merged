"use client";

import Link from "next/link";
import { ExternalLink } from "lucide-react";
import { ErrorState, LoadingRows } from "@/components/dashboard/states";
import { usePayoutCycle } from "@/lib/api/hooks";
import type { PayoutKind } from "@/lib/api/types";
import { formatKas, formatNacho, truncateMiddle } from "@/lib/format";
import { explorerTx } from "@/lib/explorer";
import { cn } from "@/lib/utils";

export function CycleRecipients({ cycleId, kind }: { cycleId: number; kind: PayoutKind }) {
  const { data, isLoading, isError, refetch } = usePayoutCycle(cycleId);
  const recipients = data?.recipients ?? [];

  if (isError) {
    return <ErrorState onRetry={() => void refetch()} />;
  }
  if (isLoading) {
    return <LoadingRows rows={4} />;
  }
  if (recipients.length === 0) {
    return <p className="text-sm text-muted-foreground">No recipient rows recorded for this cycle.</p>;
  }

  return (
    <div className="overflow-x-auto rounded-xl border border-border/50 bg-card/40">
      <table className="w-full min-w-[640px] text-sm" aria-label={`Cycle ${cycleId} recipients`}>
        <thead>
          <tr className="border-b border-border/60 text-left text-xs uppercase tracking-wide text-muted-foreground">
            <th className="px-4 py-2.5 font-medium">Miner</th>
            <th className="px-4 py-2.5 text-right font-medium">
              {kind === "nacho" ? "Rebate (KAS)" : "Amount"}
            </th>
            {kind === "nacho" ? (
              <th className="px-4 py-2.5 text-right font-medium">NACHO sent</th>
            ) : null}
            <th className="px-4 py-2.5 font-medium">Status</th>
            <th className="px-4 py-2.5 font-medium">Tx</th>
          </tr>
        </thead>
        <tbody>
          {recipients.map((r) => {
            const tx = r.tx_hash ?? r.krc20_reveal_hash ?? r.krc20_commit_hash;
            return (
              <tr key={r.payout_id} className="border-b border-border/40 last:border-0">
                <td className="px-4 py-2.5">
                  <Link
                    href={`/miner/${encodeURIComponent(r.address)}`}
                    className="font-mono text-xs text-primary hover:underline"
                    onClick={(e) => e.stopPropagation()}
                  >
                    {truncateMiddle(r.address, 18)}
                  </Link>
                </td>
                <td className="px-4 py-2.5 text-right font-medium tnum">{formatKas(r.amount.kas)}</td>
                {kind === "nacho" ? (
                  <td className="px-4 py-2.5 text-right tnum text-secondary">
                    {r.nacho_amount ? formatNacho(r.nacho_amount) : "—"}
                  </td>
                ) : null}
                <td className="px-4 py-2.5">
                  <span
                    className={cn(
                      "rounded-full px-2 py-0.5 text-[10px] uppercase tracking-wide",
                      r.status === "confirmed"
                        ? "bg-success/15 text-success"
                        : "bg-muted text-muted-foreground",
                    )}
                  >
                    {r.status}
                  </span>
                </td>
                <td className="px-4 py-2.5">
                  {tx ? (
                    <a
                      href={explorerTx(tx)}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="inline-flex items-center gap-1 font-mono text-xs text-primary hover:underline"
                      onClick={(e) => e.stopPropagation()}
                    >
                      {tx.slice(0, 8)}… <ExternalLink className="size-3" />
                    </a>
                  ) : (
                    <span className="text-muted-foreground">—</span>
                  )}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
