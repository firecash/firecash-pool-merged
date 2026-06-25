import Image from "next/image";
import Link from "next/link";
import { cn } from "@/lib/utils";

const POOL_NAME = process.env.NEXT_PUBLIC_POOL_NAME ?? "katpool";

/**
 * Horizontal wordmark (glyph + name) used in the sidebar and mobile header.
 *
 * The PNG carries its own deep-teal background (`#060e11`); surfaces that host
 * it match that exact fill so the mark blends in with no visible plate or
 * framing.
 */
export function Brand({ className }: { className?: string }) {
  return (
    <Link href="/" className={cn("flex items-center", className)} aria-label={`${POOL_NAME} home`}>
      <Image
        src="/katpool-wordmark.png"
        alt={POOL_NAME}
        width={2500}
        height={800}
        priority
        className="h-9 w-auto select-none"
      />
    </Link>
  );
}
