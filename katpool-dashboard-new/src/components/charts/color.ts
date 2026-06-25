/**
 * Apply an alpha to a CSS color string for canvas use. Handles the modern
 * `oklch()/rgb()/hsl()` space-separated syntax (inject `/ <a>`) and `#rrggbb`
 * hex (append a hex alpha byte). Colors that already carry alpha are returned
 * unchanged.
 */
export function withAlpha(color: string, a: number): string {
  const alpha = Math.max(0, Math.min(1, a));
  const c = color.trim();

  if (/^(oklch|oklab|rgb|hsl|lab|lch|color)\(/i.test(c)) {
    if (c.includes("/")) return c; // already has an alpha channel
    return c.replace(/\)\s*$/, ` / ${alpha})`);
  }
  if (/^#([0-9a-f]{6})$/i.test(c)) {
    const byte = Math.round(alpha * 255)
      .toString(16)
      .padStart(2, "0");
    return `${c}${byte}`;
  }
  return c;
}
