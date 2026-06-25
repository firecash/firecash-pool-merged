"use client";

import { type ReactNode } from "react";
import { Reveal } from "./reveal";
import { Card } from "@/components/ui/card";
import { cn } from "@/lib/utils";

/** A titled content panel with an optional actions slot (range toggles etc.). */
export function Panel({
  title,
  description,
  actions,
  eyebrow,
  children,
  className,
  bodyClassName,
  index,
}: {
  title?: ReactNode;
  description?: ReactNode;
  actions?: ReactNode;
  /** Small uppercase kicker above the title. */
  eyebrow?: ReactNode;
  children: ReactNode;
  className?: string;
  bodyClassName?: string;
  index?: number;
}) {
  return (
    <Reveal index={index} className={cn("h-full", className)}>
      <Card className="flex h-full flex-col overflow-hidden">
        {(title || actions) && (
          <div className="flex flex-wrap items-center justify-between gap-3 p-5 pb-4">
            <div className="min-w-0">
              {eyebrow ? (
                <div className="mb-1 text-[0.6875rem] font-medium uppercase tracking-[0.12em] text-muted-foreground/80">
                  {eyebrow}
                </div>
              ) : null}
              {title ? (
                <h3 className="text-[0.9375rem] font-semibold tracking-tight">{title}</h3>
              ) : null}
              {description ? (
                <p className="mt-0.5 text-xs text-muted-foreground">{description}</p>
              ) : null}
            </div>
            {actions ? <div className="flex shrink-0 items-center gap-2">{actions}</div> : null}
          </div>
        )}
        <div className={cn("flex-1 px-5 pb-5", !(title || actions) && "pt-5", bodyClassName)}>
          {children}
        </div>
      </Card>
    </Reveal>
  );
}
