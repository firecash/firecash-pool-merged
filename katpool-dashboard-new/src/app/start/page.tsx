import type { Metadata } from "next";
import { PageHeader } from "@/components/dashboard/page-header";
import { StartGuide } from "@/features/start/start-guide";

export const metadata: Metadata = {
  title: "Start Mining",
  description:
    "Connect your rig to the katpool Kaspa pool in under two minutes — stratum host, ports, difficulty and payout settings for every ASIC.",
  alternates: { canonical: "/start" },
};

export default function StartPage() {
  return (
    <div className="space-y-6">
      <PageHeader
        title="Start mining"
        description="Everything you need to point a rig at katpool and start earning."
      />
      <StartGuide />
    </div>
  );
}
