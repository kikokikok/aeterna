import { useQuery } from "@tanstack/react-query"
import { Wrench, Loader2, Server, Database, Cpu, Key } from "lucide-react"
import { cn } from "@/lib/utils"
import { apiClient } from "@/api/client"
import type { AdminSession } from "@/api/types"

function InfoRow({ label, value }: { label: string; value: string | undefined | null }) {
  return (
    <div className="flex items-center justify-between border-b border-gray-100 py-2 last:border-0 dark:border-gray-700">
      <span className="text-sm text-gray-500 dark:text-gray-400">{label}</span>
      <span className="text-sm font-medium text-gray-900 dark:text-gray-100">
        {value ?? "N/A"}
      </span>
    </div>
  )
}

export function Component() {
  return <ServerConfigPage />
}

export default function ServerConfigPage() {
  const {
    data: session,
    isLoading,
    error,
    refetch,
  } = useQuery<AdminSession>({
    queryKey: ["session"],
    queryFn: () => apiClient.post("/api/v1/auth/admin/session"),
  })

  const { data: health } = useQuery<{
    status: string
    version?: string
    deployment_mode?: string
    features?: string[]
    components?: Record<string, { status: string }>
  }>({
    queryKey: ["health-config"],
    queryFn: () => apiClient.get("/health"),
  })

  return (
    <div>
      <div className="mb-6 flex items-center gap-3">
        <Wrench className="h-6 w-6 text-gray-400" />
        <h1 className="text-2xl font-semibold text-gray-900 dark:text-gray-100">Settings</h1>
      </div>

      {isLoading && (
        <div className="flex justify-center p-8">
          <Loader2 className="h-6 w-6 animate-spin text-gray-400" />
        </div>
      )}

      {error && (
        <div className="p-8 text-center text-red-600">
          Failed to load configuration.{" "}
          <button onClick={() => refetch()} className="underline">
            Retry
          </button>
        </div>
      )}

      {!isLoading && !error && (
        <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
          {/* Server Info */}
          <div className="rounded-lg border border-gray-200 bg-white p-5 dark:border-gray-700 dark:bg-gray-800">
            <div className="mb-4 flex items-center gap-2">
              <Server className="h-5 w-5 text-gray-400" />
              <h2 className="text-sm font-medium text-gray-700 dark:text-gray-300">Server Info</h2>
            </div>
            <div className="space-y-0">
              <InfoRow label="Version" value={health?.version} />
              <InfoRow
                label="Deployment Mode"
                value={health?.deployment_mode}
              />
              <InfoRow label="Status" value={health?.status} />
            </div>
          </div>

          {/* Current User */}
          <div className="rounded-lg border border-gray-200 bg-white p-5 dark:border-gray-700 dark:bg-gray-800">
            <div className="mb-4 flex items-center gap-2">
              <Key className="h-5 w-5 text-gray-400" />
              <h2 className="text-sm font-medium text-gray-700 dark:text-gray-300">Current User</h2>
            </div>
            <div className="space-y-0">
              <InfoRow label="Email" value={session?.user.email} />
              <InfoRow label="GitHub Login" value={session?.user.github_login} />
              <InfoRow
                label="Platform Admin"
                value={session?.is_platform_admin ? "Yes" : "No"}
              />
              <InfoRow
                label="Tenants"
                value={String(session?.tenants.length ?? 0)}
              />
            </div>
          </div>

          {/* Features */}
          {health?.features && health.features.length > 0 && (
            <div className="rounded-lg border border-gray-200 bg-white p-5 dark:border-gray-700 dark:bg-gray-800">
              <div className="mb-4 flex items-center gap-2">
                <Cpu className="h-5 w-5 text-gray-400" />
                <h2 className="text-sm font-medium text-gray-700 dark:text-gray-300">
                  Configured Features
                </h2>
              </div>
              <div className="flex flex-wrap gap-2">
                {health.features.map((feat) => (
                  <span
                    key={feat}
                    className="inline-flex rounded-full bg-blue-100 px-2.5 py-0.5 text-xs font-medium text-blue-700 dark:bg-blue-900 dark:text-blue-300"
                  >
                    {feat}
                  </span>
                ))}
              </div>
            </div>
          )}

          {/* Storage Backends */}
          <div className="rounded-lg border border-gray-200 bg-white p-5 dark:border-gray-700 dark:bg-gray-800">
            <div className="mb-4 flex items-center gap-2">
              <Database className="h-5 w-5 text-gray-400" />
              <h2 className="text-sm font-medium text-gray-700 dark:text-gray-300">
                Storage Backends
              </h2>
            </div>
            {health?.components ? (
              <div className="space-y-2">
                {Object.entries(health.components).map(([name, comp]) => (
                  <div
                    key={name}
                    className="flex items-center justify-between rounded-md bg-gray-50 px-3 py-2 dark:bg-gray-700"
                  >
                    <span className="text-sm font-medium text-gray-900 capitalize dark:text-gray-100">
                      {name}
                    </span>
                    <span
                      className={cn(
                        "inline-flex rounded-full px-2 py-0.5 text-xs font-medium",
                        comp.status === "healthy"
                          ? "bg-green-100 text-green-700 dark:bg-green-900 dark:text-green-300"
                          : "bg-red-100 text-red-700 dark:bg-red-900 dark:text-red-300",
                      )}
                    >
                      {comp.status}
                    </span>
                  </div>
                ))}
              </div>
            ) : (
              <p className="text-sm text-gray-400">No backend data available.</p>
            )}
          </div>
        </div>
      )}
    </div>
  )
}
