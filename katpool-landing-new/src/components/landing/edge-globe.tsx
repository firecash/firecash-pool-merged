"use client";

import { useEffect, useRef } from "react";
import createGlobe from "cobe";
import { EDGE_REGIONS, MINT, POOL_ORIGIN, TEAL } from "@/lib/edge-regions";

export function EdgeGlobe({ className }: { className?: string }) {
  const rootRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const pointerInteracting = useRef<number | null>(null);
  const movementRef = useRef(0);
  const phiRef = useRef(2.35);
  const widthRef = useRef(0);

  useEffect(() => {
    const root = rootRef.current;
    const canvas = canvasRef.current;
    if (!root || !canvas) return;

    const measure = () => {
      widthRef.current = root.offsetWidth;
    };
    measure();

    const observer = new ResizeObserver(measure);
    observer.observe(root);

    const origin = POOL_ORIGIN.location;

    const globe = createGlobe(canvas, {
      devicePixelRatio: Math.min(window.devicePixelRatio, 2),
      width: widthRef.current * 2,
      height: widthRef.current * 2,
      phi: phiRef.current,
      theta: 0.24,
      dark: 1,
      diffuse: 1.7,
      mapSamples: widthRef.current < 240 ? 12000 : 28000,
      mapBrightness: 4.8,
      mapBaseBrightness: 0.035,
      baseColor: [0.1, 0.16, 0.18],
      markerColor: TEAL,
      glowColor: TEAL,
      markerElevation: 0.045,
      scale: 1.05,
      opacity: 0.96,
      markers: [
        { location: origin, size: 0.08, color: MINT },
        ...EDGE_REGIONS.map((r) => ({
          location: r.location,
          size: 0.048,
          color: TEAL,
        })),
      ],
      arcs: EDGE_REGIONS.map((r) => ({
        from: origin,
        to: r.location,
        color: TEAL,
      })),
      arcColor: TEAL,
      arcWidth: 0.38,
      arcHeight: 0.24,
    });

    let frame = 0;
    const tick = () => {
      if (pointerInteracting.current === null) {
        phiRef.current += 0.002;
      }
      const w = widthRef.current;
      globe.update({
        phi: phiRef.current + movementRef.current,
        width: w * 2,
        height: w * 2,
        mapSamples: w < 240 ? 12000 : 28000,
      });
      frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);

    requestAnimationFrame(() => canvas.classList.add("is-ready"));

    return () => {
      cancelAnimationFrame(frame);
      globe.destroy();
      observer.disconnect();
    };
  }, []);

  function onPointerDown(e: React.PointerEvent<HTMLCanvasElement>) {
    e.stopPropagation();
    pointerInteracting.current = e.clientX - movementRef.current;
    e.currentTarget.setPointerCapture(e.pointerId);
  }

  function onPointerUp(e: React.PointerEvent<HTMLCanvasElement>) {
    pointerInteracting.current = null;
    e.currentTarget.releasePointerCapture(e.pointerId);
  }

  function onPointerMove(e: React.PointerEvent<HTMLCanvasElement>) {
    if (pointerInteracting.current !== null) {
      movementRef.current = (e.clientX - pointerInteracting.current) / 160;
    }
  }

  return (
    <div
      ref={rootRef}
      className={`relative mx-auto aspect-square w-full ${className ?? ""}`}
    >
      <div
        className="pointer-events-none absolute inset-[6%] rounded-full"
        style={{
          background:
            "radial-gradient(circle, oklch(0.82 0.15 184 / 24%) 0%, oklch(0.83 0.14 78 / 10%) 38%, transparent 66%)",
        }}
      />
      <div
        className="pointer-events-none absolute inset-[14%] rounded-full"
        style={{
          boxShadow:
            "0 0 100px oklch(0.82 0.15 184 / 20%), inset 0 0 60px oklch(0.82 0.15 184 / 5%)",
        }}
      />

      <canvas
        ref={canvasRef}
        className="relative z-10 size-full cursor-grab touch-none opacity-0 transition-opacity duration-1000 active:cursor-grabbing [&.is-ready]:opacity-100"
        onPointerDown={onPointerDown}
        onPointerUp={onPointerUp}
        onPointerOut={onPointerUp}
        onPointerMove={onPointerMove}
      />
    </div>
  );
}
