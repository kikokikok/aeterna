import { useState } from "react"
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query"
import { Network, ChevronRight, ChevronDown, Users, Plus, Loader2, X } from "lucide-react"
import { cn } from "@/lib/utils"
import { apiClient } from "@/api/client"
import type { OrganizationalUnit } from "@/api/types"

interface OrgNodeProps {
  unit: OrganizationalUnit
  children: OrganizationalUnit[]
  allUnits: OrganizationalUnit[]
  onSelect: (unit: OrganizationalUnit) => void
  selectedId: string | null
}

function OrgNode({ unit, children, allUnits, onSelect, selectedId }: OrgNodeProps) {
  const [expanded, setExpanded] = useState(true)
  const hasChildren = children.length > 0

  return (
    <div>
      <div
        className={cn(
          "flex cursor-pointer items-center gap-2 rounded-md px-3 py-2 text-sm hover:bg-gray-100 dark:hover:bg-gray-700",
          selectedId === unit.id && "bg-blue-50 dark:bg-blue-900/20",
        )}
        onClick={() => onSelect(unit)}
      >
        <button
          onClick={(e) => {
            e.stopPropagation()
            setExpanded(!expanded)
          }}
          className="flex h-5 w-5 items-center justify-center"
        >
          {hasChildren ? (
            expanded ? (
              <ChevronDown className="h-4 w-4 text-gray-400" />
            ) : (
              <ChevronRight className="h-4 w-4 text-gray-400" />
            )
          ) : (
            <span className="h-4 w-4" />
          )}
        </button>
        <span className="font-medium text-gray-900 dark:text-gray-100">{unit.name}</span>
        <span className="rounded bg-gray-100 px-1.5 py-0.5 text-xs text-gray-500 dark:bg-gray-700 dark:text-gray-400">
          {unit.unitType}
        </span>
      </div>
      {expanded && hasChildren && (
        <div className="ml-6 border-l border-gray-200 pl-2 dark:border-gray-700">
          {children.map((child) => (
            <OrgNode
              key={child.id}
              unit={child}
              children={allUnits.filter((u) => u.parentId === child.id)}
              allUnits={allUnits}
              onSelect={onSelect}
              selectedId={selectedId}
            />
          ))}
        </div>
      )}
    </div>
  )
}

function CreateOrgDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const queryClient = useQueryClient()
  const [name, setName] = useState("")
  const [unitType, setUnitType] = useState("Organization")

  const create = useMutation({
    mutationFn: (data: { name: string; unit_type: string }) =>
      apiClient.post("/api/v1/org", data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["organizations"] })
      setName("")
      onClose()
    },
  })

  if (!open) return null

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="w-full max-w-md rounded-lg bg-white p-6 shadow-xl dark:bg-gray-800">
        <div className="mb-4 flex items-center justify-between">
          <h2 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
            Create Organization
          </h2>
          <button onClick={onClose} className="text-gray-400 hover:text-gray-600">
            <X className="h-5 w-5" />
          </button>
        </div>
        <form
          onSubmit={(e) => {
            e.preventDefault()
            create.mutate({ name, unit_type: unitType })
          }}
        >
          <div className="space-y-4">
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">Name</label>
              <input
                type="text"
                value={name}
                onChange={(e) => setName(e.target.value)}
                required
                className="mt-1 block w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-700 dark:text-gray-100"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">Type</label>
              <select
                value={unitType}
                onChange={(e) => setUnitType(e.target.value)}
                className="mt-1 block w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-700 dark:text-gray-100"
              >
                <option value="Organization">Organization</option>
                <option value="Team">Team</option>
                <option value="Project">Project</option>
              </select>
            </div>
          </div>
          {create.isError && (
            <p className="mt-2 text-sm text-red-600">Failed to create. Please try again.</p>
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
              disabled={create.isPending}
              className="inline-flex items-center gap-2 rounded-md bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 disabled:opacity-50"
            >
              {create.isPending && <Loader2 className="h-4 w-4 animate-spin" />}
              Create
            </button>
          </div>
        </form>
      </div>
    </div>
  )
}

function DetailPanel({ unit }: { unit: OrganizationalUnit }) {
  const { data: members, isLoading } = useQuery<Array<{ id: string; name: string; email: string }>>({
    queryKey: ["org", unit.id, "members"],
    queryFn: () => apiClient.get(`/api/v1/org/${unit.id}/members`),
    enabled: !!unit.id,
  })

  return (
    <div className="rounded-lg border border-gray-200 bg-white p-5 dark:border-gray-700 dark:bg-gray-800">
      <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100">{unit.name}</h3>
      <p className="mt-1 text-sm text-gray-500 dark:text-gray-400">
        Type: {unit.unitType} | ID: {unit.id?.slice(0, 8)}
      </p>

      <div className="mt-4">
        <h4 className="flex items-center gap-2 text-sm font-medium text-gray-700 dark:text-gray-300">
          <Users className="h-4 w-4" /> Members
        </h4>
        {isLoading ? (
          <div className="mt-2 flex justify-center">
            <Loader2 className="h-4 w-4 animate-spin text-gray-400" />
          </div>
        ) : (
          <div className="mt-2 space-y-2">
            {(members ?? []).length === 0 ? (
              <p className="text-sm text-gray-400">No members</p>
            ) : (
              (members ?? []).map((m) => (
                <div key={m.id} className="flex items-center gap-2 text-sm">
                  <div className="flex h-6 w-6 items-center justify-center rounded-full bg-blue-100 text-xs font-medium text-blue-700 dark:bg-blue-900 dark:text-blue-300">
                    {m.name.charAt(0).toUpperCase()}
                  </div>
                  <span className="text-gray-900 dark:text-gray-100">{m.name}</span>
                  <span className="text-gray-400">{m.email}</span>
                </div>
              ))
            )}
          </div>
        )}
      </div>
    </div>
  )
}

export function Component() {
  return <OrgTreePage />
}

export default function OrgTreePage() {
  const [dialogOpen, setDialogOpen] = useState(false)
  const [selected, setSelected] = useState<OrganizationalUnit | null>(null)

  const { data, isLoading, error, refetch } = useQuery<
    OrganizationalUnit[] | { items: OrganizationalUnit[] }
  >({
    queryKey: ["organizations"],
    queryFn: () => apiClient.get("/api/v1/org"),
  })

  const units: OrganizationalUnit[] = Array.isArray(data) ? data : (data?.items ?? [])
  const roots = units.filter((u) => !u.parentId)

  return (
    <div>
      <div className="mb-6 flex items-center justify-between">
        <div className="flex items-center gap-3">
          <Network className="h-6 w-6 text-gray-400" />
          <h1 className="text-2xl font-semibold text-gray-900 dark:text-gray-100">Organizations</h1>
        </div>
        <button
          onClick={() => setDialogOpen(true)}
          className="inline-flex items-center gap-2 rounded-md bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700"
        >
          <Plus className="h-4 w-4" />
          Create Org
        </button>
      </div>

      {isLoading && (
        <div className="flex justify-center p-8">
          <Loader2 className="h-6 w-6 animate-spin text-gray-400" />
        </div>
      )}

      {error && (
        <div className="p-8 text-center text-red-600">
          Failed to load organizations.{" "}
          <button onClick={() => refetch()} className="underline">
            Retry
          </button>
        </div>
      )}

      {!isLoading && !error && (
        <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
          <div className="rounded-lg border border-gray-200 bg-white p-4 dark:border-gray-700 dark:bg-gray-800">
            {roots.length === 0 ? (
              <p className="py-8 text-center text-sm text-gray-500">No organizations found.</p>
            ) : (
              roots.map((root) => (
                <OrgNode
                  key={root.id}
                  unit={root}
                  children={units.filter((u) => u.parentId === root.id)}
                  allUnits={units}
                  onSelect={setSelected}
                  selectedId={selected?.id ?? null}
                />
              ))
            )}
          </div>

          <div>
            {selected ? (
              <DetailPanel unit={selected} />
            ) : (
              <div className="rounded-lg border border-dashed border-gray-300 p-8 text-center text-sm text-gray-400 dark:border-gray-600">
                Select an organization, team, or project to view details.
              </div>
            )}
          </div>
        </div>
      )}

      <CreateOrgDialog open={dialogOpen} onClose={() => setDialogOpen(false)} />
    </div>
  )
}
