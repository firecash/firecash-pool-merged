"use client";

import { useCallback, useEffect, useState } from "react";

/**
 * Client-side "this is my miner" preference, persisted in localStorage.
 *
 * There is no wallet auth, so a miner marks their own address via the
 * "Set as mine" toggle on the miner page; the sidebar MinerPulse then tracks
 * it. The value syncs across browser tabs (native `storage` event) and across
 * components in the *same* tab (a custom event, since `storage` does not fire
 * in the tab that wrote it).
 */
const KEY = "katpool:my-miner";
const SAME_TAB_EVENT = "katpool:my-miner-change";

function read(): string | null {
  if (typeof window === "undefined") return null;
  try {
    const v = window.localStorage.getItem(KEY);
    return v && v.length > 0 ? v : null;
  } catch {
    return null;
  }
}

export interface MyMiner {
  /** The saved address, or null when none is set. */
  address: string | null;
  /** False during SSR / before the first client read, to avoid hydration flashes. */
  hydrated: boolean;
  /** True once hydrated and the given address matches the saved one. */
  isMine: (address: string) => boolean;
  setMine: (address: string) => void;
  clear: () => void;
}

export function useMyMiner(): MyMiner {
  const [address, setAddress] = useState<string | null>(null);
  const [hydrated, setHydrated] = useState(false);

  useEffect(() => {
    setAddress(read());
    setHydrated(true);

    const sync = () => setAddress(read());
    const onStorage = (e: StorageEvent) => {
      if (e.key === KEY) sync();
    };
    window.addEventListener(SAME_TAB_EVENT, sync);
    window.addEventListener("storage", onStorage);
    return () => {
      window.removeEventListener(SAME_TAB_EVENT, sync);
      window.removeEventListener("storage", onStorage);
    };
  }, []);

  const setMine = useCallback((next: string) => {
    try {
      window.localStorage.setItem(KEY, next);
    } catch {
      /* storage unavailable (private mode / quota) — fall through to in-memory */
    }
    setAddress(next);
    window.dispatchEvent(new Event(SAME_TAB_EVENT));
  }, []);

  const clear = useCallback(() => {
    try {
      window.localStorage.removeItem(KEY);
    } catch {
      /* ignore */
    }
    setAddress(null);
    window.dispatchEvent(new Event(SAME_TAB_EVENT));
  }, []);

  const isMine = useCallback(
    (candidate: string) => hydrated && address === candidate,
    [hydrated, address],
  );

  return { address, hydrated, isMine, setMine, clear };
}
