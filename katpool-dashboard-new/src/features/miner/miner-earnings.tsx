"use client";

import { Panel } from "@/components/dashboard/panel";
import { ErrorState, LoadingRows } from "@/components/dashboard/states";
import { useMinerProfile, useNetworkContext } from "@/lib/api/hooks";
import { ECOSYSTEM } from "@/lib/ecosystem";
import { ExtLink } from "@/components/ext-link";
import { formatKas, formatUsd, sompiToUsd } from "@/lib/format";
import type { KasAmount } from "@/lib/api/types";

function Line({
  label,
  amount,
  kasUsd,
  emphasize,
}: {
  label: string;
  amount: KasAmount;
  kasUsd: number | null;
  emphasize?: boolean;
}) {
  const usd = sompiToUsd(amount.sompi, kasUsd);
  return (
    <div className="flex items-center justify-between py-2.5">
      <span className="text-sm text-muted-foreground">{label}</span>
      <span className="text-right">
        <span className={emphasize ? "block text-sm font-semibold text-primary metric" : "block text-sm font-medium metric"}>
          {formatKas(amount.kas)}
        </span>
        {usd != null ? <span className="block text-xs text-muted-foreground tnum">{formatUsd(usd)}</span> : null}
      </span>
    </div>
  );
}

/** Full KAS + NACHO-rebate earnings breakdown for a miner. */
export function MinerEarnings({ address }: { address: string }) {
  const { data, isLoading, isError, refetch } = useMinerProfile(address);
  const network = useNetworkContext();
  const kasUsd = network.data?.prices.kas_usd ?? null;

  return (
    <Panel eyebrow="Earnings" title="Balance breakdown">
      {isError ? (
        <ErrorState onRetry={() => void refetch()} />
      ) : isLoading || !data ? (
        <LoadingRows rows={5} />
      ) : (
        <div className="grid grid-cols-1 gap-x-8 sm:grid-cols-2">
          <div>
            <p className="mb-1 text-[0.6875rem] font-medium uppercase tracking-[0.1em] text-muted-foreground">
              <ExtLink href={ECOSYSTEM.kaspa}>KAS</ExtLink>
            </p>
            <div className="divide-y divide-border/50">
              <Line label="Allocated" amount={data.kas.allocated} kasUsd={kasUsd} />
              <Line label="Paid" amount={data.kas.paid} kasUsd={kasUsd} />
              <Line label="Payable" amount={data.kas.payable} kasUsd={kasUsd} emphasize />
            </div>
          </div>
          <div>
            <p className="mb-1 text-[0.6875rem] font-medium uppercase tracking-[0.1em] text-muted-foreground">
              <ExtLink href={ECOSYSTEM.nacho}>NACHO</ExtLink> rebate (
              <ExtLink href={ECOSYSTEM.kaspa}>KAS</ExtLink> value)
            </p>
            <div className="divide-y divide-border/50">
              <Line label="Accrued" amount={data.nacho_rebate.accrued} kasUsd={kasUsd} />
              <Line label="Paid" amount={data.nacho_rebate.paid} kasUsd={kasUsd} />
              <Line label="Pending" amount={data.nacho_rebate.pending} kasUsd={kasUsd} emphasize />
            </div>
          </div>
        </div>
      )}
    </Panel>
  );
}
