import { useState, useRef, useEffect } from "react"
import { Building2, ChevronDown, Search } from "lucide-react"
import { cn } from "@/lib/utils"
import { useTenant } from "@/hooks/useTenant"

export function TenantSelector() {
  const { tenants, activeTenant, isPlatformAdmin, switchTenant } = useTenant()
  const [isOpen, setIsOpen] = useState(false)
  const [search, setSearch] = useState("")
  const dropdownRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    function handleClickOutside(event: MouseEvent) {
      if (
        dropdownRef.current &&
        !dropdownRef.current.contains(event.target as Node)
      ) {
        setIsOpen(false)
        setSearch("")
      }
    }
    document.addEventListener("mousedown", handleClickOutside)
    return () => document.removeEventListener("mousedown", handleClickOutside)
  }, [])

  const filteredTenants = tenants.filter(
    (t) =>
      t.name.toLowerCase().includes(search.toLowerCase()) ||
      t.slug.toLowerCase().includes(search.toLowerCase()),
  )

  return (
    <div className="relative" ref={dropdownRef}>
      <button
        onClick={() => setIsOpen(!isOpen)}
        className="flex items-center gap-2 rounded-md border border-gray-200 bg-white px-3 py-1.5 text-sm text-gray-700 transition-colors hover:bg-gray-50"
      >
        <Building2 className="h-4 w-4 text-gray-400" />
        <span className="max-w-[160px] truncate">
          {activeTenant?.name ?? "Select tenant"}
        </span>
        <ChevronDown className="h-3.5 w-3.5 text-gray-400" />
      </button>

      {isOpen && (
        <div className="absolute right-0 top-full z-50 mt-1 w-64 rounded-md border border-gray-200 bg-white shadow-lg">
          {isPlatformAdmin && (
            <div className="border-b border-gray-100 p-2">
              <div className="flex items-center gap-2 rounded-md border border-gray-200 px-2 py-1.5">
                <Search className="h-3.5 w-3.5 text-gray-400" />
                <input
                  type="text"
                  placeholder="Search tenants..."
                  value={search}
                  onChange={(e) => setSearch(e.target.value)}
                  className="w-full bg-transparent text-sm outline-none placeholder:text-gray-400"
                  autoFocus
                />
              </div>
            </div>
          )}

          <div className="max-h-60 overflow-y-auto p-1">
            {filteredTenants.length === 0 ? (
              <div className="px-3 py-2 text-sm text-gray-400">
                No tenants found
              </div>
            ) : (
              filteredTenants.map((tenant) => (
                <button
                  key={tenant.id}
                  onClick={() => {
                    switchTenant(tenant.id)
                    setIsOpen(false)
                    setSearch("")
                  }}
                  className={cn(
                    "flex w-full items-center gap-2 rounded-md px-3 py-2 text-left text-sm transition-colors",
                    tenant.id === activeTenant?.id
                      ? "bg-gray-100 text-gray-900"
                      : "text-gray-600 hover:bg-gray-50",
                  )}
                >
                  <Building2 className="h-4 w-4 flex-shrink-0 text-gray-400" />
                  <div className="min-w-0 flex-1">
                    <div className="truncate font-medium">{tenant.name}</div>
                    <div className="truncate text-xs text-gray-400">
                      {tenant.slug}
                    </div>
                  </div>
                </button>
              ))
            )}
          </div>
        </div>
      )}
    </div>
  )
}
