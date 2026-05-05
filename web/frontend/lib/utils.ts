import { clsx, type ClassValue } from 'clsx'
import { twMerge } from 'tailwind-merge'
import { format } from 'date-fns'

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

/**
 * Returns the minimum interval between consecutive timestamps (in ms).
 * Used to decide whether sub-second formatting is needed.
 */
export function minIntervalMs(timestamps: number[]): number {
  if (timestamps.length < 2) return Infinity
  let min = Infinity
  for (let i = 1; i < timestamps.length; i++) {
    const delta = timestamps[i] - timestamps[i - 1]
    if (delta > 0 && delta < min) min = delta
  }
  return min
}

/**
 * Formats a timestamp for chart display.
 * Uses tenths-of-second precision when the sampling interval is sub-second.
 */
export function formatChartTime(ts: number, intervalMs: number): string {
  return format(new Date(ts), 'HH:mm:ss.SSS')
}
