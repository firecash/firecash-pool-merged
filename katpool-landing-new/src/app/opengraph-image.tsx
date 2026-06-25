import { ImageResponse } from "next/og";

export const alt = "Kat Pool - Kaspa Mining Pool";
export const size = { width: 1200, height: 630 };
export const contentType = "image/png";

export default function OgImage() {
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
              fontSize: 68,
              fontWeight: 700,
              lineHeight: 1.02,
              letterSpacing: "-0.03em",
              maxWidth: 920,
            }}
          >
            Mine Kaspa at the edge
          </div>
          <div style={{ marginTop: 20, fontSize: 28, color: "#9ccbc4", maxWidth: 820 }}>
            Global anycast stratum · PROP payouts · NACHO fee rebates
          </div>
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 12, fontSize: 22, color: "#70c7ba" }}>
          <span>kas.katpool.com</span>
          <span>·</span>
          <span>0.5% listed fee</span>
          <span>·</span>
          <span>7 regions</span>
        </div>
      </div>
    ),
    { ...size },
  );
}
