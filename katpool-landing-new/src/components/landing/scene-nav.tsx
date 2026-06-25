"use client";

import { motion } from "framer-motion";
import { cn } from "@/lib/utils";

interface SceneNavProps {
  scenes: readonly { id: string; label: string }[];
  active: number;
  onSelect: (index: number) => void;
}

export function SceneNav({ scenes, active, onSelect }: SceneNavProps) {
  return (
    <nav
      className="absolute inset-x-0 bottom-0 z-40 flex flex-col items-center gap-3 px-5 pb-6 sm:pb-8"
      aria-label="Scene navigation"
    >
      <p className="hidden text-[10px] uppercase tracking-[0.2em] text-muted-foreground sm:block">
        Scroll or use arrow keys · {scenes[active]?.label}
      </p>
      <div className="flex items-center gap-2 rounded-full border border-border bg-card/60 px-3 py-2 backdrop-blur-md">
        {scenes.map((scene, i) => (
          <button
            key={scene.id}
            type="button"
            aria-label={`Go to ${scene.label}`}
            aria-current={i === active ? "step" : undefined}
            onClick={() => onSelect(i)}
            className="group relative flex size-8 items-center justify-center"
          >
            <span
              className={cn(
                "block rounded-full transition-all duration-300",
                i === active
                  ? "size-2.5 bg-primary shadow-[0_0_12px_oklch(0.82_0.15_184/60%)]"
                  : "size-1.5 bg-muted-foreground/40 group-hover:bg-muted-foreground/70",
              )}
            />
            {i === active && (
              <motion.span
                layoutId="scene-ring"
                className="absolute inset-0 rounded-full border border-primary/40"
                transition={{ type: "spring", stiffness: 380, damping: 30 }}
              />
            )}
          </button>
        ))}
      </div>
    </nav>
  );
}
