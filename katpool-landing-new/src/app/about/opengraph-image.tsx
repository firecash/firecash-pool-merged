import { ogImage, ogSize, ogContentType } from "@/lib/og";

export const size = ogSize;
export const contentType = ogContentType;
export const alt = "About Kat Pool";

export default function Image() {
  return ogImage("About Kat Pool", "Open-source Kaspa mining pool from the NACHO ecosystem");
}
