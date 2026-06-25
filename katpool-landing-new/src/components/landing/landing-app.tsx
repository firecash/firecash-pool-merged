"use client";

import { useCallback, useEffect, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import Image from "next/image";
import Link from "next/link";
import { ArrowRight, Github } from "lucide-react";
import type { MiningPoolStats } from "@/lib/pool-stats";
import { usePoolStats } from "@/hooks/use-pool-stats";
import { APP_URL, GITHUB_URL, TWITTER_URL } from "@/lib/mining";
import { XIcon } from "@/components/x-icon";
import { SceneNav } from "./scene-nav";
import { HeroScene } from "./scenes/hero-scene";
import { EdgeScene } from "./scenes/edge-scene";
import { ConnectScene } from "./scenes/connect-scene";
import { FeesScene } from "./scenes/fees-scene";
import { StartScene } from "./scenes/start-scene";

const SCENES = [
  { id: "hero", label: "Home" },
  { id: "edge", label: "Edge" },
  { id: "connect", label: "Connect" },
  { id: "fees", label: "Fees" },
  { id: "start", label: "Start" },
] as const;

type SceneId = (typeof SCENES)[number]["id"];

const sceneVariants = {
  enter: (dir: number) => ({
    opacity: 0,
    y: dir > 0 ? 48 : -48,
    scale: 0.98,
    filter: "blur(6px)",
  }),
  center: {
    opacity: 1,
    y: 0,
    scale: 1,
    filter: "blur(0px)",
  },
  exit: (dir: number) => ({
    opacity: 0,
    y: dir > 0 ? -48 : 48,
    scale: 0.98,
    filter: "blur(6px)",
  }),
};

export function LandingApp({ initialStats }: { initialStats: MiningPoolStats | null }) {
  const [index, setIndex] = useState(0);
  const [direction, setDirection] = useState(0);
  const { stats, syncing } = usePoolStats(initialStats);
  const [wheelLock, setWheelLock] = useState(false);

  const goTo = useCallback((next: number) => {
    if (next < 0 || next >= SCENES.length || next === index) return;
    setDirection(next > index ? 1 : -1);
    setIndex(next);
  }, [index]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "ArrowDown" || e.key === "ArrowRight" || e.key === "PageDown") {
        e.preventDefault();
        goTo(index + 1);
      } else if (e.key === "ArrowUp" || e.key === "ArrowLeft" || e.key === "PageUp") {
        e.preventDefault();
        goTo(index - 1);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [goTo, index]);

  useEffect(() => {
    let touchY = 0;
    const onTouchStart = (e: TouchEvent) => {
      touchY = e.touches[0]?.clientY ?? 0;
    };
    const onTouchEnd = (e: TouchEvent) => {
      const dy = (e.changedTouches[0]?.clientY ?? 0) - touchY;
      if (Math.abs(dy) < 48) return;

      const panel = document.querySelector("[data-scene-panel]");
      if (panel instanceof HTMLElement && panel.scrollHeight > panel.clientHeight + 8) {
        const atTop = panel.scrollTop <= 8;
        const atBottom = panel.scrollTop >= panel.scrollHeight - panel.clientHeight - 8;
        if (dy < 0 && !atBottom) return;
        if (dy > 0 && !atTop) return;
      }

      goTo(dy < 0 ? index + 1 : index - 1);
    };
    window.addEventListener("touchstart", onTouchStart, { passive: true });
    window.addEventListener("touchend", onTouchEnd, { passive: true });
    return () => {
      window.removeEventListener("touchstart", onTouchStart);
      window.removeEventListener("touchend", onTouchEnd);
    };
  }, [goTo, index]);

  useEffect(() => {
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      if (wheelLock) return;
      if (Math.abs(e.deltaY) < 12) return;
      setWheelLock(true);
      goTo(e.deltaY > 0 ? index + 1 : index - 1);
      window.setTimeout(() => setWheelLock(false), 700);
    };
    window.addEventListener("wheel", onWheel, { passive: false });
    return () => window.removeEventListener("wheel", onWheel);
  }, [goTo, index, wheelLock]);

  const sceneId: SceneId = SCENES[index].id;

  useEffect(() => {
    const panel = document.querySelector("[data-scene-panel]");
    if (panel instanceof HTMLElement) panel.scrollTop = 0;
  }, [sceneId]);

  return (
    <div className="landing-aurora relative h-[100dvh] w-full overflow-hidden bg-brand-bg text-foreground">
      {/* Ambient grid */}
      <div className="pointer-events-none absolute inset-0 opacity-[0.14]" aria-hidden>
        <div
          className="grid-drift absolute inset-[-48px] bg-[linear-gradient(oklch(1_0_0/6%)_1px,transparent_1px),linear-gradient(90deg,oklch(1_0_0/6%)_1px,transparent_1px)] bg-size-[48px_48px]"
        />
      </div>

      {/* Header - brand-bg matches the wordmark PNG plate (#060e11) */}
      <header className="absolute inset-x-0 top-0 z-40 border-b border-white/[0.06] bg-brand-bg">
        <div className="flex h-16 items-center justify-between px-5 sm:h-[4.5rem] sm:px-8">
          <Link href="/" className="flex items-center" aria-label="Kat Pool home">
            <Image
              src="/katpool-wordmark.png"
              alt="Kat Pool"
              width={2500}
              height={800}
              priority
              className="h-8 w-auto select-none sm:h-9"
            />
          </Link>
          <div className="flex items-center gap-2 sm:gap-3">
            <a
              href={TWITTER_URL}
              target="_blank"
              rel="noopener noreferrer"
              aria-label="Kat Pool on X"
              className="inline-flex size-9 items-center justify-center rounded-full border border-border text-muted-foreground transition hover:border-primary/40 hover:text-foreground"
            >
              <XIcon className="size-3.5" />
            </a>
            <a
              href={GITHUB_URL}
              target="_blank"
              rel="noopener noreferrer"
              className="hidden items-center gap-1.5 rounded-full border border-border px-3 py-1.5 text-xs text-muted-foreground transition hover:border-primary/40 hover:text-foreground sm:inline-flex"
            >
              <Github className="size-3.5" />
              Open source
            </a>
            <a
              href={APP_URL}
              className="inline-flex items-center gap-1.5 rounded-full bg-primary px-4 py-2 text-sm font-medium text-primary-foreground shadow-[0_0_40px_oklch(0.82_0.15_184/25%)] transition hover:brightness-110"
            >
              Dashboard
              <ArrowRight className="size-3.5" />
            </a>
          </div>
        </div>
      </header>

      {/* Scenes */}
      <main className="relative z-10 h-full">
        <AnimatePresence mode="wait" custom={direction}>
          <motion.div
            key={sceneId}
            custom={direction}
            variants={sceneVariants}
            initial="enter"
            animate="center"
            exit="exit"
            transition={{ duration: 0.55, ease: [0.22, 1, 0.36, 1] }}
            data-scene-panel
            className="absolute inset-0 flex w-full items-start justify-center overflow-x-hidden overflow-y-auto overscroll-y-contain px-4 pb-[5.5rem] pt-[4.25rem] [-webkit-overflow-scrolling:touch] sm:px-8 sm:pb-28 sm:pt-24 lg:items-center lg:overflow-hidden"
          >
            {sceneId === "hero" && (
              <HeroScene stats={stats} syncing={syncing} onNext={() => goTo(index + 1)} />
            )}
            {sceneId === "edge" && <EdgeScene />}
            {sceneId === "connect" && <ConnectScene />}
            {sceneId === "fees" && <FeesScene stats={stats} />}
            {sceneId === "start" && <StartScene />}
          </motion.div>
        </AnimatePresence>
      </main>

      <SceneNav scenes={SCENES} active={index} onSelect={goTo} />
    </div>
  );
}
