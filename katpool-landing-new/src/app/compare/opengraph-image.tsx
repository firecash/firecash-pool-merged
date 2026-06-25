import { ogImage, ogSize, ogContentType } from "@/lib/og";

export const size = ogSize;
export const contentType = ogContentType;
export const alt = "Best Kaspa mining pools compared";

export default function Image() {
  return ogImage("Best Kaspa mining pools compared", "Fees, reward schemes & open-source status — 2026");
}
