import { cn } from "@/lib/utils";

/** A shimmering placeholder used for loading states (no layout shift). */
function Skeleton({ className, ...props }: React.HTMLAttributes<HTMLDivElement>) {
  return <div className={cn("skeleton rounded-md", className)} {...props} />;
}

export { Skeleton };
