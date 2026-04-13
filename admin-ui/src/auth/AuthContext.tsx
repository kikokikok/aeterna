import {
  createContext,
  useContext,
  useEffect,
  useState,
  useCallback,
  useRef,
  type ReactNode,
} from "react"
import type {
  AuthTokens,
  UserProfile,
  TenantRecord,
  RoleAssignment,
  AdminSession,
} from "@/api/types"
import {
  getStoredTokens,
  storeTokens,
  clearTokens,
  isTokenExpired,
  shouldRefresh,
  refreshTokens,
} from "@/auth/token-manager"
import { apiClient } from "@/api/client"

interface AuthState {
  user: UserProfile | null
  tokens: AuthTokens | null
  tenants: TenantRecord[]
  roles: RoleAssignment[]
  isAuthenticated: boolean
  isPlatformAdmin: boolean
  isTenantAdmin: boolean
  activeTenantId: string | null
  isLoading: boolean
}

interface AuthContextValue extends AuthState {
  login: (tokens: AuthTokens) => Promise<void>
  logout: () => void
  setActiveTenant: (tenantId: string | null) => void
}

const AuthContext = createContext<AuthContextValue | null>(null)

const initialState: AuthState = {
  user: null,
  tokens: null,
  tenants: [],
  roles: [],
  isAuthenticated: false,
  isPlatformAdmin: false,
  isTenantAdmin: false,
  activeTenantId: null,
  isLoading: true,
}

export function AuthProvider({ children }: { children: ReactNode }) {
  const [state, setState] = useState<AuthState>(initialState)
  const refreshTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  const clearRefreshTimer = useCallback(() => {
    if (refreshTimerRef.current) {
      clearTimeout(refreshTimerRef.current)
      refreshTimerRef.current = null
    }
  }, [])

  const scheduleRefresh = useCallback(
    (tokens: AuthTokens) => {
      clearRefreshTimer()
      if (!tokens.stored_at) return

      const now = Math.floor(Date.now() / 1000)
      const expiresAt = tokens.stored_at + tokens.expires_in
      // Refresh 60 seconds before expiry, minimum 10 seconds from now
      const refreshAt = Math.max(expiresAt - 60, now + 10)
      const delayMs = (refreshAt - now) * 1000

      refreshTimerRef.current = setTimeout(async () => {
        try {
          const newTokens = await refreshTokens(tokens.refresh_token)
          storeTokens(newTokens)
          setState((prev) => ({ ...prev, tokens: newTokens }))
          scheduleRefresh(newTokens)
        } catch {
          clearTokens()
          setState({ ...initialState, isLoading: false })
        }
      }, delayMs)
    },
    [clearRefreshTimer],
  )

  const fetchSession = useCallback(async (): Promise<AdminSession> => {
    return apiClient.get<AdminSession>("/api/v1/auth/session")
  }, [])

  const login = useCallback(
    async (tokens: AuthTokens) => {
      storeTokens(tokens)
      try {
        const session = await fetchSession()
        const isPlatformAdmin = session.is_platform_admin
        const isTenantAdmin =
          isPlatformAdmin ||
          session.roles.some((r) => r.role === "TenantAdmin")
        const activeTenantId =
          session.tenants.length > 0 ? session.tenants[0].id : null

        if (activeTenantId) {
          apiClient.setTargetTenant(activeTenantId)
        }

        setState({
          user: session.user,
          tokens,
          tenants: session.tenants,
          roles: session.roles,
          isAuthenticated: true,
          isPlatformAdmin,
          isTenantAdmin,
          activeTenantId,
          isLoading: false,
        })
        scheduleRefresh(tokens)
      } catch {
        clearTokens()
        setState({ ...initialState, isLoading: false })
      }
    },
    [fetchSession, scheduleRefresh],
  )

  const logout = useCallback(() => {
    clearRefreshTimer()
    clearTokens()
    apiClient.setTargetTenant(null)
    setState({ ...initialState, isLoading: false })
  }, [clearRefreshTimer])

  const setActiveTenant = useCallback((tenantId: string | null) => {
    apiClient.setTargetTenant(tenantId)
    setState((prev) => ({ ...prev, activeTenantId: tenantId }))
  }, [])

  // On mount: check stored tokens
  useEffect(() => {
    const tokens = getStoredTokens()
    if (!tokens || isTokenExpired(tokens)) {
      clearTokens()
      setState({ ...initialState, isLoading: false })
      return
    }

    if (shouldRefresh(tokens)) {
      refreshTokens(tokens.refresh_token)
        .then((newTokens) => {
          storeTokens(newTokens)
          return login(newTokens)
        })
        .catch(() => {
          clearTokens()
          setState({ ...initialState, isLoading: false })
        })
    } else {
      login(tokens).catch(() => {
        setState({ ...initialState, isLoading: false })
      })
    }

    return () => clearRefreshTimer()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  return (
    <AuthContext.Provider
      value={{ ...state, login, logout, setActiveTenant }}
    >
      {children}
    </AuthContext.Provider>
  )
}

export function useAuth(): AuthContextValue {
  const ctx = useContext(AuthContext)
  if (!ctx) {
    throw new Error("useAuth must be used within an AuthProvider")
  }
  return ctx
}
