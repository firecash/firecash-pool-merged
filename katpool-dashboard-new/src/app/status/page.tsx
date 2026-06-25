import type { Metadata } from "next";
import { PageHeader } from "@/components/dashboard/page-header";
import { StatusBoard } from "@/features/status/status-board";

export const metadata: Metadata = {
  title: "Status",
  description: "Live health of the katpool Kaspa pool API and upstream network data sources.",
  alternates: { canonical: "/status" },
};

export default function StatusPage() {
  return (
    <div className="space-y-6">
      <PageHeader title="Status" description="Live health of the pool API and network data sources." />
      <StatusBoard />
    </div>
  );
}
