"use client";

import { createContext, useCallback, useContext, useMemo, useRef, type ReactNode } from "react";

/**
 * Lets any component focus the global wallet search without prop-drilling.
 *
 * `WalletSearch` renders twice (desktop topbar + mobile row); each instance
 * registers a focuser that focuses itself only if it is currently visible.
 * `focusSearch()` invokes them until the visible one handles it, so the CTA
 * always lands on the search box the user can actually see.
 */
type Focuser = () => boolean;

interface SearchFocusValue {
  register: (focuser: Focuser) => () => void;
  focusSearch: () => void;
}

const NOOP: SearchFocusValue = {
  register: () => () => {},
  focusSearch: () => {},
};

const SearchFocusContext = createContext<SearchFocusValue | null>(null);

export function SearchFocusProvider({ children }: { children: ReactNode }) {
  const focusers = useRef<Set<Focuser>>(new Set());

  const register = useCallback((focuser: Focuser) => {
    focusers.current.add(focuser);
    return () => {
      focusers.current.delete(focuser);
    };
  }, []);

  const focusSearch = useCallback(() => {
    for (const focuser of focusers.current) {
      if (focuser()) return;
    }
  }, []);

  const value = useMemo(() => ({ register, focusSearch }), [register, focusSearch]);

  return <SearchFocusContext.Provider value={value}>{children}</SearchFocusContext.Provider>;
}

/** Access the search-focus controls; safely no-ops outside a provider. */
export function useSearchFocus(): SearchFocusValue {
  return useContext(SearchFocusContext) ?? NOOP;
}
