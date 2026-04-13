import { useCallback, useMemo } from "react"
import { useAuth } from "@/auth/AuthContext"
import type { TenantRecord } from "@/api/types"

export function useTenant() {
  const { tenants, activeTenantId, setActiveTenant, isPlatformAdmin } = useAuth()

  const activeTenant: TenantRecord | null = useMemo(
    () => tenants.find((t) => t.id === activeTenantId) ?? null,
    [tenants, activeTenantId],
  )

  const switchTenant = useCallback(
    (tenantId: string) => {
      const tenant = tenants.find((t) => t.id === tenantId)
      if (tenant) {
        setActiveTenant(tenantId)
      }
    },
    [tenants, setActiveTenant],
  )

  const clearTenant = useCallback(() => {
    setActiveTenant(null)
  }, [setActiveTenant])

  return {
    tenants,
    activeTenant,
    activeTenantId,
    isPlatformAdmin,
    switchTenant,
    clearTenant,
  }
}
