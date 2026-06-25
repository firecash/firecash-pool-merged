"use client";

import { useState } from "react";
import { ChevronLeft, ChevronRight, ExternalLink } from "lucide-react";
import { Panel } from "@/components/dashboard/panel";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { EmptyState, ErrorState, LoadingRows } from "@/components/dashboard/states";
import { LiveBadge } from "@/components/dashboard/live-badge";
import { useMinerPayouts, useNetworkContext } from "@/lib/api/hooks";
import { useHighlightNew } from "@/lib/use-highlight-new";
import { formatDateTime, formatKas, formatNacho, formatRelative, formatUsd, sompiToUsd } from "@/lib/format";
import { explorerTx } from "@/lib/explorer";
import { cn } from "@/lib/utils";
import { PayoutStatusBadge } from "./payout-status-badge";

const PAGE = 25;

/** Per-miner payout history (keyset-paginated). */
export function MinerPayouts({ address }: { address: string }) {
  const [stack, setStack] = useState<number[]>([]);
  const before = stack[stack.length - 1];
  const { data, isLoading, isError, refetch, dataUpdatedAt, isFetching } = useMinerPayouts(address, PAGE, before);
  const network = useNetworkContext();
  const kasUsd = network.data?.prices.kas_usd ?? null;
  // "Planned" is an internal pre-broadcast bookkeeping state with no on-chain
  // action yet; hide it (matching the pool cycles ledger) and show payouts only
  // once they actually broadcast.
  const payouts = (data?.payouts ?? []).filter((p) => p.status !== "planned");
  const onFirstPage = stack.length === 0;
  const fresh = useHighlightNew(onFirstPage ? payouts.map((p) => p.id) : []);

  return (
    <Panel
      title="Payout history"
      description="Your KAS and NACHO payouts, newest first"
      actions={
        <div className="flex items-center gap-2">
          <LiveBadge updatedAt={dataUpdatedAt} isFetching={isFetching} className="mr-1 hidden sm:inline-flex" />
          <Button variant="outline" size="icon" aria-label="Previous page" disabled={stack.length === 0} onClick={() => setStack((s) => s.slice(0, -1))}>
            <ChevronLeft className="size-4" />
          </Button>
          <Button
            variant="outline"
            size="icon"
            aria-label="Next page"
            disabled={data?.next_before == null}
            onClick={() => data?.next_before != null && setStack((s) => [...s, data.next_before!])}
          >
            <ChevronRight className="size-4" />
          </Button>
        </div>
      }
      bodyClassName="p-0"
    >
      {isError ? (
        <div className="p-5">
          <ErrorState onRetry={() => void refetch()} />
        </div>
      ) : isLoading ? (
        <div className="p-5">
          <LoadingRows rows={6} />
        </div>
      ) : payouts.length === 0 ? (
        <div className="p-5">
          <EmptyState title="No payouts yet" description="Payouts appear here once you reach the threshold and a cycle settles." />
        </div>
      ) : (
        <div className="overflow-x-auto">
          <table className="w-full min-w-[640px] text-sm" aria-label="Payout history">
            <thead>
              <tr className="border-b border-border text-left text-xs uppercase tracking-wide text-muted-foreground">
                <th className="px-3 py-3 sm:px-5 font-medium">Asset</th>
                <th className="px-3 py-3 sm:px-5 text-right font-medium">Amount</th>
                <th className="px-3 py-3 sm:px-5 font-medium">Status</th>
                <th className="px-3 py-3 sm:px-5 font-medium">Tx</th>
                <th className="px-3 py-3 sm:px-5 text-right font-medium">When</th>
              </tr>
            </thead>
            <tbody>
              {payouts.map((p) => {
                const tx = p.tx_hash ?? p.krc20_reveal_hash ?? p.krc20_commit_hash;
                const usd = kasUsd != null ? sompiToUsd(p.amount.sompi, kasUsd) : null;
                const when = p.confirmed_at ?? p.submitted_at ?? p.planned_at;
                return (
                  <tr
                    key={p.id}
                    className={cn(
                      "border-b border-border/60 transition-colors hover:bg-muted/40",
                      fresh.has(p.id) && "row-flash",
                    )}
                  >
                    <td className="px-3 py-3 sm:px-5">
                      <span className="inline-flex items-center gap-2">
                        <span
                          className={cn(
                            "size-1.5 rounded-full",
                            p.kind === "kas" ? "bg-primary" : "bg-secondary",
                          )}
                        />
                        <Badge variant={p.kind === "kas" ? "default" : "secondary"}>
                          {p.kind === "kas" ? "KAS" : "NACHO"}
                        </Badge>
                      </span>
                    </td>
                    <td className="px-3 py-3 sm:px-5 text-right">
                      {p.kind === "nacho" && p.nacho_amount ? (
                        <>
                          <span className="block font-medium tnum text-secondary">
                            {formatNacho(p.nacho_amount)}
                          </span>
                          <span className="block text-xs text-muted-foreground tnum">
                            {formatKas(p.amount.kas)} rebate value
                          </span>
                        </>
                      ) : (
                        <>
                          <span className="block font-medium tnum">{formatKas(p.amount.kas)}</span>
                          {usd != null ? (
                            <span className="block text-xs text-muted-foreground tnum">{formatUsd(usd)}</span>
                          ) : null}
                        </>
                      )}
                    </td>
                    <td className="px-3 py-3 sm:px-5">
                      <PayoutStatusBadge status={p.status} reason={p.failure_reason} />
                    </td>
                    <td className="px-3 py-3 sm:px-5">
                      {tx ? (
                        <a
                          href={explorerTx(tx)}
                          target="_blank"
                          rel="noopener noreferrer"
                          className="inline-flex items-center gap-1 font-mono text-xs text-primary hover:underline"
                        >
                          {tx.slice(0, 8)}… <ExternalLink className="size-3" />
                        </a>
                      ) : (
                        <span className="text-muted-foreground">—</span>
                      )}
                    </td>
                    <td
                      className="px-3 py-3 sm:px-5 text-right text-muted-foreground"
                      title={formatDateTime(when)}
                    >
                      {formatRelative(when)}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}
    </Panel>
  );
}
