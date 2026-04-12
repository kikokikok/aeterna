# Aeterna Admin UI

Web-based administration interface for Aeterna. Provides tenant management, memory browsing, knowledge governance, user administration, and system health monitoring.

---

## Tech Stack

| Layer | Technology |
|---|---|
| Framework | React 19 |
| Build | Vite 8 |
| Language | TypeScript 6 |
| Styling | Tailwind CSS 4 |
| Data fetching | TanStack Query 5 |
| Routing | React Router 7 |
| Icons | Lucide React |
| UI utilities | clsx, tailwind-merge, class-variance-authority |

---

## Development

### Prerequisites

- Node.js (LTS)
- The Aeterna server running at `http://localhost:8080`

### Getting Started

```bash
npm install
npm run dev
```

The dev server starts at `http://localhost:5173`. API requests to `/api/*` are proxied to `http://localhost:8080` (configured in `vite.config.ts`).

### Build

```bash
npm run build
```

Output is written to `dist/`. The Aeterna server serves this directory at `/admin/*` when it exists (or when `AETERNA_ADMIN_UI_PATH` points to it).

### Lint

```bash
npm run lint
```

---

## Directory Structure

```
src/
  api/              API client and shared type definitions
    client.ts       Fetch-based API client with auth headers
    types.ts        Shared TypeScript types for API responses
  auth/             Authentication system
    AuthContext.tsx  React context: user, tokens, roles, tenant
    LoginPage.tsx   GitHub OAuth login page
    ProtectedRoute.tsx  Route guard (redirects unauthenticated users)
    RequireRole.tsx     Role-based access guard
    token-manager.ts    JWT storage, refresh, and expiry helpers
  components/       Shared UI components
  hooks/            Custom React hooks
  layouts/
    AdminLayout.tsx Main application layout with sidebar navigation
  lib/              Utility functions
  pages/            Page components organized by section
    admin/          System health and admin tools
    dashboard/      Main dashboard
    governance/     Governance workflows
    knowledge/      Knowledge management
    memory/         Memory browser
    organizations/  Organization management
    policies/       Policy management
    settings/       Application settings
    tenants/        Tenant management
    users/          User management
  main.tsx          Application entry point
  index.css         Global styles (Tailwind imports)
```

---

## Authentication Flow

1. User navigates to `/admin/` and is redirected to `LoginPage` if not authenticated.
2. `LoginPage` initiates GitHub OAuth via the Aeterna server (`POST /api/v1/auth/plugin/bootstrap`).
3. After GitHub authorization, the server issues JWT tokens (access + refresh).
4. `token-manager.ts` stores tokens in memory and handles automatic refresh.
5. `AuthContext` provides authentication state to all components:
   - `user` -- User profile (from admin session endpoint)
   - `isAuthenticated`, `isPlatformAdmin`, `isTenantAdmin`
   - `activeTenantId` -- Currently selected tenant
   - `tenants` -- Available tenant list
   - `roles` -- Role assignments
6. `ProtectedRoute` wraps routes that require authentication.
7. `RequireRole` wraps routes that require specific roles.

---

## API Client

The `apiClient` (`src/api/client.ts`) is a thin wrapper around `fetch` that:
- Automatically includes the JWT `Authorization` header
- Sets `X-Tenant-ID` from the active tenant context
- Handles token refresh on 401 responses
- Returns typed responses

Usage:
```typescript
import { apiClient } from '@/api/client';

const response = await apiClient.get('/api/v1/tenants');
const tenants = await response.json();
```

---

## Adding a New Page

1. Create a directory under `src/pages/<section>/` (e.g., `src/pages/backups/`).
2. Create the page component:
```tsx
import { useQuery } from '@tanstack/react-query';
import { apiClient } from '@/api/client';

export default function BackupsPage() {
  const { data, isLoading, error } = useQuery({
    queryKey: ['backups'],
    queryFn: () => apiClient.get('/api/v1/backup/exports').then(r => r.json()),
  });

  if (isLoading) return <div>Loading...</div>;
  if (error) return <div>Error loading backups</div>;

  return (
    <div>
      <h1 className="text-2xl font-bold">Backups</h1>
      {/* Render data */}
    </div>
  );
}
```
3. Add the route to the router configuration.
4. Add a navigation link in `AdminLayout.tsx` if the page should appear in the sidebar.

---

## Serving in Production

The Aeterna server (`cli/src/server/router.rs`) serves the admin UI as static files:

- Default path: `./admin-ui/dist` (relative to the server working directory)
- Override: Set `AETERNA_ADMIN_UI_PATH` environment variable
- The server uses `ServeDir` with `index.html` fallback for SPA client-side routing
- Base path: `/admin/` (configured in `vite.config.ts` as `base: '/admin/'`)
