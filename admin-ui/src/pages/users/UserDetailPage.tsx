import { useState } from "react"
import { useParams, useNavigate } from "react-router-dom"
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query"
import { Users, ArrowLeft, Shield, Plus, Loader2, X } from "lucide-react"
import { cn } from "@/lib/utils"
import { apiClient } from "@/api/client"
import type { UserRecord, RoleAssignment, Role } from "@/api/types"

const ALL_ROLES: Role[] = [
  "PlatformAdmin",
  "TenantAdmin",
  "Admin",
  "Architect",
  "TechLead",
  "Developer",
  "Viewer",
  "Agent",
]

function GrantRoleDialog({
  open,
  onClose,
  userId,
}: {
  open: boolean
  onClose: () => void
  userId: string
}) {
  const queryClient = useQueryClient()
  const [role, setRole] = useState<Role>("Developer")
  const [scope, setScope] = useState("")

  const grantRole = useMutation({
    mutationFn: (data: { role: Role; resource_type?: string; resource_id?: string }) =>
      apiClient.post(`/api/v1/user/${userId}/roles`, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["user", userId, "roles"] })
      onClose()
    },
  })

  if (!open) return null

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="w-full max-w-md rounded-lg bg-white p-6 shadow-xl dark:bg-gray-800">
        <div className="mb-4 flex items-center justify-between">
          <h2 className="text-lg font-semibold text-gray-900 dark:text-gray-100">Grant Role</h2>
          <button onClick={onClose} className="text-gray-400 hover:text-gray-600">
            <X className="h-5 w-5" />
          </button>
        </div>
        <form
          onSubmit={(e) => {
            e.preventDefault()
            const parts = scope.split("/").filter(Boolean)
            grantRole.mutate({
              role,
              resource_type: parts[0] || undefined,
              resource_id: parts[1] || undefined,
            })
          }}
        >
          <div className="space-y-4">
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">Role</label>
              <select
                value={role}
                onChange={(e) => setRole(e.target.value as Role)}
                className="mt-1 block w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-700 dark:text-gray-100"
              >
                {ALL_ROLES.map((r) => (
                  <option key={r} value={r}>
                    {r}
                  </option>
                ))}
              </select>
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">
                Scope (optional, e.g. org/org-id)
              </label>
              <input
                type="text"
                value={scope}
                onChange={(e) => setScope(e.target.value)}
                placeholder="resource_type/resource_id"
                className="mt-1 block w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-700 dark:text-gray-100"
              />
            </div>
          </div>
          {grantRole.isError && (
            <p className="mt-2 text-sm text-red-600">Failed to grant role.</p>
          )}
          <div className="mt-6 flex justify-end gap-3">
            <button
              type="button"
              onClick={onClose}
              className="rounded-md border border-gray-300 px-4 py-2 text-sm font-medium text-gray-700 hover:bg-gray-50 dark:border-gray-600 dark:text-gray-300"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={grantRole.isPending}
              className="inline-flex items-center gap-2 rounded-md bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 disabled:opacity-50"
            >
              {grantRole.isPending && <Loader2 className="h-4 w-4 animate-spin" />}
              Grant
            </button>
          </div>
        </form>
      </div>
    </div>
  )
}

export function Component() {
  return <UserDetailPage />
}

export default function UserDetailPage() {
  const { id } = useParams()
  const navigate = useNavigate()
  const [roleDialogOpen, setRoleDialogOpen] = useState(false)

  const { data: user, isLoading, error, refetch } = useQuery<UserRecord>({
    queryKey: ["user", id],
    queryFn: () => apiClient.get(`/api/v1/user/${id}`),
    enabled: !!id,
  })

  const { data: rolesData } = useQuery<RoleAssignment[] | { items: RoleAssignment[] }>({
    queryKey: ["user", id, "roles"],
    queryFn: () => apiClient.get(`/api/v1/user/${id}/roles`),
    enabled: !!id,
  })

  const roles: RoleAssignment[] = Array.isArray(rolesData) ? rolesData : (rolesData?.items ?? [])

  const roleColor = (role: string) => {
    const colors: Record<string, string> = {
      PlatformAdmin: "bg-purple-100 text-purple-700 dark:bg-purple-900 dark:text-purple-300",
      TenantAdmin: "bg-indigo-100 text-indigo-700 dark:bg-indigo-900 dark:text-indigo-300",
      Admin: "bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-300",
      Architect: "bg-cyan-100 text-cyan-700 dark:bg-cyan-900 dark:text-cyan-300",
      TechLead: "bg-teal-100 text-teal-700 dark:bg-teal-900 dark:text-teal-300",
      Developer: "bg-green-100 text-green-700 dark:bg-green-900 dark:text-green-300",
      Viewer: "bg-gray-100 text-gray-700 dark:bg-gray-700 dark:text-gray-300",
      Agent: "bg-orange-100 text-orange-700 dark:bg-orange-900 dark:text-orange-300",
    }
    return colors[role] ?? "bg-gray-100 text-gray-700"
  }

  return (
    <div>
      <div className="mb-6 flex items-center gap-3">
        <button onClick={() => navigate("/admin/users")} className="text-gray-400 hover:text-gray-600">
          <ArrowLeft className="h-5 w-5" />
        </button>
        <Users className="h-6 w-6 text-gray-400" />
        <h1 className="text-2xl font-semibold text-gray-900 dark:text-gray-100">User Detail</h1>
      </div>

      {isLoading && (
        <div className="flex justify-center p-8">
          <Loader2 className="h-6 w-6 animate-spin text-gray-400" />
        </div>
      )}

      {error && (
        <div className="p-8 text-center text-red-600">
          Failed to load user.{" "}
          <button onClick={() => refetch()} className="underline">
            Retry
          </button>
        </div>
      )}

      {user && (
        <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
          {/* Profile Card */}
          <div className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-800">
            <div className="flex items-center gap-4">
              {user.avatarUrl ? (
                <img src={user.avatarUrl} alt="" className="h-16 w-16 rounded-full" />
              ) : (
                <div className="flex h-16 w-16 items-center justify-center rounded-full bg-blue-100 text-2xl font-medium text-blue-700 dark:bg-blue-900 dark:text-blue-300">
                  {user.name?.charAt(0)?.toUpperCase() ?? "?"}
                </div>
              )}
              <div>
                <h2 className="text-xl font-semibold text-gray-900 dark:text-gray-100">
                  {user.name}
                </h2>
                <p className="text-sm text-gray-500 dark:text-gray-400">{user.email}</p>
                <span
                  className={cn(
                    "mt-1 inline-flex rounded-full px-2 py-0.5 text-xs font-medium",
                    user.status === "active"
                      ? "bg-green-100 text-green-700 dark:bg-green-900 dark:text-green-300"
                      : "bg-gray-100 text-gray-700 dark:bg-gray-700 dark:text-gray-300",
                  )}
                >
                  {user.status}
                </span>
              </div>
            </div>
          </div>

          {/* Roles Section */}
          <div className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-800">
            <div className="mb-4 flex items-center justify-between">
              <h3 className="flex items-center gap-2 text-sm font-medium text-gray-700 dark:text-gray-300">
                <Shield className="h-4 w-4" /> Role Assignments
              </h3>
              <button
                onClick={() => setRoleDialogOpen(true)}
                className="inline-flex items-center gap-1 text-sm text-blue-600 hover:text-blue-700"
              >
                <Plus className="h-3 w-3" /> Grant Role
              </button>
            </div>
            <div className="space-y-3">
              {roles.length === 0 ? (
                <p className="text-sm text-gray-400">No roles assigned.</p>
              ) : (
                roles.map((ra, i) => (
                  <div
                    key={`${ra.role}-${ra.resource_type}-${ra.resource_id}-${i}`}
                    className="flex items-center justify-between rounded-md bg-gray-50 px-3 py-2 dark:bg-gray-700"
                  >
                    <span className={cn("inline-flex rounded-full px-2 py-0.5 text-xs font-medium", roleColor(ra.role))}>
                      {ra.role}
                    </span>
                    <span className="text-xs text-gray-500 dark:text-gray-400">
                      {ra.resource_type
                        ? `${ra.resource_type}/${ra.resource_id ?? "*"}`
                        : "Global"}
                    </span>
                  </div>
                ))
              )}
            </div>
          </div>
        </div>
      )}

      {id && (
        <GrantRoleDialog
          open={roleDialogOpen}
          onClose={() => setRoleDialogOpen(false)}
          userId={id}
        />
      )}
    </div>
  )
}
