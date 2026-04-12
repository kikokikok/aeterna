## MODIFIED Requirements

### Requirement: HTTP Router Composition
The server SHALL compose a unified Axum HTTP router by merging sub-routers from each service crate, with a global authentication tower layer applied to all protected route groups.

#### Scenario: Admin UI route registration
- **WHEN** the server builds the HTTP router
- **AND** the configured admin UI dist directory exists at the path specified by `AETERNA_ADMIN_UI_PATH`
- **THEN** the server SHALL register an `/admin` route group serving static files from the admin UI dist directory via `tower_http::services::ServeDir`
- **AND** the `/admin` route group SHALL be placed outside the global authentication layer
- **AND** unmatched `/admin/*` paths SHALL fall back to serving `index.html` for SPA client-side routing

#### Scenario: Admin session endpoint registration
- **WHEN** the server builds the HTTP router
- **THEN** the server SHALL register `POST /api/v1/auth/admin/session` within the protected route group
- **AND** the endpoint SHALL require a valid bearer token and return user profile, roles, and tenant memberships
