import { ogImage, ogSize, ogContentType } from "@/lib/og";

export const size = ogSize;
export const contentType = ogContentType;
export const alt = "Kat Pool live Kaspa stats";

export default function Image() {
  return ogImage("Kat Pool live stats", "Live Kaspa pool hashrate, blocks found & fee");
}
