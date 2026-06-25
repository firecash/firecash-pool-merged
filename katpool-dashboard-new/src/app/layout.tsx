import type { Metadata, Viewport } from "next";
import { Geist, Geist_Mono } from "next/font/google";
import "./globals.css";
import { Providers } from "@/components/providers";
import { TooltipProvider } from "@/components/ui/tooltip";
import { AppShell } from "@/components/shell/app-shell";
import { JsonLd } from "@/components/json-ld";
import { APP_ORIGIN, POOL_NAME, TWITTER_HANDLE } from "@/lib/brand";
import { organizationLd, webApplicationLd, websiteLd } from "@/lib/structured-data";

const geistSans = Geist({ variable: "--font-geist-sans", subsets: ["latin"] });
const geistMono = Geist_Mono({ variable: "--font-geist-mono", subsets: ["latin"] });

const DESCRIPTION =
  "Live Kaspa (KAS) mining pool analytics for katpool: real-time pool and network hashrate, blocks found, payout cycles, miner leaderboard and per-wallet worker stats. Open source, NACHO rebates, lowest effective fees.";

export const metadata: Metadata = {
  metadataBase: new URL(APP_ORIGIN),
  title: {
    default: `${POOL_NAME} — Kaspa Mining Pool Dashboard`,
    template: `%s · ${POOL_NAME}`,
  },
  description: DESCRIPTION,
  applicationName: POOL_NAME,
  category: "technology",
  keywords: [
    "Kaspa mining pool",
    "Kaspa pool dashboard",
    "Kaspa",
    "KAS",
    "KAS hashrate",
    "mining pool",
    "katpool",
    "Kat Pool",
    "NACHO",
    "PROP payouts",
    "stratum",
    "hashrate",
    "Kaspa blocks",
    "Kaspa payouts",
    "HumPool alternative",
    "crypto mining",
  ],
  authors: [{ name: "Kat Pool" }],
  creator: "Kat Pool",
  publisher: "Kat Pool",
  openGraph: {
    title: `${POOL_NAME} — Kaspa Mining Pool Dashboard`,
    description: "Real-time pool analytics: hashrate, blocks, payouts, leaderboard and miner insights.",
    siteName: POOL_NAME,
    type: "website",
    url: "/",
    locale: "en_US",
  },
  twitter: {
    card: "summary_large_image",
    title: `${POOL_NAME} — Kaspa Mining Pool Dashboard`,
    description: "Real-time pool analytics: hashrate, blocks, payouts, leaderboard and miner insights.",
    site: TWITTER_HANDLE,
    creator: TWITTER_HANDLE,
  },
  robots: {
    index: true,
    follow: true,
    googleBot: { index: true, follow: true, "max-image-preview": "large", "max-snippet": -1 },
  },
};

export const viewport: Viewport = {
  themeColor: [
    { media: "(prefers-color-scheme: dark)", color: "#0b0e12" },
    { media: "(prefers-color-scheme: light)", color: "#fafbfc" },
  ],
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" suppressHydrationWarning>
      <body className={`${geistSans.variable} ${geistMono.variable} font-sans antialiased`}>
        <JsonLd data={[organizationLd(), websiteLd(), webApplicationLd()]} />
        <Providers>
          <TooltipProvider delayDuration={150}>
            <AppShell>{children}</AppShell>
          </TooltipProvider>
        </Providers>
      </body>
    </html>
  );
}
