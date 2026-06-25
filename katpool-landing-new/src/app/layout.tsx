import type { Metadata, Viewport } from "next";
import { Geist, Geist_Mono } from "next/font/google";
import "./globals.css";
import { JsonLd } from "@/components/json-ld";
import { organizationLd, websiteLd } from "@/lib/structured-data";

const geistSans = Geist({
  variable: "--font-geist-sans",
  subsets: ["latin"],
});

const geistMono = Geist_Mono({
  variable: "--font-geist-mono",
  subsets: ["latin"],
});

const TITLE = "Kat Pool — Open-Source Kaspa (KAS) Mining Pool · Lowest Fees";
const DESCRIPTION =
  "Mine Kaspa (KAS) on Kat Pool — the open-source pool with global anycast stratum across 7 regions, transparent PROP (proportional) payouts, and NACHO fee rebates that cut the effective fee to as low as 0%. The transparent HumPool alternative.";

export const metadata: Metadata = {
  metadataBase: new URL(process.env.NEXT_PUBLIC_SITE_URL ?? "https://katpool.com"),
  title: TITLE,
  description: DESCRIPTION,
  applicationName: "Kat Pool",
  category: "technology",
  keywords: [
    "Kaspa mining pool",
    "best Kaspa mining pool",
    "open source Kaspa pool",
    "lowest fee Kaspa pool",
    "Kaspa",
    "KAS",
    "KAS mining",
    "mine Kaspa",
    "katpool",
    "Kat Pool",
    "NACHO",
    "NACHO rebate",
    "PROP payouts",
    "proportional rewards",
    "stratum",
    "kHeavyHash",
    "IceRiver KS5",
    "Bitmain KS5",
    "HumPool alternative",
    "ASIC mining",
    "crypto mining pool",
  ],
  authors: [{ name: "Kat Pool", url: "https://katpool.com" }],
  creator: "Kat Pool",
  publisher: "Kat Pool",
  icons: {
    icon: "/katpool-icon.png",
    apple: "/icon-192.png",
  },
  openGraph: {
    title: TITLE,
    description:
      "Open-source Kaspa mining pool. Global anycast stratum, transparent PROP payouts, NACHO rebates — effective fees as low as 0%.",
    siteName: "Kat Pool",
    type: "website",
    locale: "en_US",
    url: "/",
  },
  twitter: {
    card: "summary_large_image",
    title: TITLE,
    description:
      "Open-source Kaspa mining pool. Global stratum, transparent PROP payouts, NACHO rebates — effective fees as low as 0%.",
    site: "@Katpool_Mining",
    creator: "@Katpool_Mining",
  },
  robots: {
    index: true,
    follow: true,
    googleBot: { index: true, follow: true, "max-image-preview": "large", "max-snippet": -1 },
  },
  alternates: { canonical: "/" },
};

export const viewport: Viewport = {
  themeColor: "#060e11",
  colorScheme: "dark",
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" className="dark">
      <body className={`${geistSans.variable} ${geistMono.variable} font-sans antialiased`}>
        <JsonLd data={[organizationLd(), websiteLd()]} />
        {children}
      </body>
    </html>
  );
}
