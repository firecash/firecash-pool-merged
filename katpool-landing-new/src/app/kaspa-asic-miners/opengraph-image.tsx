import { ogImage, ogSize, ogContentType } from "@/lib/og";

export const size = ogSize;
export const contentType = ogContentType;
export const alt = "Kaspa ASIC miners — supported hardware & setup";

export default function Image() {
  return ogImage("Kaspa ASIC miners", "Supported hardware & setup — IceRiver, Bitmain, Goldshell");
}
