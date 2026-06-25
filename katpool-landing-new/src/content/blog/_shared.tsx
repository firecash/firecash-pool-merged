/**
 * Shared presentational primitives for blog post bodies. Every post file
 * imports these so prose styling stays consistent across the blog. The post
 * `Body` returns a flat fragment of these elements as siblings — the
 * `/blog/[slug]` page wraps them in a `space-y-6` container.
 */
export const H2 = "text-xl font-semibold tracking-tight text-foreground";
export const H3 = "text-base font-semibold tracking-tight text-foreground";
export const P = "text-muted-foreground";
export const LINK = "text-primary underline-offset-2 hover:underline";
export const UL = "list-disc space-y-1.5 pl-5 text-muted-foreground";
export const OL = "list-decimal space-y-1.5 pl-5 text-muted-foreground";
export const TABLE_WRAP = "overflow-x-auto rounded-2xl border border-border";
export const TABLE = "w-full border-collapse text-sm";
export const TH = "border-b border-border bg-white/[0.02] px-4 py-2.5 text-left font-semibold text-foreground";
export const TD = "border-b border-border/60 px-4 py-2.5 align-top text-muted-foreground";
