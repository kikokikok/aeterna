import { useAuth } from "@/auth/AuthContext"
import type { Role } from "@/api/types"
import type { ReactNode } from "react"

interface RequireRoleProps {
  role: Role
  children: ReactNode
  fallback?: ReactNode
}

export function RequireRole({ role, children, fallback = null }: RequireRoleProps) {
  const { roles, isPlatformAdmin } = useAuth()

  if (isPlatformAdmin) {
    return <>{children}</>
  }

  const hasRole = roles.some((r) => r.role === role)
  if (!hasRole) {
    return <>{fallback}</>
  }

  return <>{children}</>
}
