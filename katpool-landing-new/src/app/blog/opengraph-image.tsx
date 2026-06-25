import { ogImage, ogSize, ogContentType } from "@/lib/og";

export const size = ogSize;
export const contentType = ogContentType;
export const alt = "Kat Pool Blog";

export default function Image() {
  return ogImage("Kat Pool blog", "Kaspa mining guides, comparisons & insights");
}
