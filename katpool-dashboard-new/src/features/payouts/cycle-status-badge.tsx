import { Badge } from "@/components/ui/badge";
import type { CycleStatus } from "@/lib/api/types";

const LABELS: Record<CycleStatus, { label: string; variant: "default" | "secondary" | "success" | "warning" | "destructive" | "outline" }> = {
  planned: { label: "Planned", variant: "outline" },
  broadcasting: { label: "Broadcasting", variant: "secondary" },
  partially_settled: { label: "Partial", variant: "warning" },
  settled: { label: "Settled", variant: "success" },
  failed: { label: "Failed", variant: "destructive" },
};

/** A colored badge for a payout-cycle status. */
export function CycleStatusBadge({ status }: { status: CycleStatus }) {
  const meta = LABELS[status];
  return <Badge variant={meta.variant}>{meta.label}</Badge>;
}
