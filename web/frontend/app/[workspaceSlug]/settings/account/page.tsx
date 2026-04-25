'use client'

import { useState } from 'react'
import { Lock, Loader2, User, LogOut, Trash2 } from 'lucide-react'
import { useRouter } from 'next/navigation'
import { DashboardLayout } from '@/components/layout/DashboardLayout'
import { toast } from 'sonner'
import { useWorkspace } from '@/components/layout/WorkspaceProvider'

export default function AccountSettingsPage() {
  const router = useRouter()
  const { currentWorkspace, workspaces, setCurrentWorkspace } = useWorkspace()
  const [currentPassword, setCurrentPassword] = useState('')
  const [newPassword, setNewPassword] = useState('')
  const [confirmPassword, setConfirmPassword] = useState('')
  const [loading, setLoading] = useState(false)
  const [message, setMessage] = useState<{ text: string; type: 'success' | 'error' } | null>(null)
  const [loggingOut, setLoggingOut] = useState(false)
  const [deletingWorkspace, setDeletingWorkspace] = useState(false)
  const [deleteConfirmation, setDeleteConfirmation] = useState('')

  const handleChangePassword = async (e: React.FormEvent) => {
    e.preventDefault()
    setMessage(null)

    if (newPassword !== confirmPassword) {
      setMessage({ text: 'New passwords do not match', type: 'error' })
      return
    }
    if (newPassword.length < 8) {
      setMessage({ text: 'New password must be at least 8 characters', type: 'error' })
      return
    }

    setLoading(true)
    try {
      const res = await fetch('/api/auth/change-password', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          current_password: currentPassword,
          new_password: newPassword,
        }),
        credentials: 'include',
      })

      if (res.ok) {
        setMessage({ text: 'Password changed successfully', type: 'success' })
        setCurrentPassword('')
        setNewPassword('')
        setConfirmPassword('')
      } else {
        const err = await res.text()
        setMessage({ text: err || 'Failed to change password', type: 'error' })
      }
    } catch {
      setMessage({ text: 'An error occurred. Please try again.', type: 'error' })
    } finally {
      setLoading(false)
    }
  }

  const handleLogout = async () => {
    setLoggingOut(true)
    try {
      const res = await fetch('/api/auth/logout', { 
        method: 'POST',
        credentials: 'include'
      })
      if (res.ok) {
        toast.success('Logged out successfully')
        router.push('/login')
      } else {
        toast.error('Failed to logout')
      }
    } catch (error) {
      toast.error('An error occurred during logout')
    } finally {
      setLoggingOut(false)
    }
  }

  const handleDeleteWorkspace = async () => {
    if (!currentWorkspace) return
    
    if (deleteConfirmation !== currentWorkspace.name) {
      toast.error('Workspace name does not match')
      return
    }

    setDeletingWorkspace(true)
    try {
      const res = await fetch(`/api/workspaces/${currentWorkspace.slug}`, {
        method: 'DELETE',
        credentials: 'include'
      })

      if (res.ok) {
        toast.success('Workspace deleted successfully')
        // Switch to another workspace or redirect
        const remainingWorkspaces = workspaces.filter(w => w.id !== currentWorkspace.id)
        if (remainingWorkspaces.length > 0) {
          setCurrentWorkspace(remainingWorkspaces[0])
          router.push(`/${remainingWorkspaces[0].slug}`)
        } else {
          router.push('/')
        }
      } else {
        const error = await res.text()
        toast.error(`Failed to delete workspace: ${error}`)
      }
    } catch (error) {
      console.error('Failed to delete workspace:', error)
      toast.error('Failed to delete workspace. Please try again.')
    } finally {
      setDeletingWorkspace(false)
      setDeleteConfirmation('')
    }
  }

  const isWorkspaceAdmin = currentWorkspace?.role === 'admin'

  return (
    <DashboardLayout>
      <div className="p-4 md:p-8 max-w-4xl mx-auto w-full">
        <div className="mb-8">
          <h1 className="text-2xl md:text-3xl font-bold text-foreground mb-2 flex items-center">
            <User className="w-8 h-8 mr-3 text-primary" />
            Account Settings
          </h1>
          <p className="text-muted-foreground">
            Manage your account security and preferences.
          </p>
        </div>

        {/* Change Password Section */}
        <div className="bg-card border border-border rounded-md overflow-hidden mb-6">
          <div className="p-6 border-b border-border">
            <h2 className="text-lg font-semibold mb-1 flex items-center gap-2">
              <Lock className="w-5 h-5 text-primary" />
              Change Password
            </h2>
            <p className="text-sm text-muted-foreground">
              Update your account password. You&apos;ll need your current password to confirm.
            </p>
          </div>
          <div className="p-6">
            <form onSubmit={handleChangePassword} className="space-y-4 max-w-md">
              <div>
                <label className="block text-sm font-medium text-foreground mb-1.5">
                  Current Password
                </label>
                <input
                  type="password"
                  value={currentPassword}
                  onChange={(e) => setCurrentPassword(e.target.value)}
                  required
                  autoComplete="current-password"
                  className="w-full bg-background border border-border rounded-md px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground/50 focus:outline-none focus:ring-2 focus:ring-primary/50 focus:border-primary transition-all"
                  placeholder="Enter current password"
                />
              </div>
              <div>
                <label className="block text-sm font-medium text-foreground mb-1.5">
                  New Password
                </label>
                <input
                  type="password"
                  value={newPassword}
                  onChange={(e) => setNewPassword(e.target.value)}
                  required
                  autoComplete="new-password"
                  minLength={8}
                  className="w-full bg-background border border-border rounded-md px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground/50 focus:outline-none focus:ring-2 focus:ring-primary/50 focus:border-primary transition-all"
                  placeholder="At least 8 characters"
                />
              </div>
              <div>
                <label className="block text-sm font-medium text-foreground mb-1.5">
                  Confirm New Password
                </label>
                <input
                  type="password"
                  value={confirmPassword}
                  onChange={(e) => setConfirmPassword(e.target.value)}
                  required
                  autoComplete="new-password"
                  className="w-full bg-background border border-border rounded-md px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground/50 focus:outline-none focus:ring-2 focus:ring-primary/50 focus:border-primary transition-all"
                  placeholder="Repeat new password"
                />
              </div>

              {message && (
                <div className={`p-3 rounded-md text-sm ${
                  message.type === 'success' 
                    ? 'bg-green-500/10 text-green-500 border border-green-500/20' 
                    : 'bg-destructive/10 text-destructive border border-destructive/20'
                }`}>
                  {message.text}
                </div>
              )}

              <button
                type="submit"
                disabled={loading || !currentPassword || !newPassword || !confirmPassword}
                className="bg-primary hover:brightness-110 text-primary-foreground px-4 py-2 rounded-md font-medium text-sm flex items-center transition-all disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {loading && <Loader2 className="w-4 h-4 mr-2 animate-spin" />}
                Update Password
              </button>
            </form>
          </div>
        </div>

        {/* Delete Workspace Section */}
        {isWorkspaceAdmin && currentWorkspace && (
          <div className="bg-card border border-destructive/20 rounded-md overflow-hidden mb-6">
            <div className="p-6 border-b border-destructive/20">
              <h2 className="text-lg font-semibold mb-1 flex items-center gap-2 text-destructive">
                <Trash2 className="w-5 h-5" />
                Delete Workspace
              </h2>
              <p className="text-sm text-muted-foreground">
                Permanently delete the workspace &quot;{currentWorkspace.name}&quot; and all its data. This action cannot be undone.
              </p>
            </div>
            <div className="p-6">
              <div className="space-y-4 max-w-md">
                <div className="bg-destructive/10 border border-destructive/20 rounded-md p-4">
                  <p className="text-sm text-destructive font-medium mb-2">⚠️ Warning</p>
                  <p className="text-xs text-muted-foreground">
                    This will permanently delete:
                  </p>
                  <ul className="text-xs text-muted-foreground list-disc list-inside mt-2 space-y-1">
                    <li>All projects in this workspace</li>
                    <li>All runs and their records</li>
                    <li>All workspace members and settings</li>
                  </ul>
                </div>

                <div>
                  <label className="block text-sm font-medium text-foreground mb-1.5">
                    Type <span className="font-mono bg-destructive/10 px-1.5 py-0.5 rounded text-destructive">{currentWorkspace.name}</span> to confirm
                  </label>
                  <input
                    type="text"
                    value={deleteConfirmation}
                    onChange={(e) => setDeleteConfirmation(e.target.value)}
                    className="w-full bg-background border border-border rounded-md px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground/50 focus:outline-none focus:ring-2 focus:ring-destructive/50 focus:border-destructive transition-all"
                    placeholder={`Type "${currentWorkspace.name}" to confirm`}
                  />
                </div>

                <button
                  onClick={handleDeleteWorkspace}
                  disabled={deletingWorkspace || deleteConfirmation !== currentWorkspace.name}
                  className="bg-destructive hover:brightness-110 text-destructive-foreground px-4 py-2 rounded-md font-medium text-sm flex items-center transition-all disabled:opacity-50 disabled:cursor-not-allowed"
                >
                  {deletingWorkspace ? (
                    <>
                      <Loader2 className="w-4 h-4 mr-2 animate-spin" />
                      Deleting...
                    </>
                  ) : (
                    <>
                      <Trash2 className="w-4 h-4 mr-2" />
                      Delete Workspace
                    </>
                  )}
                </button>
              </div>
            </div>
          </div>
        )}

        {/* Logout Section */}
        <div className="bg-card border border-destructive/20 rounded-md overflow-hidden">
          <div className="p-6 border-b border-destructive/20">
            <h2 className="text-lg font-semibold mb-1 flex items-center gap-2 text-destructive">
              <LogOut className="w-5 h-5" />
              Logout
            </h2>
            <p className="text-sm text-muted-foreground">
              Sign out of your account on this device.
            </p>
          </div>
          <div className="p-6">
            <button
              onClick={handleLogout}
              disabled={loggingOut}
              className="bg-destructive hover:brightness-110 text-destructive-foreground px-4 py-2 rounded-md font-medium text-sm flex items-center transition-all disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {loggingOut ? (
                <>
                  <Loader2 className="w-4 h-4 mr-2 animate-spin" />
                  Logging out...
                </>
              ) : (
                <>
                  <LogOut className="w-4 h-4 mr-2" />
                  Logout
                </>
              )}
            </button>
          </div>
        </div>
      </div>
    </DashboardLayout>
  )
}
