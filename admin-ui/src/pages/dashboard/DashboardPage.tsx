import { Link } from "react-router-dom"
import { useQuery } from "@tanstack/react-query"
import {
  Activity,
  AlertCircle,
  CheckCircle2,
  Clock,
  Building2,
  Users,
  Brain,
  BookOpen,
  Download,
} from "lucide-react"
import { cn } from "@/lib/utils"
import { apiClient } from "@/api/client"
import { useAuth } from "@/auth/AuthContext"
import type { HealthResponse, GovernanceRequest } from "@/api/types"

interface ExportJob {
  jobId: string
  status: string
  createdAt: string
  format?: string
}

function HealthCard() {
  const { data: health, isError } = useQuery<HealthResponse>({
    queryKey: ["health"],
    queryFn: () => apiClient.get("/health"),
    refetchInterval: 30_000,
  })

  const statusColor = isError
    ? "text-red-500"
    : health?.status === "healthy"
      ? "text-green-500"
      : "text-yellow-500"

  const StatusIcon =
    isError ? AlertCircle : health?.status === "healthy" ? CheckCircle2 : AlertCircle

  return (
    <div className="rounded-lg border border-gray-200 bg-white p-5 dark:border-gray-700 dark:bg-gray-800">
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-medium text-gray-500 dark:text-gray-400">System Health</h3>
        <StatusIcon className={cn("h-5 w-5", statusColor)} />
      </div>
      <div className="mt-3">
        <span className={cn("text-lg font-semibold capitalize", statusColor)}>
          {isError ? "Unreachable" : health?.status ?? "Loading..."}
        </span>
      </div>
      {health?.components && (
        <div className="mt-3 space-y-1">
          {Object.entries(health.components).map(([name, comp]) => (
            <div key={name} className="flex items-center justify-between text-xs">
              <span className="text-gray-500 dark:text-gray-400">{name}</span>
              <span
                className={cn(
                  "font-medium",
                  comp.status === "healthy" ? "text-green-600" : "text-red-600",
                )}
              >
                {comp.status}
              </span>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}

function PendingApprovalsCard() {
  const { isAuthenticated } = useAuth()
  const { data } = useQuery<GovernanceRequest[]>({
    queryKey: ["governance", "pending"],
    queryFn: () => apiClient.get("/api/v1/govern/pending"),
    refetchInterval: 30_000,
    enabled: isAuthenticated,
  })

  const count = data?.length ?? 0

  return (
    <div className="rounded-lg border border-gray-200 bg-white p-5 dark:border-gray-700 dark:bg-gray-800">
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-medium text-gray-500 dark:text-gray-400">Pending Approvals</h3>
        <Clock className="h-5 w-5 text-gray-400" />
      </div>
      <div className="mt-3">
        <span className="text-2xl font-semibold text-gray-900 dark:text-gray-100">{count}</span>
        <span className="ml-1 text-sm text-gray-500 dark:text-gray-400">requests</span>
      </div>
      <div className="mt-3">
        <Link
          to="/admin/governance"
          className="text-sm font-medium text-blue-600 hover:text-blue-700 dark:text-blue-400"
        >
          View all requests
        </Link>
      </div>
    </div>
  )
}

function QuickStatsCard() {
  const { isAuthenticated } = useAuth()
  const { data: stats } = useQuery<{
    tenantCount: number
    userCount: number
    memoryCount: number
    knowledgeCount: number
  }>({
    queryKey: ["admin", "stats"],
    queryFn: () => apiClient.get("/api/v1/admin/stats"),
    refetchInterval: 30_000,
    enabled: isAuthenticated,
  })

  const items = [
    { label: "Tenants", value: stats?.tenantCount, icon: Building2 },
    { label: "Users", value: stats?.userCount, icon: Users },
    { label: "Memories", value: stats?.memoryCount, icon: Brain },
    { label: "Knowledge", value: stats?.knowledgeCount, icon: BookOpen },
  ]

  return (
    <div className="rounded-lg border border-gray-200 bg-white p-5 dark:border-gray-700 dark:bg-gray-800">
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-medium text-gray-500 dark:text-gray-400">Quick Stats</h3>
        <Activity className="h-5 w-5 text-gray-400" />
      </div>
      <div className="mt-3 grid grid-cols-2 gap-4">
        {items.map((item) => (
          <div key={item.label} className="flex items-center gap-2">
            <item.icon className="h-4 w-4 text-gray-400" />
            <div>
              <div className="text-lg font-semibold text-gray-900 dark:text-gray-100">
                {item.value ?? "N/A"}
              </div>
              <div className="text-xs text-gray-500 dark:text-gray-400">{item.label}</div>
            </div>
          </div>
        ))}
      </div>
    </div>
  )
}

function RecentExportsCard() {
  const { isAuthenticated } = useAuth()
  const { data } = useQuery<{ jobs: ExportJob[] }>({
    queryKey: ["admin", "exports"],
    queryFn: () => apiClient.get("/api/v1/admin/exports"),
    refetchInterval: 30_000,
    enabled: isAuthenticated,
  })

  const jobs = (data?.jobs ?? []).slice(0, 3)

  const statusBadge = (status: string) => {
    const colors: Record<string, string> = {
      completed: "bg-green-100 text-green-700 dark:bg-green-900 dark:text-green-300",
      running: "bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-300",
      failed: "bg-red-100 text-red-700 dark:bg-red-900 dark:text-red-300",
      pending: "bg-yellow-100 text-yellow-700 dark:bg-yellow-900 dark:text-yellow-300",
    }
    return (
      <span
        className={cn(
          "inline-flex rounded-full px-2 py-0.5 text-xs font-medium",
          colors[status?.toLowerCase() ?? ""] ?? "bg-gray-100 text-gray-700",
        )}
      >
        {status}
      </span>
    )
  }

  return (
    <div className="rounded-lg border border-gray-200 bg-white p-5 dark:border-gray-700 dark:bg-gray-800">
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-medium text-gray-500 dark:text-gray-400">Recent Exports</h3>
        <Download className="h-5 w-5 text-gray-400" />
      </div>
      <div className="mt-3 space-y-3">
        {jobs.length === 0 ? (
          <p className="text-sm text-gray-400">No recent exports</p>
        ) : (
          jobs.map((job) => (
            <div key={job.jobId} className="flex items-center justify-between text-sm">
              <div>
                <span className="text-gray-700 dark:text-gray-300">{job.jobId?.slice(0, 8)}</span>
                <span className="ml-2 text-xs text-gray-400">
                  {new Date(job.createdAt).toLocaleDateString()}
                </span>
              </div>
              {statusBadge(job.status)}
            </div>
          ))
        )}
      </div>
    </div>
  )
}

export default function DashboardPage() {
  return (
    <div>
      <h1 className="mb-6 text-2xl font-semibold text-gray-900 dark:text-gray-100">Dashboard</h1>
      <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
        <HealthCard />
        <PendingApprovalsCard />
        <QuickStatsCard />
        <RecentExportsCard />
      </div>
    </div>
  )
}
