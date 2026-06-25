import { ImageResponse } from "next/og";

/**
 * Shared 1200x630 OpenGraph image renderer for content pages, matching the
 * root `opengraph-image` styling. Each route's `opengraph-image.tsx` is a thin
 * wrapper that calls `ogImage(title, subtitle)` so social/SERP cards are
 * page-specific without duplicating the layout.
 */
export const ogSize = { width: 1200, height: 630 };
export const ogContentType = "image/png";

export function ogImage(title: string, subtitle: string) {
  return new ImageResponse(
    (
      <div
        style={{
          width: "100%",
          height: "100%",
          display: "flex",
          flexDirection: "column",
          justifyContent: "space-between",
          padding: "64px 72px",
          background: "linear-gradient(145deg, #060e11 0%, #0b1a1f 45%, #102820 100%)",
          color: "#e8f7f4",
          fontFamily: "ui-sans-serif, system-ui, sans-serif",
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: 16 }}>
          <div
            style={{
              width: 56,
              height: 56,
              borderRadius: 16,
              background: "linear-gradient(135deg, #70c7ba, #49eacb)",
            }}
          />
          <div style={{ fontSize: 36, fontWeight: 700, letterSpacing: "-0.02em" }}>Kat Pool</div>
        </div>
        <div style={{ display: "flex", flexDirection: "column" }}>
          <div
            style={{
              fontSize: 64,
              fontWeight: 700,
              lineHeight: 1.04,
              letterSpacing: "-0.03em",
              maxWidth: 980,
            }}
          >
            {title}
          </div>
          <div style={{ marginTop: 20, fontSize: 28, color: "#9ccbc4", maxWidth: 900 }}>{subtitle}</div>
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 12, fontSize: 22, color: "#70c7ba" }}>
          <span>katpool.com</span>
          <span>·</span>
          <span>Open-source Kaspa mining pool</span>
        </div>
      </div>
    ),
    { ...ogSize },
  );
}
