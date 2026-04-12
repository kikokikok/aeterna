import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query"
import { Settings2, Loader2, CheckCircle2, XCircle, RefreshCw, Download } from "lucide-react"
import { cn } from "@/lib/utils"
import { apiClient } from "@/api/client"
import type { HealthResponse, ReadinessResponse } from "@/api/types"

interface ExportJob {
  id: string
  status: string
  created_at: string
  format?: string
}

function ComponentCard({
  name,
  status,
  message,
}: {
  name: string
  status: string
  message?: string
}) {
  const isHealthy = status === "healthy"

  return (
    <div className="rounded-lg border border-gray-200 bg-white p-4 dark:border-gray-700 dark:bg-gray-800">
      <div className="flex items-center justify-between">
        <h3 className="font-medium text-gray-900 dark:text-gray-100 capitalize">{name}</h3>
        {isHealthy ? (
          <CheckCircle2 className="h-5 w-5 text-green-500" />
        ) : (
          <XCircle className="h-5 w-5 text-red-500" />
        )}
      </div>
      <div className="mt-2">
        <span
          className={cn(
            "inline-flex rounded-full px-2 py-0.5 text-xs font-medium",
            isHealthy
              ? "bg-green-100 text-green-700 dark:bg-green-900 dark:text-green-300"
              : "bg-red-100 text-red-700 dark:bg-red-900 dark:text-red-300",
          )}
        >
          {status}
        </span>
      </div>
      {message && (
        <p className="mt-2 text-xs text-gray-500 dark:text-gray-400">{message}</p>
      )}
    </div>
  )
}

export function Component() {
  return <SystemHealthPage />
}

export default function SystemHealthPage() {
  const queryClient = useQueryClient()

  const {
    data: health,
    isLoading: healthLoading,
    error: healthError,
    refetch: refetchHealth,
  } = useQuery<HealthResponse>({
    queryKey: ["health"],
    queryFn: () => apiClient.get("/health"),
    refetchInterval: 30_000,
  })

  const {
    data: ready,
    isLoading: readyLoading,
  } = useQuery<ReadinessResponse>({
    queryKey: ["ready"],
    queryFn: () => apiClient.get("/ready"),
    refetchInterval: 30_000,
  })

  const {
    data: exportsData,
    isLoading: exportsLoading,
  } = useQuery<{ items: ExportJob[] }>({
    queryKey: ["admin", "exports"],
    queryFn: () => apiClient.get("/api/v1/admin/exports"),
  })

  const triggerExport = useMutation({
    mutationFn: () => apiClient.post("/api/v1/admin/exports", { format: "json" }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["admin", "exports"] })
    },
  })

  const exports = exportsData?.items ?? []

  const statusColor: Record<string, string> = {
    completed: "bg-green-100 text-green-700 dark:bg-green-900 dark:text-green-300",
    running: "bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-300",
    failed: "bg-red-100 text-red-700 dark:bg-red-900 dark:text-red-300",
    pending: "bg-yellow-100 text-yellow-700 dark:bg-yellow-900 dark:text-yellow-300",
  }

  return (
    <div>
      <div className="mb-6 flex items-center justify-between">
        <div className="flex items-center gap-3">
          <Settings2 className="h-6 w-6 text-gray-400" />
          <h1 className="text-2xl font-semibold text-gray-900 dark:text-gray-100">System Health</h1>
        </div>
        <button
          onClick={() => refetchHealth()}
          className="inline-flex items-center gap-2 rounded-md border border-gray-300 px-3 py-2 text-sm font-medium text-gray-700 hover:bg-gray-50 dark:border-gray-600 dark:text-gray-300 dark:hover:bg-gray-700"
        >
          <RefreshCw className="h-4 w-4" />
          Refresh
        </button>
      </div>

      {/* Overall Status */}
      <div className="mb-6 rounded-lg border border-gray-200 bg-white p-5 dark:border-gray-700 dark:bg-gray-800">
        <div className="flex items-center justify-between">
          <div>
            <h2 className="text-lg font-semibold text-gray-900 dark:text-gray-100">Overall Status</h2>
            {healthLoading ? (
              <Loader2 className="mt-2 h-5 w-5 animate-spin text-gray-400" />
            ) : healthError ? (
              <span className="mt-2 inline-flex rounded-full bg-red-100 px-3 py-1 text-sm font-medium text-red-700 dark:bg-red-900 dark:text-red-300">
                Unreachable
              </span>
            ) : (
              <span
                className={cn(
                  "mt-2 inline-flex rounded-full px-3 py-1 text-sm font-medium capitalize",
                  health?.status === "healthy"
                    ? "bg-green-100 text-green-700 dark:bg-green-900 dark:text-green-300"
                    : "bg-yellow-100 text-yellow-700 dark:bg-yellow-900 dark:text-yellow-300",
                )}
              >
                {health?.status}
              </span>
            )}
          </div>
          <div>
            <span className="text-sm text-gray-500 dark:text-gray-400">
              Ready: {readyLoading ? "..." : ready?.ready ? "Yes" : "No"}
            </span>
          </div>
        </div>
      </div>

      {/* Component Cards */}
      <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">Components</h2>
      {healthLoading ? (
        <div className="flex justify-center p-8">
          <Loader2 className="h-6 w-6 animate-spin text-gray-400" />
        </div>
      ) : health?.components ? (
        <div className="mb-8 grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
          {Object.entries(health.components).map(([name, comp]) => (
            <ComponentCard
              key={name}
              name={name}
              status={comp.status}
              message={comp.message}
            />
          ))}
        </div>
      ) : (
        <p className="mb-8 text-sm text-gray-500">No component data available.</p>
      )}

      {/* Readiness Checks */}
      {ready?.checks && Object.keys(ready.checks).length > 0 && (
        <>
          <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">
            Readiness Checks
          </h2>
          <div className="mb-8 grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
            {Object.entries(ready.checks).map(([name, ok]) => (
              <div
                key={name}
                className="flex items-center justify-between rounded-lg border border-gray-200 bg-white px-4 py-3 dark:border-gray-700 dark:bg-gray-800"
              >
                <span className="text-sm font-medium text-gray-900 capitalize dark:text-gray-100">
                  {name}
                </span>
                {ok ? (
                  <CheckCircle2 className="h-5 w-5 text-green-500" />
                ) : (
                  <XCircle className="h-5 w-5 text-red-500" />
                )}
              </div>
            ))}
          </div>
        </>
      )}

      {/* Export/Import */}
      <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">
        Export / Import
      </h2>
      <div className="rounded-lg border border-gray-200 bg-white p-5 dark:border-gray-700 dark:bg-gray-800">
        <div className="mb-4 flex items-center justify-between">
          <span className="text-sm text-gray-500 dark:text-gray-400">Recent export jobs</span>
          <button
            onClick={() => triggerExport.mutate()}
            disabled={triggerExport.isPending}
            className="inline-flex items-center gap-2 rounded-md bg-blue-600 px-3 py-2 text-sm font-medium text-white hover:bg-blue-700 disabled:opacity-50"
          >
            {triggerExport.isPending ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : (
              <Download className="h-4 w-4" />
            )}
            New Export
          </button>
        </div>

        {exportsLoading ? (
          <div className="flex justify-center p-4">
            <Loader2 className="h-5 w-5 animate-spin text-gray-400" />
          </div>
        ) : exports.length === 0 ? (
          <p className="py-4 text-center text-sm text-gray-400">No export jobs.</p>
        ) : (
          <div className="space-y-2">
            {exports.map((job) => (
              <div
                key={job.id}
                className="flex items-center justify-between rounded-md bg-gray-50 px-3 py-2 dark:bg-gray-700"
              >
                <div>
                  <span className="font-mono text-sm text-gray-700 dark:text-gray-300">
                    {job.id.slice(0, 12)}
                  </span>
                  <span className="ml-2 text-xs text-gray-400">
                    {new Date(job.created_at).toLocaleString()}
                  </span>
                </div>
                <span
                  className={cn(
                    "inline-flex rounded-full px-2 py-0.5 text-xs font-medium",
                    statusColor[job.status.toLowerCase()] ?? "bg-gray-100 text-gray-700",
                  )}
                >
                  {job.status}
                </span>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  )
}
