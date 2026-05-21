'use client'

import { useEffect } from 'react'

export default function DashboardRedirect() {
  useEffect(() => {
    fetch('/api/workspaces', { credentials: 'include', cache: 'no-store' })
      .then(res => (res.ok ? res.json() : []))
      .then((workspaces: { slug: string }[]) => {
        window.location.href = workspaces.length > 0 ? `/${workspaces[0].slug}` : '/create-workspace'
      })
      .catch(() => {
        window.location.href = '/create-workspace'
      })
  }, [])

  return null
}
