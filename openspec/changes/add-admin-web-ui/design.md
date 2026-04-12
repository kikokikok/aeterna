## Context

Aeterna is a Rust-based platform with an Axum 0.8 HTTP server exposing 70+ REST endpoints across tenant management, organizational hierarchy, user/role administration, knowledge repository, memory system, governance workflows, policy management, and administrative operations. Authentication uses GitHub OAuth bootstrapped through `POST /api/v1/auth/plugin/bootstrap`, which exchanges a GitHub access token for Aeterna-issued JWTs (access + refresh tokens with single-use rotation). The server uses tower-http middleware for CORS, compression, request-id, and tracing.

There is no frontend application. The only web presence is a Docusaurus documentation site at `website/`. Administration is performed entirely through the `aeterna` CLI or direct REST API calls. This change adds a browser-based admin dashboard that consumes the existing REST API surface without modifying it.

The admin UI must handle multi-tenancy: regular users see their tenant's data, while PlatformAdmin users can switch tenant context using the `X-Target-Tenant-ID` header pattern already supported by the server. PlatformAdmin roles are stored in the `user_roles` table under the `__root__` sentinel tenant ID. The role lookup SQL (`SELECT DISTINCT role FROM user_roles WHERE user_id = $1 AND (tenant_id = $2 OR tenant_id = '__root__')`) always returns instance-scoped roles regardless of which tenant the user authenticates into, so PlatformAdmin detection is automatic — no separate "global admin login" is needed.

## Goals / Non-Goals

**Goals:**
- Provide a production-quality admin dashboard covering all 10 functional areas (dashboard, tenants, orgs, users, knowledge, memory, governance, policies, admin ops, settings).
- Reuse the existing plugin_auth JWT flow adapted for browser sessions (localStorage token storage with refresh rotation).
- Serve the built SPA from the Axum server at `/admin/*` so no separate web server is needed in production.
- Support development with Vite dev server proxying API requests to the Axum backend.
- Implement PlatformAdmin cross-tenant context switching via tenant selector and `X-Target-Tenant-ID` header injection.
- Surface all CLI-equivalent admin functions in UI form: tenant management, org hierarchy, user/role management, knowledge search and promotion, memory management and feedback, governance approval workflows, policy management and drift detection, export/import, sync, and system health.

**Non-Goals:**
- Real-time collaboration or WebSocket-driven live updates (polling via TanStack Query refetchInterval is sufficient for V1).
- End-user-facing features (this is an admin/operator dashboard, not a user portal).
- Mobile-first design (desktop-first admin interface; responsive but not mobile-optimized).
- Replacing the CLI (the CLI remains the primary automation interface; the UI is complementary).
- Custom design system (use shadcn/ui with Tailwind for rapid development).
- Server-side rendering (SPA served as static assets is sufficient).
- Internationalization/localization (English only for V1).
- Automated TypeScript type generation from Rust types (manual for V1).

## Decisions

### Use React 18 + Vite + TypeScript + Tailwind CSS + shadcn/ui

**Decision:** The admin UI is built with React 18+, Vite as the build tool, TypeScript in strict mode, Tailwind CSS for styling, and shadcn/ui as the component library.

**Why:** React has the largest ecosystem for admin dashboards. Vite provides fast HMR during development and optimized production builds. TypeScript catches type errors at build time, which is critical for mapping to the Rust API's type contracts. Tailwind CSS enables rapid UI development. shadcn/ui provides accessible, composable components that are copy-pasted into the project (not a runtime dependency), giving full control over styling and behavior.

**Alternatives considered:**
- **Vue 3 + Vuetify**: Viable but smaller ecosystem for enterprise admin tooling.
- **SvelteKit**: Excellent DX but smaller component ecosystem for complex admin UIs (data tables, tree views, form builders).
- **Leptos (Rust WASM)**: Keeps everything in Rust but significantly slower to develop UI, smaller component ecosystem, and adds WASM compilation complexity.
- **Plain HTML + htmx**: Lacks the component abstraction needed for complex UIs like knowledge graph visualization and organizational tree management.

### Serve static assets from Axum using tower-http ServeDir with SPA fallback

**Decision:** The built `admin-ui/dist/` directory is served by Axum at the `/admin` path prefix using `tower_http::services::ServeDir` with a fallback to `index.html` for client-side routing support. The `/admin/*` route group is placed outside the API authentication layer.

**Why:** This eliminates the need for a separate web server (nginx, caddy) in production. tower-http's `ServeDir` is already a dependency (tower-http 0.6 is in the workspace — just needs `fs` feature flag). The SPA fallback ensures that deep links like `/admin/tenants/acme-corp` serve `index.html` and let React Router handle the route. Authentication happens at the API layer (JWT bearer tokens on `/api/v1/*` calls), not at the static asset layer.

**Alternatives considered:**
- **Separate nginx/caddy container**: Adds container complexity and requires separate CORS configuration.
- **Embed assets in the Rust binary with rust-embed**: Increases binary size significantly and requires full rebuild for any UI change.
- **CDN hosting**: Not viable for self-hosted/air-gapped enterprise deployments.

### Adapt existing plugin_auth flow for browser sessions

**Decision:** The admin UI uses the same `POST /api/v1/auth/plugin/bootstrap` and `POST /api/v1/auth/plugin/refresh` endpoints. The browser initiates GitHub OAuth via a popup/redirect, obtains a GitHub access token, and exchanges it through the bootstrap endpoint. Access and refresh tokens are stored in localStorage with automatic refresh before expiry.

**Why:** Reusing the existing auth flow avoids adding a separate OIDC/session-cookie auth system. The plugin_auth flow is already proven and handles JWT issuance, refresh rotation, and revocation. PlatformAdmin detection is automatic because the role lookup SQL always includes `__root__` grants — no special admin login endpoint is needed.

**Alternatives considered:**
- **HTTP-only cookies with server-side sessions**: More secure against XSS but requires server-side session state and CSRF protection, adding complexity to the stateless JWT architecture.
- **Separate OIDC flow (Okta/Auth0)**: The project has Okta specs but they target production user auth, not admin tooling. Adding a separate IdP for the admin UI is overkill for V1.

### Use TanStack Query for server state management

**Decision:** All API data fetching, caching, and mutation is managed through TanStack Query. Local UI state uses React useState/useContext.

**Why:** TanStack Query provides automatic caching, background refetching, optimistic updates, and request deduplication. It eliminates the need for a global state manager for server-derived data. Its devtools panel helps debug API state during development.

**Alternatives considered:**
- **Redux Toolkit + RTK Query**: More boilerplate for the same result.
- **SWR**: Lighter but lacks built-in mutation support and devtools.
- **Plain fetch + useState**: Too much manual caching and refetch logic for 70+ endpoints.

### Add optional POST /api/v1/auth/admin/session convenience endpoint

**Decision:** Add a single new endpoint that returns the authenticated user's profile (GitHub identity, email), resolved roles across all tenants (including `__root__` PlatformAdmin grants), and tenant memberships. The UI calls this once after login to render the shell.

**Why:** Without this, the UI would need 3+ serial API calls on startup (user profile, roles, tenant list). A single call reduces initial load latency and simplifies the auth bootstrap logic.

**Alternatives considered:**
- **Client-side assembly from multiple endpoints**: Slower and more complex error handling for the critical auth bootstrap path.
- **Embed all data in JWT claims**: JWT size would grow with every tenant membership and role assignment, eventually exceeding header size limits.

### Use React Router v6 with route-based code splitting

**Decision:** React Router v6 handles all client-side navigation. Each major page area is a lazy-loaded route chunk for optimal initial load time.

**Why:** Route-based code splitting keeps the initial bundle small (auth + shell layout only) and loads page-specific code on demand. The admin UI has 10+ page areas — loading all of them upfront would degrade the first-paint time.

**Alternatives considered:**
- **TanStack Router**: Type-safe routing but smaller community and less mature ecosystem.
- **File-based routing (Remix)**: Adds server-side framework complexity unnecessary when Axum already serves the API.

## Risks / Trade-offs

- **[Risk] localStorage token storage is vulnerable to XSS** — Mitigation: Content Security Policy headers served with the SPA, input sanitization in all form fields, no user-generated HTML rendering. V2 can migrate to HTTP-only cookies if needed.
- **[Risk] Admin UI adds Node.js build tooling to a Rust-only project** — Mitigation: Admin UI build is isolated in `admin-ui/` with its own `package.json`. CI builds the UI as a separate step. The Rust build does not depend on Node.js.
- **[Risk] API type drift between Rust types and TypeScript types** — Mitigation: TypeScript types are manually maintained initially. A future improvement can auto-generate types from Rust structs via schemars + openapi-generator.
- **[Risk] Large number of pages/features may result in incomplete V1** — Mitigation: Phased delivery. Phase 1-3 (auth, shell, dashboard, tenants) are immediately useful. Subsequent phases add functional areas incrementally.
- **[Risk] tower-http ServeDir adds the `fs` feature to the cli crate** — Mitigation: The `fs` feature is small and well-maintained. The feature flag is additive only.
- **[Trade-off] No real-time updates (WebSocket) in V1** — The admin dashboard uses polling via TanStack Query's refetchInterval. WebSocket push is a V2 enhancement.
- **[Trade-off] No automated TypeScript type generation** — Manual type definitions in V1. OpenAPI spec generation from the Axum router is a future improvement that would eliminate this maintenance burden.

## Migration Plan

1. Scaffold the `admin-ui/` project with Vite + React + TypeScript + Tailwind + shadcn/ui.
2. Implement auth flow (GitHub OAuth → JWT bootstrap → token management) and the admin shell layout (sidebar, header, tenant selector, role-based navigation).
3. Build the dashboard home page with system health and quick stats.
4. Incrementally add page areas in dependency order: tenants → org hierarchy → users → knowledge → memory → governance → policies → admin ops → settings.
5. Add the Axum static asset serving route (`/admin/*` with ServeDir + SPA fallback) and the admin session convenience endpoint.
6. Integrate the admin UI build into the CI pipeline and container image build.
7. Add end-to-end tests for critical flows (login, tenant CRUD, governance approval).

## Open Questions

- Should the admin UI be included in the default container image, or should it be an optional sidecar/volume mount for minimal deployments?
- Should we generate TypeScript types from Rust structs automatically (via schemars + openapi-generator), or maintain them manually for V1?
- Should the GitHub OAuth popup flow use a server-side callback endpoint, or handle the OAuth exchange entirely client-side with CORS?
- Should the admin UI have its own feature flag (`AETERNA_FEATURE_ADMIN_UI`) to disable static serving in production environments that do not need it?
- Should the knowledge relations graph use a dedicated visualization library (e.g., react-force-graph, cytoscape.js) or a simpler D3-based custom component?
