"use client";

import { Fragment, useState } from "react";
import { ChevronDown, ChevronLeft, ChevronRight } from "lucide-react";
import { Panel } from "@/components/dashboard/panel";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { EmptyState, ErrorState, LoadingRows } from "@/components/dashboard/states";
import { LiveBadge } from "@/components/dashboard/live-badge";
import { CycleStatusBadge } from "./cycle-status-badge";
import { CycleRecipients } from "./cycle-recipients";
import { usePayoutCycles } from "@/lib/api/hooks";
import { ECOSYSTEM } from "@/lib/ecosystem";
import { ExtLink } from "@/components/ext-link";
import { useHighlightNew } from "@/lib/use-highlight-new";
import { formatDateTime, formatKas, formatNumber, formatRelative } from "@/lib/format";
import { cn } from "@/lib/utils";

const PAGE = 25;

/** Paginated table of payout cycles with expandable recipient breakdown. */
export function CyclesTable() {
  const [stack, setStack] = useState<number[]>([]);
  const [expandedId, setExpandedId] = useState<number | null>(null);
  const before = stack[stack.length - 1];
  const { data, isLoading, isError, refetch, dataUpdatedAt, isFetching } = usePayoutCycles(PAGE, before);

  const cycles = (data?.cycles ?? []).filter((c) => c.status !== "planned");
  const onFirstPage = stack.length === 0;
  const fresh = useHighlightNew(onFirstPage ? cycles.map((c) => c.id) : []);

  return (
    <Panel
      title="Payout cycles"
      description={
        <>
          <ExtLink href={ECOSYSTEM.kaspa}>KAS</ExtLink> and{" "}
          <ExtLink href={ECOSYSTEM.nacho}>NACHO</ExtLink> distribution cycles — expand a row to see every
          recipient
        </>
      }
      actions={
        <div className="flex items-center gap-2">
          <LiveBadge updatedAt={dataUpdatedAt} isFetching={isFetching} className="mr-1 hidden sm:inline-flex" />
          <Button
            variant="outline"
            size="icon"
            aria-label="Previous page"
            disabled={stack.length === 0}
            onClick={() => {
              setExpandedId(null);
              setStack((s) => s.slice(0, -1));
            }}
          >
            <ChevronLeft className="size-4" />
          </Button>
          <Button
            variant="outline"
            size="icon"
            aria-label="Next page"
            disabled={data?.next_before == null}
            onClick={() => {
              setExpandedId(null);
              if (data?.next_before != null) {
                setStack((s) => [...s, data.next_before!]);
              }
            }}
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
          <LoadingRows rows={8} />
        </div>
      ) : cycles.length === 0 ? (
        <div className="p-5">
          <EmptyState title="No payout cycles yet" description="Distribution cycles appear here once the pool settles rewards." />
        </div>
      ) : (
        <div className="overflow-x-auto">
          <table className="w-full min-w-[680px] text-sm" aria-label="Payout cycles">
            <thead>
              <tr className="border-b border-border text-left text-xs uppercase tracking-wide text-muted-foreground">
                <th className="w-8 px-3 py-3 sm:px-5" />
                <th className="px-3 py-3 sm:px-5 font-medium">Cycle</th>
                <th className="px-3 py-3 sm:px-5 font-medium">Asset</th>
                <th className="px-3 py-3 sm:px-5 font-medium">Status</th>
                <th className="px-3 py-3 sm:px-5 text-right font-medium">Recipients</th>
                <th className="px-3 py-3 sm:px-5 text-right font-medium">Total</th>
                <th className="px-3 py-3 sm:px-5 text-right font-medium">When</th>
              </tr>
            </thead>
            <tbody>
              {cycles.map((c) => {
                const expanded = expandedId === c.id;
                return (
                  <Fragment key={c.id}>
                    <tr
                      className={cn(
                        "cursor-pointer border-b border-border/60 transition-colors hover:bg-muted/40",
                        fresh.has(c.id) && "row-flash",
                        expanded && "bg-muted/25",
                      )}
                      onClick={() => setExpandedId(expanded ? null : c.id)}
                      aria-expanded={expanded}
                    >
                      <td className="px-3 py-3 sm:px-5 text-muted-foreground">
                        <ChevronDown
                          className={cn("size-4 transition-transform", expanded && "rotate-180")}
                        />
                      </td>
                      <td className="px-3 py-3 sm:px-5 font-mono text-xs text-muted-foreground">#{c.id}</td>
                      <td className="px-3 py-3 sm:px-5">
                        <span className="inline-flex items-center gap-2">
                          <span
                            className={cn(
                              "size-1.5 rounded-full",
                              c.kind === "kas" ? "bg-primary" : "bg-secondary",
                            )}
                          />
                          <Badge variant={c.kind === "kas" ? "default" : "secondary"}>
                            {c.kind === "kas" ? "KAS" : "NACHO"}
                          </Badge>
                        </span>
                      </td>
                      <td className="px-3 py-3 sm:px-5">
                        <CycleStatusBadge status={c.status} />
                      </td>
                      <td className="px-3 py-3 sm:px-5 text-right tnum">
                        {c.total_recipients > 0 ? (
                          formatNumber(c.total_recipients)
                        ) : (
                          <span className="text-muted-foreground">—</span>
                        )}
                      </td>
                      <td className="px-3 py-3 sm:px-5 text-right">
                        <span className="block font-medium tnum">{formatKas(c.total.kas)}</span>
                        {c.kind === "nacho" ? (
                          <span className="block text-xs text-muted-foreground">KAS rebate value</span>
                        ) : null}
                      </td>
                      <td
                        className="px-3 py-3 sm:px-5 text-right text-muted-foreground"
                        title={formatDateTime(c.settled_at ?? c.planned_at)}
                      >
                        {formatRelative(c.settled_at ?? c.planned_at)}
                      </td>
                    </tr>
                    {expanded ? (
                      <tr className="border-b border-border/60 bg-muted/15">
                        <td colSpan={7} className="px-3 py-4 sm:px-5">
                          <CycleRecipients cycleId={c.id} kind={c.kind} />
                        </td>
                      </tr>
                    ) : null}
                  </Fragment>
                );
              })}
            </tbody>
          </table>
        </div>
      )}
    </Panel>
  );
}
