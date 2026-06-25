import { ogImage, ogSize, ogContentType } from "@/lib/og";

export const size = ogSize;
export const contentType = ogContentType;
export const alt = "Kat Pool vs HumPool";

export default function Image() {
  return ogImage("Kat Pool vs HumPool", "The open-source, lower-fee HumPool alternative for Kaspa");
}
