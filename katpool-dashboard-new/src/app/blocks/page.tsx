import type { Metadata } from "next";
import { PageHeader } from "@/components/dashboard/page-header";
import { BlocksTable } from "@/features/blocks/blocks-table";
import { BlocksSummary } from "@/features/overview/blocks-summary";

export const metadata: Metadata = {
  title: "Blocks",
  description:
    "Every Kaspa block katpool has found, with live confirmation and maturity status, reward and timestamp.",
  alternates: { canonical: "/blocks" },
};

export default function BlocksPage() {
  return (
    <div className="space-y-6">
      <PageHeader title="Blocks" description="Every block the pool has found, with live lifecycle status." />
      <div className="grid grid-cols-1 items-start gap-6 lg:grid-cols-3">
        <div className="lg:col-span-2">
          <BlocksTable />
        </div>
        <div className="lg:sticky lg:top-20">
          <BlocksSummary />
        </div>
      </div>
    </div>
  );
}
