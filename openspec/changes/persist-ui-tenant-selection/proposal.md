## Why

The admin UI's `TenantSelector` currently stores the active tenant in browser `sessionStorage` and sends it as `X-Target-Tenant-ID` on every API call. There is no link to the new server-side `users.default_tenant_id` preference (introduced by `refactor-platform-admin-impersonation`). A user who switches devices, refreshes after closing the tab, or opens a new browser starts from "no tenant selected" every time. Worse, when the server emits `400 select_tenant` on tenant-scoped endpoints, the UI currently surfaces a generic "Something went wrong" banner instead of routing the user to a picker with the payload already in hand.

This change makes the UI a first-class consumer of the server's preference model: the TenantSelector persists to the server by default, bootstraps from the session payload on load, and surfaces `400 select_tenant` as an actionable banner.

## What Changes

- On UI load, `AuthContext` reads `defaultTenantId`/`defaultTenantSlug` from `GET /api/v1/auth/session` and uses it to seed the active tenant when `sessionStorage` has no override.
- `TenantSelector` change writes: when the user picks a tenant, the UI calls `PUT /api/v1/user/me/default-tenant { slug }` in addition to updating local state. A toggle in the selector (`[Remember across devices]`, checked by default) controls whether the server write happens.
- Add a `SelectTenantBanner` component: whenever any API call returns `400 select_tenant`, the global error interceptor surfaces the banner at the top of the app with the `availableTenants` payload, a picker dropdown, and a `hint` tooltip. Selecting a tenant sets the active tenant, optionally persists via `PUT /user/me/default-tenant`, and retries the pending request.
- Extend the top-bar to show the current tenant's source (`local session`, `server default`, `admin-impersonation`) as a small muted label next to the selector, matching the CLI `auth status` vocabulary.
- PlatformAdmin-only UX: TenantSelector exposes a "Clear active tenant" option that removes the `X-Target-Tenant-ID` header for subsequent calls (enabling platform-scoped pages and cross-tenant listings with `?tenant=*`).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `admin-dashboard`: TenantSelector gains server-side persistence and bootstrap; new `SelectTenantBanner` handles `400 select_tenant`; tenant source label added to the shell; PlatformAdmin can explicitly clear the active tenant.

## Impact

- **Affected code**: `admin-ui/src/auth/AuthContext.tsx` (bootstrap from session payload, new `defaultTenantId` state), `admin-ui/src/components/TenantSelector.tsx` (server write, remember-across-devices toggle, clear option), `admin-ui/src/api/client.ts` (global 400 interceptor), new `admin-ui/src/components/SelectTenantBanner.tsx`, `admin-ui/src/hooks/useTenant.ts` (include source label).
- **Affected APIs consumed**: `GET|PUT|DELETE /api/v1/user/me/default-tenant` and extended `/api/v1/auth/session` payload (from `refactor-platform-admin-impersonation`).
- **Dependencies**: requires `refactor-platform-admin-impersonation` merged (server endpoints and error shape) and `add-admin-web-ui` archived (introduces the `admin-dashboard` capability).
- **UX**: no visible regression for users with a single tenant — selector remains hidden, auto-select behavior on server covers them.
