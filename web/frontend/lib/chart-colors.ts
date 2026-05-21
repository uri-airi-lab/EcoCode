/**
 * Utility function to resolve chart color CSS variables at runtime with optional alpha.
 * 
 * @param n - Chart color number (1-6 for named slots)
 * @param alpha - Optional opacity (0 to 1)
 * @returns HSL color string (e.g., "hsl(199 90% 64% / 0.5)")
 */
export function chartColor(n: number, alpha?: number): string {
  // Wrap around the 6 available palette slots
  const slot = ((n - 1) % 6) + 1

  const getHsl = (channels: string) => {
    return alpha !== undefined ? `hsl(${channels} / ${alpha})` : `hsl(${channels})`
  }

  // SSR fallback
  if (typeof window === 'undefined') {
    const defaults: Record<number, string> = {
      1: '142 80% 52%',  // green
      2: '27 96% 61%',   // orange
      3: '142 71% 45%',  // green
      4: '262 83% 58%',  // purple
      5: '0 84% 60%',    // red
      6: '47 95% 55%',   // yellow
    }
    return getHsl(defaults[slot])
  }

  // Client-side: read CSS variable
  const channels = getComputedStyle(document.documentElement)
    .getPropertyValue(`--chart-${slot}`)
    .trim()

  return channels ? getHsl(channels) : getHsl('0 0% 50%')
}
