'use client'

import { useEffect } from 'react'
import { safeWorkspaceHref } from '@/lib/navigation'

export default function DashboardRedirect() {
  useEffect(() => {
    fetch('/api/workspaces', { credentials: 'include', cache: 'no-store' })
      .then(res => (res.ok ? res.json() : []))
      .then((workspaces: { slug: string }[]) => {
        // safeWorkspaceHref validates the slug before navigating (CWE-601 fix)
        window.location.href = workspaces.length > 0
          ? safeWorkspaceHref(workspaces[0].slug)
          : '/create-workspace'
      })
      .catch(() => {
        window.location.href = '/create-workspace'
      })
  }, [])

  return null
}
