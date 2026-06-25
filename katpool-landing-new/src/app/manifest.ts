import type { MetadataRoute } from "next";

export default function manifest(): MetadataRoute.Manifest {
  return {
    name: "Kat Pool — Kaspa Mining Pool",
    short_name: "Kat Pool",
    description:
      "Open-source Kaspa (KAS) mining pool. Global anycast stratum, transparent PROP (proportional) payouts, and NACHO fee rebates — effective fees as low as 0%.",
    start_url: "/",
    display: "standalone",
    background_color: "#060e11",
    theme_color: "#060e11",
    categories: ["finance", "utilities", "productivity"],
    icons: [
      { src: "/icon-192.png", sizes: "192x192", type: "image/png" },
      { src: "/icon-512.png", sizes: "512x512", type: "image/png" },
      { src: "/icon-512.png", sizes: "512x512", type: "image/png", purpose: "maskable" },
    ],
  };
}
