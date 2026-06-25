import { ogImage, ogSize, ogContentType } from "@/lib/og";

export const size = ogSize;
export const contentType = ogContentType;
export const alt = "Kaspa mining calculator";

export default function Image() {
  return ogImage("Kaspa mining calculator", "Estimate daily KAS earnings with live network data");
}
