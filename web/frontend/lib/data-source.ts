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

export function buildAbsoluteApiUrl(path: string): string {
  // For SSE connections, always use relative URLs in the browser to go through Next.js proxy
  // This ensures proper CORS handling and network accessibility
  if (typeof window !== 'undefined') {
    return path
  }
  
  // Server-side: use absolute URL if available
  if (API_BASE_URL) {
    return new URL(path, API_BASE_URL).toString()
  }
  return path
}
