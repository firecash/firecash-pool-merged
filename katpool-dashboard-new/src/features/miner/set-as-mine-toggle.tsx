"use client";

import { Star } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useMyMiner } from "@/lib/use-my-miner";
import { cn } from "@/lib/utils";

/**
 * Marks (or unmarks) this address as "mine", persisted in localStorage and
 * surfaced by the sidebar MinerPulse. No auth — purely a local convenience.
 */
export function SetAsMineToggle({ address }: { address: string }) {
  const { isMine, setMine, clear, hydrated } = useMyMiner();
  if (!hydrated) return null;

  const mine = isMine(address);
  return (
    <Button
      type="button"
      variant={mine ? "secondary" : "outline"}
      size="sm"
      onClick={() => (mine ? clear() : setMine(address))}
      aria-pressed={mine}
      title={mine ? "Stop tracking this miner in the sidebar" : "Track this miner in the sidebar"}
    >
      <Star className={cn("size-3.5", mine && "fill-current text-secondary")} />
      {mine ? "Your miner" : "Set as mine"}
    </Button>
  );
}
