import type { Metadata } from "next";
import { PageHeader } from "@/components/dashboard/page-header";
import { CyclesTable } from "@/features/payouts/cycles-table";
import { PayoutFlow } from "@/features/payouts/payout-flow";
import { PayoutsSummary } from "@/features/overview/payouts-summary";
import { ECOSYSTEM } from "@/lib/ecosystem";
import { ExtLink } from "@/components/ext-link";

export const metadata: Metadata = {
  title: "Payouts",
  description:
    "Kaspa (KAS) and NACHO payout cycles for katpool — distribution history, amounts and treasury position, paid automatically from a 10 KAS minimum.",
  alternates: { canonical: "/payouts" },
};

export default function PayoutsPage() {
  return (
    <div className="space-y-6">
      <PageHeader
        title="Payouts"
        description={
          <>
            <ExtLink href={ECOSYSTEM.kaspa}>KAS</ExtLink> and{" "}
            <ExtLink href={ECOSYSTEM.nacho}>NACHO</ExtLink> distribution cycles and treasury position.
          </>
        }
      />
      <div className="grid grid-cols-1 items-stretch gap-6 lg:grid-cols-3">
        <div className="lg:col-span-2">
          <PayoutFlow />
        </div>
        <PayoutsSummary />
      </div>
      <CyclesTable />
    </div>
  );
}
