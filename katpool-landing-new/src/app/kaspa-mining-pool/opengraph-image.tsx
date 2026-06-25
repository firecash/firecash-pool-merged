import { ogImage, ogSize, ogContentType } from "@/lib/og";

export const size = ogSize;
export const contentType = ogContentType;
export const alt = "How to mine Kaspa on Kat Pool";

export default function Image() {
  return ogImage("How to mine Kaspa (KAS)", "Fees, PROP rewards, NACHO rebates & setup in two minutes");
}
