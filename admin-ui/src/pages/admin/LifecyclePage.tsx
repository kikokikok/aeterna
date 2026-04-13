import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { apiClient } from '@/api/client'
import { Loader2, CheckCircle, XCircle, Clock, AlertTriangle, Trash2, RefreshCw, Shield } from 'lucide-react'
import { useState } from 'react'

interface RemediationRequest {
  id: string
  requestType: string
  riskTier: string
  entityType: string
  entityIds: string[]
  tenantId: string | null
  description: string
  proposedAction: string
  detectedBy: string
  status: string
  createdAt: number
  reviewedBy: string | null
  reviewedAt: number | null
  resolutionNotes: string | null
}

interface LifecycleStatus {
  enabled: boolean
  tasks: Record<string, { lastRun: string | null; nextRun: string | null; interval: string }>
  remediationSummary: { pending: number; approved: number; rejected: number; expired: number }
}

function statusBadge(status: string) {
  const colors: Record<string, string> = {
    pending: 'bg-yellow-100 text-yellow-800 dark:bg-yellow-900 dark:text-yellow-200',
    approved: 'bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-200',
    executed: 'bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-200',
    rejected: 'bg-red-100 text-red-800 dark:bg-red-900 dark:text-red-200',
    expired: 'bg-gray-100 text-gray-800 dark:bg-gray-700 dark:text-gray-300',
    failed: 'bg-red-100 text-red-800 dark:bg-red-900 dark:text-red-200',
  }
  return (
    <span className={`px-2 py-0.5 rounded text-xs font-medium ${colors[status] || 'bg-gray-100 text-gray-800'}`}>
      {status}
    </span>
  )
}

function riskBadge(tier: string) {
  const colors: Record<string, string> = {
    auto_execute: 'bg-green-100 text-green-700 dark:bg-green-900 dark:text-green-300',
    notify_and_execute: 'bg-yellow-100 text-yellow-700 dark:bg-yellow-900 dark:text-yellow-300',
    require_approval: 'bg-red-100 text-red-700 dark:bg-red-900 dark:text-red-300',
  }
  return (
    <span className={`px-2 py-0.5 rounded text-xs font-medium ${colors[tier] || 'bg-gray-100 text-gray-800'}`}>
      {tier.replace(/_/g, ' ')}
    </span>
  )
}

export default function LifecyclePage() {
  const queryClient = useQueryClient()
  const [showAll, setShowAll] = useState(false)
  const [rejectId, setRejectId] = useState<string | null>(null)
  const [rejectReason, setRejectReason] = useState('')

  const { data: status, isLoading: statusLoading } = useQuery<LifecycleStatus>({
    queryKey: ['lifecycle-status'],
    queryFn: () => apiClient.get('/api/v1/admin/lifecycle/status'),
    refetchInterval: 30000,
  })

  const { data: remediations, isLoading: remLoading } = useQuery<RemediationRequest[]>({
    queryKey: ['remediations', showAll],
    queryFn: () => apiClient.get(`/api/v1/admin/lifecycle/remediations${showAll ? '?all=true' : ''}`),
    refetchInterval: 15000,
  })

  const approveMutation = useMutation({
    mutationFn: (id: string) => apiClient.post(`/api/v1/admin/lifecycle/remediations/${id}/approve`, {}),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['remediations'] }),
  })

  const rejectMutation = useMutation({
    mutationFn: ({ id, reason }: { id: string; reason: string }) =>
      apiClient.post(`/api/v1/admin/lifecycle/remediations/${id}/reject`, { reason }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['remediations'] })
      setRejectId(null)
      setRejectReason('')
    },
  })

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold text-gray-900 dark:text-white">Lifecycle Operations</h1>
        <button
          onClick={() => queryClient.invalidateQueries({ queryKey: ['lifecycle-status', 'remediations'] })}
          className="flex items-center gap-2 px-3 py-1.5 text-sm bg-gray-100 dark:bg-gray-800 rounded hover:bg-gray-200 dark:hover:bg-gray-700"
        >
          <RefreshCw className="w-4 h-4" /> Refresh
        </button>
      </div>

      {/* Status cards */}
      <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
        <div className="bg-white dark:bg-gray-800 rounded-lg border border-gray-200 dark:border-gray-700 p-4">
          <div className="flex items-center gap-2 text-sm text-gray-500 dark:text-gray-400 mb-1">
            <Shield className="w-4 h-4" /> Lifecycle Manager
          </div>
          <div className="text-lg font-semibold text-gray-900 dark:text-white">
            {statusLoading ? '...' : status?.enabled ? 'Active' : 'Disabled'}
          </div>
        </div>

        <div className="bg-white dark:bg-gray-800 rounded-lg border border-gray-200 dark:border-gray-700 p-4">
          <div className="flex items-center gap-2 text-sm text-gray-500 dark:text-gray-400 mb-1">
            <Clock className="w-4 h-4" /> Pending Remediations
          </div>
          <div className="text-lg font-semibold text-yellow-600 dark:text-yellow-400">
            {statusLoading ? '...' : status?.remediationSummary?.pending || 0}
          </div>
        </div>

        <div className="bg-white dark:bg-gray-800 rounded-lg border border-gray-200 dark:border-gray-700 p-4">
          <div className="flex items-center gap-2 text-sm text-gray-500 dark:text-gray-400 mb-1">
            <CheckCircle className="w-4 h-4" /> Approved
          </div>
          <div className="text-lg font-semibold text-green-600 dark:text-green-400">
            {statusLoading ? '...' : status?.remediationSummary?.approved || 0}
          </div>
        </div>

        <div className="bg-white dark:bg-gray-800 rounded-lg border border-gray-200 dark:border-gray-700 p-4">
          <div className="flex items-center gap-2 text-sm text-gray-500 dark:text-gray-400 mb-1">
            <XCircle className="w-4 h-4" /> Rejected / Expired
          </div>
          <div className="text-lg font-semibold text-gray-600 dark:text-gray-400">
            {statusLoading ? '...' : (status?.remediationSummary?.rejected || 0) + (status?.remediationSummary?.expired || 0)}
          </div>
        </div>
      </div>

      {/* Remediation requests table */}
      <div className="bg-white dark:bg-gray-800 rounded-lg border border-gray-200 dark:border-gray-700">
        <div className="flex items-center justify-between px-4 py-3 border-b border-gray-200 dark:border-gray-700">
          <h2 className="text-lg font-semibold text-gray-900 dark:text-white">Remediation Requests</h2>
          <label className="flex items-center gap-2 text-sm text-gray-600 dark:text-gray-400">
            <input type="checkbox" checked={showAll} onChange={(e) => setShowAll(e.target.checked)} className="rounded" />
            Show all (including resolved)
          </label>
        </div>

        {remLoading ? (
          <div className="flex justify-center p-8"><Loader2 className="w-6 h-6 animate-spin text-gray-400" /></div>
        ) : !remediations?.length ? (
          <div className="p-8 text-center text-gray-500 dark:text-gray-400">
            No remediation requests {showAll ? '' : 'pending'}. The system is healthy.
          </div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead className="bg-gray-50 dark:bg-gray-900 text-left">
                <tr>
                  <th className="px-4 py-2 font-medium text-gray-600 dark:text-gray-400">Type</th>
                  <th className="px-4 py-2 font-medium text-gray-600 dark:text-gray-400">Risk</th>
                  <th className="px-4 py-2 font-medium text-gray-600 dark:text-gray-400">Entity</th>
                  <th className="px-4 py-2 font-medium text-gray-600 dark:text-gray-400">Description</th>
                  <th className="px-4 py-2 font-medium text-gray-600 dark:text-gray-400">Status</th>
                  <th className="px-4 py-2 font-medium text-gray-600 dark:text-gray-400">Created</th>
                  <th className="px-4 py-2 font-medium text-gray-600 dark:text-gray-400">Actions</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-gray-200 dark:divide-gray-700">
                {remediations.map((r) => (
                  <tr key={r.id} className="hover:bg-gray-50 dark:hover:bg-gray-900">
                    <td className="px-4 py-3 text-gray-900 dark:text-white">{r.requestType}</td>
                    <td className="px-4 py-3">{riskBadge(r.riskTier)}</td>
                    <td className="px-4 py-3 text-gray-600 dark:text-gray-400">
                      {r.entityType} ({r.entityIds.length} items)
                    </td>
                    <td className="px-4 py-3 text-gray-600 dark:text-gray-400 max-w-xs truncate">{r.description}</td>
                    <td className="px-4 py-3">{statusBadge(r.status)}</td>
                    <td className="px-4 py-3 text-gray-500 dark:text-gray-400 text-xs">
                      {new Date(r.createdAt * 1000).toLocaleString()}
                    </td>
                    <td className="px-4 py-3">
                      {r.status === 'pending' && (
                        <div className="flex gap-2">
                          <button
                            onClick={() => approveMutation.mutate(r.id)}
                            disabled={approveMutation.isPending}
                            className="px-2 py-1 text-xs bg-green-600 text-white rounded hover:bg-green-700 disabled:opacity-50"
                          >
                            Approve
                          </button>
                          <button
                            onClick={() => setRejectId(r.id)}
                            className="px-2 py-1 text-xs bg-red-600 text-white rounded hover:bg-red-700"
                          >
                            Reject
                          </button>
                        </div>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {/* Lifecycle tasks status */}
      {status?.tasks && (
        <div className="bg-white dark:bg-gray-800 rounded-lg border border-gray-200 dark:border-gray-700">
          <div className="px-4 py-3 border-b border-gray-200 dark:border-gray-700">
            <h2 className="text-lg font-semibold text-gray-900 dark:text-white">Scheduled Tasks</h2>
          </div>
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead className="bg-gray-50 dark:bg-gray-900 text-left">
                <tr>
                  <th className="px-4 py-2 font-medium text-gray-600 dark:text-gray-400">Task</th>
                  <th className="px-4 py-2 font-medium text-gray-600 dark:text-gray-400">Interval</th>
                  <th className="px-4 py-2 font-medium text-gray-600 dark:text-gray-400">Last Run</th>
                  <th className="px-4 py-2 font-medium text-gray-600 dark:text-gray-400">Next Run</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-gray-200 dark:divide-gray-700">
                {Object.entries(status.tasks).map(([name, task]) => (
                  <tr key={name} className="hover:bg-gray-50 dark:hover:bg-gray-900">
                    <td className="px-4 py-3 text-gray-900 dark:text-white font-medium">{name.replace(/_/g, ' ')}</td>
                    <td className="px-4 py-3 text-gray-600 dark:text-gray-400">{task.interval}</td>
                    <td className="px-4 py-3 text-gray-500 dark:text-gray-400 text-xs">{task.lastRun || 'Never'}</td>
                    <td className="px-4 py-3 text-gray-500 dark:text-gray-400 text-xs">{task.nextRun || 'N/A'}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}

      {/* Reject dialog */}
      {rejectId && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div className="bg-white dark:bg-gray-800 rounded-lg p-6 w-full max-w-md">
            <h3 className="text-lg font-semibold text-gray-900 dark:text-white mb-4">Reject Remediation</h3>
            <textarea
              value={rejectReason}
              onChange={(e) => setRejectReason(e.target.value)}
              placeholder="Reason for rejection..."
              className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded bg-white dark:bg-gray-900 text-gray-900 dark:text-white mb-4"
              rows={3}
            />
            <div className="flex justify-end gap-2">
              <button onClick={() => { setRejectId(null); setRejectReason('') }} className="px-3 py-1.5 text-sm bg-gray-100 dark:bg-gray-700 rounded">
                Cancel
              </button>
              <button
                onClick={() => rejectMutation.mutate({ id: rejectId, reason: rejectReason })}
                disabled={!rejectReason.trim() || rejectMutation.isPending}
                className="px-3 py-1.5 text-sm bg-red-600 text-white rounded hover:bg-red-700 disabled:opacity-50"
              >
                Reject
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
