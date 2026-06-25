"use client";

import { useEffect, useRef } from "react";
import { usePathname } from "next/navigation";
import { useQueryClient } from "@tanstack/react-query";

/** Invalidate live queries when navigating between dashboard routes. */
export function LiveQuerySync() {
  const pathname = usePathname();
  const client = useQueryClient();
  const prev = useRef(pathname);

  useEffect(() => {
    if (prev.current === pathname) return;
    prev.current = pathname;
    void client.invalidateQueries();
  }, [pathname, client]);

  return null;
}
