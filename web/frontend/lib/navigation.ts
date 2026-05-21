/**
 * Validates that a workspace slug is safe before using it in a redirect.
 *
 * Prevents open redirect attacks (CWE-601) where a compromised or crafted
 * `/api/workspaces` response could return a slug like `//evil.com` or
 * `javascript:alert(1)` and cause the browser to navigate off-site.
 *
 * Only slugs matching the expected alphanumeric-hyphen format are allowed.
 * Anything that doesn't match falls back to `/create-workspace`.
 */
export function safeWorkspaceHref(slug: string): string {
  // Slugs must be non-empty, start and end with alphanumeric, and contain
  // only lowercase letters, digits, and hyphens — no protocol separators.
  const SLUG_RE = /^[a-z0-9][a-z0-9-]*[a-z0-9]$|^[a-z0-9]$/i
  return SLUG_RE.test(slug) ? `/${slug}` : '/create-workspace'
}
