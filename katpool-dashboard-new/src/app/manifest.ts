import type { MetadataRoute } from "next";
import { POOL_NAME } from "@/lib/brand";

export default function manifest(): MetadataRoute.Manifest {
  return {
    name: `${POOL_NAME} — Kaspa Mining Pool Dashboard`,
    short_name: POOL_NAME,
    description:
      "Real-time Kaspa (KAS) mining pool analytics: hashrate, blocks, payouts, leaderboard and per-miner stats for katpool.",
    start_url: "/",
    display: "standalone",
    background_color: "#0b0e12",
    theme_color: "#0b0e12",
    categories: ["finance", "utilities", "productivity"],
    icons: [
      { src: "/icon-192.png", sizes: "192x192", type: "image/png" },
      { src: "/icon-512.png", sizes: "512x512", type: "image/png" },
      { src: "/icon-512-maskable.png", sizes: "512x512", type: "image/png", purpose: "maskable" },
    ],
  };
}
