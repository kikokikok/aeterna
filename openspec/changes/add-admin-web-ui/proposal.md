## Why

Aeterna exposes 70+ REST API endpoints across 18 modules with full multi-tenant RBAC, governance workflows, knowledge promotion, memory management, and organizational hierarchy administration. All of this functionality is accessible only through the CLI (`aeterna` binary) or direct API calls. There is no graphical interface for administrators to:

1. **Operational Visibility**: Monitor system health, pending governance requests, audit trails, drift detection results, and tenant status at a glance without composing CLI commands or API calls.
2. **Multi-tenant Administration**: Manage tenant lifecycle, organizational hierarchies, role assignments, knowledge repositories, and configuration across tenants through a visual interface — especially important for PlatformAdmins who must context-switch between tenants.
3. **Knowledge and Memory Curation**: Search, browse, promote, and curate knowledge items and memories with visual feedback — semantic search results, layer hierarchy visualization, promotion workflow state, and relation graphs are difficult to consume in CLI output.
4. **Governance Workflow Efficiency**: Review, approve, and reject governance requests with context (diff previews, audit history, approval chain visualization) that is impractical in a terminal interface.
5. **Policy and Drift Management**: Visualize policy rules, simulate policy evaluation, review drift detection results, and apply fixes through an interactive interface rather than CLI flags and JSON output.
6. **Onboarding and Adoption**: New administrators face a steep learning curve with CLI-only administration. A visual dashboard lowers the barrier to entry for tenant admins and platform operators.

## What Changes

- Add a new `admin-ui/` directory at the project root containing a React 18+ single-page application built with Vite, TypeScript, Tailwind CSS, and shadcn/ui components.
- The admin UI reuses the existing plugin_auth authentication flow (GitHub OAuth → JWT bootstrap → access/refresh token rotation) with a browser-adapted login experience. PlatformAdmin role detection uses the existing `__root__` sentinel tenant mechanism — no separate "global admin login" is needed.
- The UI provides 10 major functional areas: Dashboard (system health, quick stats), Tenant Management, Organizational Hierarchy, User & Role Management, Knowledge Management, Memory Management, Governance & Approvals, Policy Management, Admin Operations (export/import, sync, drift), and System Settings (LLM/embedding providers, storage status).
- Add server-side static asset serving via tower-http `ServeDir` to serve the built admin UI at `/admin/*` with SPA fallback for client-side routing.
- Add an optional convenience endpoint `POST /api/v1/auth/admin/session` that returns user profile, roles, and tenant memberships in a single call for efficient UI bootstrap.
- Adjust CORS configuration for development mode to allow the Vite dev server origin.

## Capabilities

### New Capabilities
- `admin-dashboard`: Browser-based administration interface for Aeterna platform and tenant management, covering system health monitoring, tenant lifecycle, organizational hierarchy, user/role management, knowledge curation and promotion, memory management and feedback, governance approval workflows, policy administration and drift detection, admin operations (export/import/sync), and system configuration (LLM/embedding providers, storage backends).

### Modified Capabilities
- `server-runtime`: Add static asset serving route group (`/admin/*`) to the HTTP router composition, excluded from API authentication layer. Add optional admin session convenience endpoint.
- `runtime-operations`: Add admin UI build integration into the deployment pipeline (Vite build → static assets → container image or Helm chart mount).

## Impact

- Affected code: `cli/src/server/router.rs` (add `/admin/*` static serving route), `Cargo.toml` workspace (add `tower-http` `fs` feature), `cli/src/server/plugin_auth.rs` (add admin session endpoint), new `admin-ui/` directory tree (React application).
- Affected APIs: New `POST /api/v1/auth/admin/session` convenience endpoint. All existing REST endpoints are consumed by the UI but not modified.
- Affected systems: Container image build (include `admin-ui/dist/` static assets), Helm chart (optional volume mount for admin UI assets), CI pipeline (add Node.js build step for admin UI).
