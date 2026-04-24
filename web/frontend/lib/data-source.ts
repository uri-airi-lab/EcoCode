export const API_BASE_URL = process.env.NEXT_PUBLIC_API_URL || ''

export function buildApiUrl(path: string): string {
  if (API_BASE_URL) {
    return new URL(path, API_BASE_URL).toString()
  }
  return path
}
