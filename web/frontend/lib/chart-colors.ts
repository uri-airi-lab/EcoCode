/**
 * Utility function to resolve chart color CSS variables at runtime.
 * 
 * Reads --chart-N CSS custom properties from the document root and wraps
 * the HSL channel values in hsl() for use in Recharts components.
 * 
 * @param n - Chart color number (1-6)
 * @returns HSL color string (e.g., "hsl(199 90% 64%)")
 */
export function chartColor(n: 1 | 2 | 3 | 4 | 5 | 6): string {
  // SSR fallback — return W&B defaults directly
  if (typeof window === 'undefined') {
    const defaults: Record<number, string> = {
      1: 'hsl(217 91% 60%)',  // blue
      2: 'hsl(27 96% 61%)',   // orange
      3: 'hsl(142 71% 45%)',  // green
      4: 'hsl(262 83% 58%)',  // purple
      5: 'hsl(0 84% 60%)',    // red
      6: 'hsl(47 95% 55%)',   // yellow
    }
    return defaults[n]
  }

  // Client-side: read CSS variable and wrap in hsl()
  const channels = getComputedStyle(document.documentElement)
    .getPropertyValue(`--chart-${n}`)
    .trim()

  return channels ? `hsl(${channels})` : `hsl(0 0% 50%)` // fallback to gray if missing
}
