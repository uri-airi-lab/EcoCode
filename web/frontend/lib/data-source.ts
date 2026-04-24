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
  // For SSE or streams that shouldn't be buffered by Next.js proxy
  if (API_BASE_URL) {
    return new URL(path, API_BASE_URL).toString()
  }
  return path
}
