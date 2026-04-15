import { useState } from "react"
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query"
import { Shield, Loader2, Check, XIcon, MessageSquare, Filter } from "lucide-react"
import { cn } from "@/lib/utils"
import { apiClient } from "@/api/client"
import type { GovernanceRequest } from "@/api/types"

function ActionDialog({
  open,
  onClose,
  requestId,
  action,
}: {
  open: boolean
  onClose: () => void
  requestId: string
  action: "approve" | "reject"
}) {
  const queryClient = useQueryClient()
  const [comment, setComment] = useState("")

  const doAction = useMutation({
    mutationFn: () =>
      apiClient.post(`/api/v1/govern/${action}/${requestId}`, {
        comment: comment || undefined,
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["governance"] })
      setComment("")
      onClose()
    },
  })

  if (!open) return null

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="w-full max-w-md rounded-lg bg-white p-6 shadow-xl dark:bg-gray-800">
        <h2 className="mb-4 text-lg font-semibold capitalize text-gray-900 dark:text-gray-100">
          {action} Request
        </h2>
        <div>
          <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">
            Comment (optional)
          </label>
          <textarea
            value={comment}
            onChange={(e) => setComment(e.target.value)}
            rows={3}
            className="mt-1 block w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-700 dark:text-gray-100"
            placeholder="Add a comment..."
          />
        </div>
        {doAction.isError && (
          <p className="mt-2 text-sm text-red-600">Action failed. Please try again.</p>
        )}
        <div className="mt-4 flex justify-end gap-3">
          <button
            onClick={onClose}
            className="rounded-md border border-gray-300 px-4 py-2 text-sm font-medium text-gray-700 hover:bg-gray-50 dark:border-gray-600 dark:text-gray-300"
          >
            Cancel
          </button>
          <button
            onClick={() => doAction.mutate()}
            disabled={doAction.isPending}
            className={cn(
              "inline-flex items-center gap-2 rounded-md px-4 py-2 text-sm font-medium text-white disabled:opacity-50",
              action === "approve"
                ? "bg-green-600 hover:bg-green-700"
                : "bg-red-600 hover:bg-red-700",
            )}
          >
            {doAction.isPending && <Loader2 className="h-4 w-4 animate-spin" />}
            {action === "approve" ? "Approve" : "Reject"}
          </button>
        </div>
      </div>
    </div>
  )
}

export function Component() {
  return <PendingRequestsPage />
}

export default function PendingRequestsPage() {
  const [typeFilter, setTypeFilter] = useState("")
  const [dialogState, setDialogState] = useState<{
    open: boolean
    requestId: string
    action: "approve" | "reject"
  }>({ open: false, requestId: "", action: "approve" })

  const { data, isLoading, error, refetch } = useQuery<GovernanceRequest[]>({
    queryKey: ["governance", "pending", typeFilter],
    queryFn: () => {
      const params = new URLSearchParams()
      if (typeFilter) params.set("type", typeFilter)
      const qs = params.toString()
      return apiClient.get(`/api/v1/govern/pending${qs ? `?${qs}` : ""}`)
    },
  })

  const requests = data ?? []

  const statusColor: Record<string, string> = {
    pending: "bg-yellow-100 text-yellow-700 dark:bg-yellow-900 dark:text-yellow-300",
    approved: "bg-green-100 text-green-700 dark:bg-green-900 dark:text-green-300",
    rejected: "bg-red-100 text-red-700 dark:bg-red-900 dark:text-red-300",
  }

  return (
    <div>
      <div className="mb-6 flex items-center gap-3">
        <Shield className="h-6 w-6 text-gray-400" />
        <h1 className="text-2xl font-semibold text-gray-900 dark:text-gray-100">Governance</h1>
      </div>

      <div className="mb-4 flex items-center gap-3">
        <Filter className="h-4 w-4 text-gray-400" />
        <select
          value={typeFilter}
          onChange={(e) => setTypeFilter(e.target.value)}
          className="rounded-md border border-gray-300 px-3 py-1.5 text-sm dark:border-gray-600 dark:bg-gray-700 dark:text-gray-100"
        >
          <option value="">All types</option>
          <option value="policy">Policy</option>
          <option value="knowledge">Knowledge</option>
          <option value="memory">Memory</option>
          <option value="role">Role</option>
          <option value="config">Config</option>
        </select>
      </div>

      {isLoading && (
        <div className="flex justify-center p-8">
          <Loader2 className="h-6 w-6 animate-spin text-gray-400" />
        </div>
      )}

      {error && (
        <div className="p-8 text-center text-red-600">
          Failed to load requests.{" "}
          <button onClick={() => refetch()} className="underline">
            Retry
          </button>
        </div>
      )}

      {!isLoading && !error && (
        <div className="overflow-hidden rounded-lg border border-gray-200 dark:border-gray-700">
          <table className="min-w-full divide-y divide-gray-200 dark:divide-gray-700">
            <thead className="bg-gray-50 dark:bg-gray-800">
              <tr>
                <th className="px-4 py-3 text-left text-xs font-medium uppercase tracking-wider text-gray-500">
                  Type
                </th>
                <th className="px-4 py-3 text-left text-xs font-medium uppercase tracking-wider text-gray-500">
                  Requestor
                </th>
                <th className="px-4 py-3 text-left text-xs font-medium uppercase tracking-wider text-gray-500">
                  Created
                </th>
                <th className="px-4 py-3 text-left text-xs font-medium uppercase tracking-wider text-gray-500">
                  Status
                </th>
                <th className="px-4 py-3 text-right text-xs font-medium uppercase tracking-wider text-gray-500">
                  Actions
                </th>
              </tr>
            </thead>
            <tbody className="divide-y divide-gray-200 bg-white dark:divide-gray-700 dark:bg-gray-900">
              {requests.length === 0 ? (
                <tr>
                  <td colSpan={5} className="px-4 py-8 text-center text-sm text-gray-500">
                    No pending requests.
                  </td>
                </tr>
              ) : (
                requests.map((req) => (
                  <tr key={req.id}>
                    <td className="whitespace-nowrap px-4 py-3 text-sm font-medium text-gray-900 dark:text-gray-100">
                      {req.request_type}
                    </td>
                    <td className="whitespace-nowrap px-4 py-3 text-sm text-gray-500 dark:text-gray-400">
                      {req.requestor_id}
                    </td>
                    <td className="whitespace-nowrap px-4 py-3 text-sm text-gray-500 dark:text-gray-400">
                      {new Date(req.created_at).toLocaleDateString()}
                    </td>
                    <td className="whitespace-nowrap px-4 py-3 text-sm">
                      <span
                        className={cn(
                          "inline-flex rounded-full px-2 py-0.5 text-xs font-medium",
                          statusColor[req.status?.toLowerCase() ?? ""] ?? "bg-gray-100 text-gray-700",
                        )}
                      >
                        {req.status}
                      </span>
                    </td>
                    <td className="whitespace-nowrap px-4 py-3 text-right text-sm">
                      {req.status?.toLowerCase() === "pending" && (
                        <div className="flex justify-end gap-2">
                          <button
                            onClick={() =>
                              setDialogState({
                                open: true,
                                requestId: req.id,
                                action: "approve",
                              })
                            }
                            className="inline-flex items-center gap-1 rounded-md bg-green-50 px-2 py-1 text-xs font-medium text-green-700 hover:bg-green-100 dark:bg-green-900/30 dark:text-green-400 dark:hover:bg-green-900/50"
                            title="Approve"
                          >
                            <Check className="h-3 w-3" /> Approve
                          </button>
                          <button
                            onClick={() =>
                              setDialogState({
                                open: true,
                                requestId: req.id,
                                action: "reject",
                              })
                            }
                            className="inline-flex items-center gap-1 rounded-md bg-red-50 px-2 py-1 text-xs font-medium text-red-700 hover:bg-red-100 dark:bg-red-900/30 dark:text-red-400 dark:hover:bg-red-900/50"
                            title="Reject"
                          >
                            <XIcon className="h-3 w-3" /> Reject
                          </button>
                        </div>
                      )}
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      )}

      <ActionDialog
        open={dialogState.open}
        onClose={() => setDialogState((s) => ({ ...s, open: false }))}
        requestId={dialogState.requestId}
        action={dialogState.action}
      />
    </div>
  )
}
