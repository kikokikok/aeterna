import { useState, useRef, useEffect } from "react"
import { LogOut, Moon, Sun, User } from "lucide-react"
import { cn } from "@/lib/utils"
import { useAuth } from "@/auth/AuthContext"

export function UserMenu() {
  const { user, isPlatformAdmin, isTenantAdmin, logout } = useAuth()
  const [isOpen, setIsOpen] = useState(false)
  const [darkMode, setDarkMode] = useState(false)
  const menuRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    function handleClickOutside(event: MouseEvent) {
      if (menuRef.current && !menuRef.current.contains(event.target as Node)) {
        setIsOpen(false)
      }
    }
    document.addEventListener("mousedown", handleClickOutside)
    return () => document.removeEventListener("mousedown", handleClickOutside)
  }, [])

  const toggleDarkMode = () => {
    setDarkMode(!darkMode)
    document.documentElement.classList.toggle("dark")
  }

  const roleBadge = isPlatformAdmin
    ? "Platform Admin"
    : isTenantAdmin
      ? "Tenant Admin"
      : "User"

  const roleBadgeColor = isPlatformAdmin
    ? "bg-purple-100 text-purple-700"
    : isTenantAdmin
      ? "bg-blue-100 text-blue-700"
      : "bg-gray-100 text-gray-700"

  return (
    <div className="relative" ref={menuRef}>
      <button
        onClick={() => setIsOpen(!isOpen)}
        className="flex items-center gap-2 rounded-full p-1 transition-colors hover:bg-gray-100"
      >
        {user?.avatar_url ? (
          <img
            src={user.avatar_url}
            alt={user.github_login}
            className="h-8 w-8 rounded-full"
          />
        ) : (
          <div className="flex h-8 w-8 items-center justify-center rounded-full bg-gray-200">
            <User className="h-4 w-4 text-gray-500" />
          </div>
        )}
      </button>

      {isOpen && (
        <div className="absolute right-0 top-full z-50 mt-1 w-64 rounded-md border border-gray-200 bg-white shadow-lg">
          <div className="border-b border-gray-100 p-3">
            <div className="flex items-center gap-3">
              {user?.avatar_url ? (
                <img
                  src={user.avatar_url}
                  alt={user.github_login}
                  className="h-10 w-10 rounded-full"
                />
              ) : (
                <div className="flex h-10 w-10 items-center justify-center rounded-full bg-gray-200">
                  <User className="h-5 w-5 text-gray-500" />
                </div>
              )}
              <div className="min-w-0 flex-1">
                <div className="truncate text-sm font-medium text-gray-900">
                  {user?.github_login ?? "Unknown"}
                </div>
                <div className="truncate text-xs text-gray-500">
                  {user?.email ?? ""}
                </div>
              </div>
            </div>
            <div className="mt-2">
              <span
                className={cn(
                  "inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium",
                  roleBadgeColor,
                )}
              >
                {roleBadge}
              </span>
            </div>
          </div>

          <div className="p-1">
            <button
              onClick={toggleDarkMode}
              className="flex w-full items-center gap-2 rounded-md px-3 py-2 text-sm text-gray-600 transition-colors hover:bg-gray-50"
            >
              {darkMode ? (
                <Sun className="h-4 w-4" />
              ) : (
                <Moon className="h-4 w-4" />
              )}
              {darkMode ? "Light mode" : "Dark mode"}
            </button>
            <button
              onClick={() => {
                setIsOpen(false)
                logout()
              }}
              className="flex w-full items-center gap-2 rounded-md px-3 py-2 text-sm text-red-600 transition-colors hover:bg-red-50"
            >
              <LogOut className="h-4 w-4" />
              Sign out
            </button>
          </div>
        </div>
      )}
    </div>
  )
}
