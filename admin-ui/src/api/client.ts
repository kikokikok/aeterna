import type { ApiClientConfig, AuthTokens } from "./types"
import { getStoredTokens, storeTokens, clearTokens, refreshTokens } from "@/auth/token-manager"

class ApiClient {
  private baseUrl: string
  private getTokens: () => AuthTokens | null
  private onTokenRefresh: (tokens: AuthTokens) => void
  private onUnauthorized: () => void
  private targetTenantId: string | null = null
  private refreshPromise: Promise<AuthTokens> | null = null

  constructor(config: ApiClientConfig) {
    this.baseUrl = config.baseUrl
    this.getTokens = config.getTokens
    this.onTokenRefresh = config.onTokenRefresh
    this.onUnauthorized = config.onUnauthorized
  }

  setTargetTenant(tenantId: string | null) {
    this.targetTenantId = tenantId
  }

  getTargetTenant(): string | null {
    return this.targetTenantId
  }

  private buildHeaders(tokens: AuthTokens | null): Record<string, string> {
    const headers: Record<string, string> = {
      "Content-Type": "application/json",
    }
    if (tokens) {
      headers["Authorization"] = `Bearer ${tokens.access_token}`
    }
    if (this.targetTenantId) {
      headers["X-Target-Tenant-ID"] = this.targetTenantId
    }
    return headers
  }

  private async tryRefresh(): Promise<AuthTokens> {
    if (this.refreshPromise) {
      return this.refreshPromise
    }

    const tokens = this.getTokens()
    if (!tokens?.refresh_token) {
      throw new Error("No refresh token available")
    }

    this.refreshPromise = refreshTokens(tokens.refresh_token)
      .then((newTokens) => {
        this.onTokenRefresh(newTokens)
        return newTokens
      })
      .finally(() => {
        this.refreshPromise = null
      })

    return this.refreshPromise
  }

  async request<T>(method: string, path: string, body?: unknown): Promise<T> {
    const tokens = this.getTokens()
    const url = `${this.baseUrl}${path}`

    const response = await fetch(url, {
      method,
      headers: this.buildHeaders(tokens),
      body: body ? JSON.stringify(body) : undefined,
    })

    if (response.status === 401) {
      try {
        const newTokens = await this.tryRefresh()
        const retryResponse = await fetch(url, {
          method,
          headers: this.buildHeaders(newTokens),
          body: body ? JSON.stringify(body) : undefined,
        })

        if (retryResponse.status === 401) {
          this.onUnauthorized()
          throw new Error("Unauthorized after token refresh")
        }

        if (!retryResponse.ok) {
          throw new Error(`API error: ${retryResponse.status} ${retryResponse.statusText}`)
        }

        return retryResponse.json() as Promise<T>
      } catch {
        this.onUnauthorized()
        throw new Error("Unauthorized")
      }
    }

    if (!response.ok) {
      throw new Error(`API error: ${response.status} ${response.statusText}`)
    }

    if (response.status === 204) {
      return undefined as T
    }

    return response.json() as Promise<T>
  }

  get<T>(path: string): Promise<T> {
    return this.request<T>("GET", path)
  }

  post<T>(path: string, body?: unknown): Promise<T> {
    return this.request<T>("POST", path, body)
  }

  put<T>(path: string, body?: unknown): Promise<T> {
    return this.request<T>("PUT", path, body)
  }

  patch<T>(path: string, body?: unknown): Promise<T> {
    return this.request<T>("PATCH", path, body)
  }

  delete<T>(path: string): Promise<T> {
    return this.request<T>("DELETE", path)
  }
}

export const apiClient = new ApiClient({
  baseUrl: "",
  getTokens: () => getStoredTokens(),
  onTokenRefresh: (tokens) => storeTokens(tokens),
  onUnauthorized: () => {
    clearTokens()
    window.location.href = "/admin/login"
  },
})
