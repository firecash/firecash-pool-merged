"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import { AnimatePresence, motion, useReducedMotion } from "framer-motion";
import { Blocks, ExternalLink, Sparkles } from "lucide-react";
import { Panel } from "@/components/dashboard/panel";
import { EmptyState, ErrorState, LoadingRows } from "@/components/dashboard/states";
import { BlockStatusBadge } from "./block-status-badge";
import { useBlocks } from "@/lib/api/hooks";
import { LiveRelative } from "@/components/live-relative";
import { formatDateTime, formatNumber, truncateMiddle } from "@/lib/format";
import { explorerBlock } from "@/lib/explorer";

const FEED_SIZE = 7;
/** Don't fire the celebratory burst more than once per this window (ms). */
const CELEBRATION_COOLDOWN = 30_000;

/** A short, tasteful particle burst when the pool solves a block. */
function BlockBurst({ trigger }: { trigger: number }) {
  const reduce = useReducedMotion();
  const [on, setOn] = useState(false);

  useEffect(() => {
    if (trigger === 0 || reduce) return;
    setOn(true);
    const id = setTimeout(() => setOn(false), 1200);
    return () => clearTimeout(id);
  }, [trigger, reduce]);

  const particles = useMemo(() => {
    void trigger; // re-roll the burst geometry on each new block
    const palette = ["var(--primary)", "var(--secondary)", "var(--chart-3)"];
    return Array.from({ length: 16 }, (_, i) => {
      const angle = (Math.PI * 2 * i) / 16 + Math.random() * 0.4;
      const dist = 34 + Math.random() * 46;
      return {
        x: Math.cos(angle) * dist,
        y: Math.sin(angle) * dist,
        size: 4 + Math.random() * 4,
        color: palette[i % palette.length],
      };
    });
  }, [trigger]);

  if (!on) return null;
  return (
    <div className="pointer-events-none absolute left-1/2 top-9 z-20 -translate-x-1/2">
      {particles.map((p, i) => (
        <motion.span
          key={i}
          className="absolute block rounded-full"
          style={{ width: p.size, height: p.size, backgroundColor: p.color }}
          initial={{ x: 0, y: 0, opacity: 1, scale: 1 }}
          animate={{ x: p.x, y: p.y, opacity: 0, scale: 0.3 }}
          transition={{ duration: 1.1, ease: "easeOut" }}
        />
      ))}
    </div>
  );
}

/**
 * A real-time feed of the latest pool blocks. New blocks slide in with a
 * brief glow; solving a block triggers a tasteful, rate-limited burst — a
 * delight on mainnet, never spam on a fast testnet.
 */
export function LiveBlockFeed() {
  const { data, isLoading, isError, refetch } = useBlocks(FEED_SIZE);
  const blocks = useMemo(() => data?.blocks ?? [], [data]);

  const lastIdRef = useRef<number | null>(null);
  const lastCelebRef = useRef(0);
  const [flash, setFlash] = useState(false);
  const [burst, setBurst] = useState(0);

  useEffect(() => {
    const top = blocks[0];
    if (!top) return;
    if (lastIdRef.current === null) {
      lastIdRef.current = top.id; // first load — don't celebrate history
      return;
    }
    if (top.id !== lastIdRef.current) {
      lastIdRef.current = top.id;
      setFlash(true);
      const now = Date.now();
      if (now - lastCelebRef.current > CELEBRATION_COOLDOWN) {
        lastCelebRef.current = now;
        setBurst((b) => b + 1);
      }
      const t = setTimeout(() => setFlash(false), 1600);
      return () => clearTimeout(t);
    }
  }, [blocks]);

  return (
    <Panel
      eyebrow="Real-time"
      title="Live blocks"
      description={`The ${FEED_SIZE} most recent blocks the pool has found`}
      actions={
        <span className="inline-flex items-center gap-1.5 text-xs font-medium text-muted-foreground">
          <span className="size-2 rounded-full bg-success live-dot" />
          Live
        </span>
      }
      bodyClassName="relative p-0"
    >
      <BlockBurst trigger={burst} />

      <AnimatePresence>
        {flash ? (
          <motion.div
            initial={{ opacity: 0, y: -6 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -6 }}
            className="absolute right-5 top-3 z-10 inline-flex items-center gap-1.5 rounded-full border border-primary/40 bg-primary/15 px-2.5 py-1 text-xs font-semibold text-primary shadow-[var(--shadow-glow)]"
          >
            <Sparkles className="size-3.5" />
            Block found
          </motion.div>
        ) : null}
      </AnimatePresence>

      {isError ? (
        <div className="p-5">
          <ErrorState onRetry={() => void refetch()} />
        </div>
      ) : isLoading ? (
        <div className="p-5">
          <LoadingRows rows={FEED_SIZE} />
        </div>
      ) : blocks.length === 0 ? (
        <EmptyState
          icon={<Blocks className="size-6" />}
          title="Awaiting the first block"
          description="Every block the pool solves will stream in here the instant it's found."
        />
      ) : (
        <ul className="divide-y divide-border/50">
          <AnimatePresence initial={false}>
            {blocks.map((b, i) => (
              <motion.li
                key={b.id}
                layout
                initial={{ opacity: 0, height: 0 }}
                animate={{ opacity: 1, height: "auto" }}
                exit={{ opacity: 0, height: 0 }}
                transition={{ duration: 0.35, ease: [0.22, 1, 0.36, 1] }}
                className={
                  i === 0 && flash
                    ? "bg-primary/[0.06] transition-colors"
                    : "transition-colors hover:bg-muted/30"
                }
              >
                <div className="flex items-center gap-3 px-5 py-3">
                  <BlockStatusBadge status={b.status} />
                  <a
                    href={explorerBlock(b.hash)}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="group inline-flex min-w-0 items-center gap-1.5 font-mono text-xs hover:text-primary"
                  >
                    <span className="truncate">{truncateMiddle(b.hash, 10, 8)}</span>
                    <ExternalLink className="size-3 shrink-0 text-muted-foreground group-hover:text-primary" />
                  </a>
                  <div className="ml-auto flex shrink-0 items-center gap-4 text-xs">
                    <span className="hidden text-muted-foreground tabular-nums sm:inline">
                      DAA {formatNumber(b.daa_score)}
                    </span>
                    <LiveRelative
                      at={b.found_at}
                      className="text-muted-foreground"
                      title={formatDateTime(b.found_at)}
                    />
                  </div>
                </div>
              </motion.li>
            ))}
          </AnimatePresence>
        </ul>
      )}
    </Panel>
  );
}
