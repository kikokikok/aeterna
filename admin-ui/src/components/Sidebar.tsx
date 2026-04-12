import { NavLink } from "react-router-dom"
import {
  LayoutDashboard,
  Building2,
  Network,
  Users,
  BookOpen,
  Brain,
  Shield,
  ScrollText,
  Settings2,
  Wrench,
  ChevronLeft,
  ChevronRight,
} from "lucide-react"
import { cn } from "@/lib/utils"
import { useAuth } from "@/auth/AuthContext"

interface SidebarProps {
  collapsed: boolean
  onToggle: () => void
}

interface NavItem {
  label: string
  path: string
  icon: React.ComponentType<{ className?: string }>
  requirePlatformAdmin?: boolean
}

const navItems: NavItem[] = [
  { label: "Dashboard", path: "/admin", icon: LayoutDashboard },
  { label: "Tenants", path: "/admin/tenants", icon: Building2, requirePlatformAdmin: true },
  { label: "Organizations", path: "/admin/organizations", icon: Network },
  { label: "Users", path: "/admin/users", icon: Users },
  { label: "Knowledge", path: "/admin/knowledge", icon: BookOpen },
  { label: "Memory", path: "/admin/memory", icon: Brain },
  { label: "Governance", path: "/admin/governance", icon: Shield },
  { label: "Policies", path: "/admin/policies", icon: ScrollText },
  { label: "Admin", path: "/admin/admin", icon: Settings2 },
  { label: "Settings", path: "/admin/settings", icon: Wrench },
]

export function Sidebar({ collapsed, onToggle }: SidebarProps) {
  const { isPlatformAdmin } = useAuth()

  const visibleItems = navItems.filter(
    (item) => !item.requirePlatformAdmin || isPlatformAdmin,
  )

  return (
    <aside
      className={cn(
        "flex h-full flex-col border-r border-gray-200 bg-white transition-all duration-200",
        collapsed ? "w-16" : "w-60",
      )}
    >
      <div className="flex h-14 items-center justify-between border-b border-gray-200 px-3">
        {!collapsed && (
          <span className="text-sm font-semibold text-gray-900">Aeterna</span>
        )}
        <button
          onClick={onToggle}
          className="rounded-md p-1.5 text-gray-400 hover:bg-gray-100 hover:text-gray-600"
          aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
        >
          {collapsed ? (
            <ChevronRight className="h-4 w-4" />
          ) : (
            <ChevronLeft className="h-4 w-4" />
          )}
        </button>
      </div>

      <nav className="flex-1 space-y-1 overflow-y-auto p-2">
        {visibleItems.map((item) => (
          <NavLink
            key={item.path}
            to={item.path}
            end={item.path === "/admin"}
            className={({ isActive }) =>
              cn(
                "flex items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors",
                isActive
                  ? "bg-gray-100 text-gray-900"
                  : "text-gray-600 hover:bg-gray-50 hover:text-gray-900",
                collapsed && "justify-center px-0",
              )
            }
            title={collapsed ? item.label : undefined}
          >
            <item.icon className="h-5 w-5 flex-shrink-0" />
            {!collapsed && <span>{item.label}</span>}
          </NavLink>
        ))}
      </nav>
    </aside>
  )
}
