"use client";

import { useEffect } from "react";
import { AlertTriangle, RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/button";

export default function Error({ error, reset }: { error: Error & { digest?: string }; reset: () => void }) {
  useEffect(() => {
    console.error(error);
  }, [error]);

  return (
    <div className="flex min-h-[60vh] flex-col items-center justify-center gap-5 text-center">
      <span className="flex size-14 items-center justify-center rounded-2xl bg-warning/10 text-warning">
        <AlertTriangle className="size-7" />
      </span>
      <div>
        <p className="text-2xl font-semibold tracking-tight">Something went wrong</p>
        <p className="mt-2 max-w-md text-sm text-muted-foreground">
          An unexpected error occurred while rendering this page. You can try again.
        </p>
      </div>
      <Button onClick={reset}>
        <RefreshCw className="size-4" /> Try again
      </Button>
    </div>
  );
}
