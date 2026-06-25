"use client";

import { motion } from "framer-motion";
import Link from "next/link";
import { ArrowRight, ExternalLink, Github, Pickaxe } from "lucide-react";
import { APP_URL, GITHUB_URL, TWITTER_URL } from "@/lib/mining";
import { ECOSYSTEM } from "@/lib/ecosystem";
import { ExtLink } from "../ext-link";
import { XIcon } from "@/components/x-icon";

export function StartScene() {
  return (
    <div className="mx-auto flex w-full max-w-3xl flex-col items-center text-center">
      <motion.div
        initial={{ opacity: 0, scale: 0.9 }}
        animate={{ opacity: 1, scale: 1 }}
        transition={{ duration: 0.5 }}
        className="mb-6 flex size-16 items-center justify-center rounded-2xl bg-primary/15 ring-1 ring-primary/30"
      >
        <Pickaxe className="size-8 text-primary" />
      </motion.div>

      <motion.h2
        initial={{ opacity: 0, y: 16 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ delay: 0.06 }}
        className="text-3xl font-semibold tracking-tight sm:text-5xl"
      >
        Ready to{" "}
        <span className="text-grad">hash?</span>
      </motion.h2>

      <motion.p
        initial={{ opacity: 0, y: 12 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ delay: 0.12 }}
        className="mt-4 max-w-lg text-muted-foreground"
      >
        Open the dashboard for wallet stats, worker monitoring, and the full mining guide. The pool
        stack is open source - audit the code, run your own node, or just plug in and mine.
      </motion.p>

      <motion.div
        initial={{ opacity: 0, y: 12 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ delay: 0.18 }}
        className="mt-10 flex w-full max-w-md flex-col gap-3 sm:flex-row sm:justify-center"
      >
        <a
          href={APP_URL}
          className="inline-flex flex-1 items-center justify-center gap-2 rounded-full bg-primary px-6 py-3.5 text-sm font-medium text-primary-foreground shadow-[0_0_56px_oklch(0.82_0.15_184/35%)] transition hover:brightness-110"
        >
          Open dashboard
          <ArrowRight className="size-4" />
        </a>
        <a
          href={GITHUB_URL}
          target="_blank"
          rel="noopener noreferrer"
          className="inline-flex flex-1 items-center justify-center gap-2 rounded-full border border-border px-6 py-3.5 text-sm text-muted-foreground transition hover:border-primary/40 hover:text-foreground"
        >
          <Github className="size-4" />
          View source
          <ExternalLink className="size-3 opacity-60" />
        </a>
      </motion.div>

      <motion.a
        href={TWITTER_URL}
        target="_blank"
        rel="noopener noreferrer"
        initial={{ opacity: 0, y: 12 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ delay: 0.24 }}
        className="mt-5 inline-flex items-center gap-2 rounded-full border border-border px-4 py-2 text-sm text-muted-foreground transition hover:border-primary/40 hover:text-foreground"
      >
        <XIcon className="size-3.5" />
        Follow @Katpool_Mining for pool news
      </motion.a>

      <motion.nav
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        transition={{ delay: 0.32 }}
        className="mt-6 flex flex-wrap items-center justify-center gap-x-5 gap-y-2 text-sm text-muted-foreground"
        aria-label="Learn more"
      >
        <Link href="/kaspa-mining-pool" className="underline-offset-2 transition hover:text-primary hover:underline">
          Kaspa mining guide
        </Link>
        <span aria-hidden className="opacity-40">·</span>
        <Link href="/vs/humpool" className="underline-offset-2 transition hover:text-primary hover:underline">
          KatPool vs HumPool
        </Link>
      </motion.nav>

      <motion.p
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        transition={{ delay: 0.28 }}
        className="mt-12 text-xs text-muted-foreground"
      >
        Kat Pool · <ExtLink href={ECOSYSTEM.kaspa}>Kaspa</ExtLink> mainnet · Open source since day one
      </motion.p>
    </div>
  );
}
