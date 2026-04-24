'use client'

import { useEffect, useState, useCallback } from 'react'
import { useParams } from 'next/navigation'
import { Users, Mail, Shield, UserPlus, Loader2, Trash2, Calendar } from 'lucide-react'
import { DashboardLayout } from '@/components/DashboardLayout'
import { useWorkspace } from '@/components/WorkspaceProvider'

interface Member {
  user_id: number
  email: string
  role: string
  joined_at: string
}

export default function MembersSettingsPage() {
  const params = useParams()
  const workspaceSlug = params.workspaceSlug as string
  const { currentWorkspace } = useWorkspace()

  const [members, setMembers] = useState<Member[]>([])
  const [loadingMembers, setLoadingMembers] = useState(true)
  const [email, setEmail] = useState('')
  const [role, setRole] = useState('member')
  const [loading, setLoading] = useState(false)
  const [message, setMessage] = useState({ text: '', type: '' })

  const isAdmin = currentWorkspace?.role === 'admin'

  const fetchMembers = useCallback(async () => {
    try {
      const res = await fetch(`/api/workspaces/${workspaceSlug}/members`)
      if (res.ok) {
        const data = await res.json()
        setMembers(data)
      }
    } catch (err) {
      console.error('Failed to fetch members:', err)
    } finally {
      setLoadingMembers(false)
    }
  }, [workspaceSlug])

  useEffect(() => {
    fetchMembers()
  }, [fetchMembers])

  const handleInvite = async (e: React.FormEvent) => {
    e.preventDefault()
    setLoading(true)
    setMessage({ text: '', type: '' })

    try {
      const res = await fetch(`/api/workspaces/${workspaceSlug}/invite`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ email, role }),
      })

      if (res.ok) {
        setMessage({ text: 'User invited successfully', type: 'success' })
        setEmail('')
        fetchMembers()
      } else {
        const err = await res.text()
        setMessage({ text: err || 'Failed to invite user', type: 'error' })
      }
    } catch (e) {
      setMessage({ text: 'An error occurred', type: 'error' })
    } finally {
      setLoading(false)
    }
  }

  const handleRemove = async (userId: number) => {
    if (!confirm('Are you sure you want to remove this member?')) return

    try {
      const res = await fetch(`/api/workspaces/${workspaceSlug}/members/${userId}`, {
        method: 'DELETE',
      })

      if (res.ok) {
        fetchMembers()
      } else {
        const err = await res.text()
        alert(err || 'Failed to remove member')
      }
    } catch (err) {
      alert('An error occurred')
    }
  }

  return (
    <DashboardLayout>
      <div className="p-4 md:p-8 max-w-5xl mx-auto w-full">
        <div className="mb-8">
          <h1 className="text-2xl md:text-3xl font-bold text-foreground mb-2 flex items-center">
            <Users className="w-8 h-8 mr-3 text-emerald-500" />
            Workspace Members
          </h1>
          <p className="text-muted-foreground">
            Manage who has access to this workspace.
          </p>
        </div>

        {/* Members List */}
        <div className="bg-neutral-900/40 border border-neutral-800 rounded-2xl overflow-hidden mb-8">
          <div className="p-6 border-b border-neutral-800">
            <h2 className="text-lg font-semibold text-foreground flex items-center">
              <Users className="w-5 h-5 mr-2 text-neutral-400" />
              Active Members
            </h2>
          </div>
          
          <div className="overflow-x-auto">
            <table className="w-full text-left border-collapse">
              <thead>
                <tr className="bg-neutral-950/30">
                  <th className="px-6 py-4 text-xs font-semibold uppercase tracking-wider text-neutral-500 border-b border-neutral-800">Email</th>
                  <th className="px-6 py-4 text-xs font-semibold uppercase tracking-wider text-neutral-500 border-b border-neutral-800">Role</th>
                  <th className="px-6 py-4 text-xs font-semibold uppercase tracking-wider text-neutral-500 border-b border-neutral-800">Joined</th>
                  <th className="px-6 py-4 text-xs font-semibold uppercase tracking-wider text-neutral-500 border-b border-neutral-800 text-right">Actions</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-neutral-800">
                {loadingMembers ? (
                  <tr>
                    <td colSpan={4} className="px-6 py-12 text-center text-neutral-500">
                      <Loader2 className="w-6 h-6 animate-spin mx-auto mb-2" />
                      Loading members...
                    </td>
                  </tr>
                ) : members.length === 0 ? (
                  <tr>
                    <td colSpan={4} className="px-6 py-12 text-center text-neutral-500">
                      No members found.
                    </td>
                  </tr>
                ) : (
                  members.map((member) => (
                    <tr key={member.user_id} className="hover:bg-neutral-800/20 transition-colors">
                      <td className="px-6 py-4">
                        <div className="flex items-center">
                          <div className="w-8 h-8 rounded-full bg-emerald-500/10 flex items-center justify-center mr-3 text-emerald-500">
                            <Mail className="w-4 h-4" />
                          </div>
                          <span className="text-sm font-medium text-white">{member.email}</span>
                        </div>
                      </td>
                      <td className="px-6 py-4">
                        <span className={`inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium ${
                          member.role === 'admin' 
                            ? 'bg-purple-500/10 text-purple-400 border border-purple-500/20' 
                            : 'bg-blue-500/10 text-blue-400 border border-blue-500/20'
                        }`}>
                          <Shield className="w-3 h-3 mr-1" />
                          {member.role.charAt(0).toUpperCase() + member.role.slice(1)}
                        </span>
                      </td>
                      <td className="px-6 py-4">
                        <div className="flex items-center text-xs text-neutral-400">
                          <Calendar className="w-3.5 h-3.5 mr-1.5" />
                          {new Date(member.joined_at).toLocaleDateString()}
                        </div>
                      </td>
                      <td className="px-6 py-4 text-right">
                        {isAdmin && (
                          <button 
                            onClick={() => handleRemove(member.user_id)}
                            className="p-2 text-neutral-500 hover:text-red-400 hover:bg-red-500/10 rounded-lg transition-colors"
                            title="Remove member"
                          >
                            <Trash2 className="w-4 h-4" />
                          </button>
                        )}
                      </td>
                    </tr>
                  ))
                )}
              </tbody>
            </table>
          </div>
        </div>

        {isAdmin && (
          <div className="bg-neutral-900/40 border border-neutral-800 rounded-2xl p-6 sm:p-8">
            <h2 className="text-lg font-semibold text-foreground mb-4 flex items-center">
              <UserPlus className="w-5 h-5 mr-2 text-neutral-400" />
              Invite a Member
            </h2>
            
            <form onSubmit={handleInvite} className="space-y-4 max-w-lg">
              {message.text && (
                <div className={`px-4 py-3 rounded-xl text-sm border ${message.type === 'success' ? 'bg-emerald-500/10 border-emerald-500/20 text-emerald-400' : 'bg-red-500/10 border-red-500/20 text-red-400'}`}>
                  {message.text}
                </div>
              )}

              <div>
                <label className="block text-sm font-medium text-neutral-300 mb-2">
                  User Email Address
                </label>
                <div className="relative">
                  <div className="absolute inset-y-0 left-0 pl-3 flex items-center pointer-events-none">
                    <Mail className="h-5 w-5 text-neutral-500" />
                  </div>
                  <input
                    type="email"
                    required
                    value={email}
                    onChange={(e) => setEmail(e.target.value)}
                    className="w-full pl-10 pr-4 py-2.5 bg-neutral-950/50 border border-neutral-800 rounded-xl text-white placeholder:text-neutral-600 focus:outline-none focus:ring-2 focus:ring-emerald-500/50"
                    placeholder="colleague@example.com"
                  />
                </div>
              </div>

              <div>
                <label className="block text-sm font-medium text-neutral-300 mb-2">
                  Role
                </label>
                <div className="relative">
                  <div className="absolute inset-y-0 left-0 pl-3 flex items-center pointer-events-none">
                    <Shield className="h-5 w-5 text-neutral-500" />
                  </div>
                  <select
                    value={role}
                    onChange={(e) => setRole(e.target.value)}
                    className="w-full pl-10 pr-4 py-2.5 bg-neutral-950/50 border border-neutral-800 rounded-xl text-white appearance-none focus:outline-none focus:ring-2 focus:ring-emerald-500/50"
                  >
                    <option value="member">Member (Can view and create runs)</option>
                    <option value="admin">Admin (Can manage settings and invite)</option>
                  </select>
                </div>
              </div>

              <button
                type="submit"
                disabled={loading}
                className="bg-emerald-500 hover:bg-emerald-600 text-neutral-950 font-medium py-2.5 px-6 rounded-xl transition-colors disabled:opacity-50 flex items-center mt-2"
              >
                {loading ? <Loader2 className="w-5 h-5 animate-spin mr-2" /> : null}
                Send Invitation
              </button>
            </form>
          </div>
        )}
      </div>
    </DashboardLayout>
  )
}

