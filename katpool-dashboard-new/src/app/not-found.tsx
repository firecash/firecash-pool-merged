import Link from "next/link";
import { Compass } from "lucide-react";
import { Button } from "@/components/ui/button";

export default function NotFound() {
  return (
    <div className="flex min-h-[60vh] flex-col items-center justify-center gap-5 text-center">
      <span className="flex size-14 items-center justify-center rounded-2xl bg-primary/10 text-primary">
        <Compass className="size-7" />
      </span>
      <div>
        <p className="text-5xl font-semibold tracking-tight">404</p>
        <p className="mt-2 text-muted-foreground">This page drifted off the DAG.</p>
      </div>
      <Button asChild>
        <Link href="/">Back to overview</Link>
      </Button>
    </div>
  );
}
