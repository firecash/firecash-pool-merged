"use client";

import { Badge, type BadgeProps } from "@/components/ui/badge";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";
import type { PayoutStatus } from "@/lib/api/types";

type Variant = NonNullable<BadgeProps["variant"]>;

interface Meta {
  label: string;
  variant: Variant;
  hint: string;
  /** Pulsing dot while the payout is still working its way on-chain. */
  pulse?: boolean;
}

/**
 * Friendly, self-explanatory labels for a per-miner payout. The raw API states
 * (`submitted`/`accepted`) are jargon; miners care about "is it on its way" vs
 * "is it paid". Each badge carries a plain-language tooltip.
 */
const META: Record<PayoutStatus, Meta> = {
  planned: {
    label: "Scheduled",
    variant: "outline",
    hint: "Queued for the next distribution cycle.",
  },
  submitted: {
    label: "Broadcasting",
    variant: "secondary",
    hint: "Payout transaction has been broadcast to the network and is awaiting acceptance.",
    pulse: true,
  },
  accepted: {
    label: "Confirming",
    variant: "warning",
    hint: "Accepted on-chain — maturing to final confirmation depth.",
    pulse: true,
  },
  confirmed: {
    label: "Paid",
    variant: "success",
    hint: "Confirmed on-chain. Funds have been delivered to your wallet.",
  },
  failed: {
    label: "Failed",
    variant: "destructive",
    hint: "This payout could not be completed; the amount is refunded into a future cycle.",
  },
};

export function PayoutStatusBadge({
  status,
  reason,
}: {
  status: PayoutStatus;
  reason?: string | null;
}) {
  const meta = META[status] ?? META.planned;
  const hint = status === "failed" && reason ? `${meta.hint} Reason: ${reason}` : meta.hint;

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Badge variant={meta.variant} className="cursor-default">
          {meta.pulse ? (
            <span className="relative mr-0.5 flex size-1.5">
              <span className="absolute inline-flex size-full animate-ping rounded-full bg-current opacity-75" />
              <span className="relative inline-flex size-1.5 rounded-full bg-current" />
            </span>
          ) : null}
          {meta.label}
        </Badge>
      </TooltipTrigger>
      <TooltipContent className={cn("max-w-[16rem]")}>{hint}</TooltipContent>
    </Tooltip>
  );
}
