export const API_BASE_URL = process.env.NEXT_PUBLIC_API_URL || ''

export function buildApiUrl(path: string): string {
  // Always use relative URLs in the browser to route through Next.js proxy
  // This avoids CORS issues and ensures cookies are sent correctly
  if (typeof window !== 'undefined') {
    return path
  }

  if (API_BASE_URL) {
    return new URL(path, API_BASE_URL).toString()
  }
  return path
}

/**
 * Returns a direct backend URL for SSE connections, bypassing the Next.js proxy.
 * Next.js rewrites/route-handlers buffer responses which breaks SSE streaming.
 * The backend has CORS configured so the browser can connect directly.
 *
 * Resolution order:
 * 1. NEXT_PUBLIC_API_URL env var (set in Docker / deployments)
 * 2. Same-host on port 3001 (local dev: browser is on localhost:3000)
 */
export function buildDirectSseUrl(path: string): string {
  if (API_BASE_URL) {
    return new URL(path, API_BASE_URL).toString()
  }
  // Local dev: backend is on the same host, port 3001
  if (typeof window !== 'undefined') {
    return `${window.location.protocol}//${window.location.hostname}:3001${path}`
  }
  return `http://localhost:3001${path}`
}
