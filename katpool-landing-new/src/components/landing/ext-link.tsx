import type { ReactNode } from "react";
import { extLinkClass } from "@/lib/ecosystem";
import { cn } from "@/lib/utils";

/** Subtle external link — inherits surrounding color until hover. */
export function ExtLink({
  href,
  children,
  className,
}: {
  href: string;
  children: ReactNode;
  className?: string;
}) {
  return (
    <a
      href={href}
      target="_blank"
      rel="noopener noreferrer"
      className={cn(extLinkClass, className)}
    >
      {children}
    </a>
  );
}
