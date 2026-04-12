## ADDED Requirements

### Requirement: Admin UI Static Asset Serving
The server SHALL optionally serve static web assets for the admin dashboard at the `/admin` path prefix, with SPA fallback routing.

#### Scenario: Admin UI assets served from configured path
- **WHEN** the `AETERNA_ADMIN_UI_PATH` environment variable is set to a valid directory containing built admin UI assets
- **THEN** the server SHALL serve static files from that directory at the `/admin/*` path prefix
- **AND** requests for paths under `/admin/*` that do not match a static file SHALL return the `index.html` file for client-side routing

#### Scenario: Admin UI path not configured or missing
- **WHEN** `AETERNA_ADMIN_UI_PATH` is not set or points to a nonexistent directory
- **THEN** the server SHALL start successfully without registering the `/admin/*` route group
- **AND** the server SHALL log an info-level message indicating the admin UI is not available
- **AND** requests to `/admin/*` SHALL be handled by the default 404 fallback

#### Scenario: Admin UI assets do not require API authentication
- **WHEN** a request arrives at `/admin/*` for a static asset (HTML, JS, CSS, images)
- **THEN** the server SHALL serve the asset without requiring a bearer token
- **AND** API authentication SHALL continue to be enforced on `/api/v1/*` endpoints that the admin UI calls

### Requirement: Admin Session Convenience Endpoint
The server SHALL provide an optional convenience endpoint for the admin dashboard to bootstrap the UI session with a single API call.

#### Scenario: Admin session endpoint returns user context
- **WHEN** an authenticated request calls `POST /api/v1/auth/admin/session`
- **THEN** the server SHALL return the authenticated user's profile (GitHub login, email, user ID), resolved roles across all tenants, and tenant memberships
- **AND** the response SHALL include the user's PlatformAdmin status derived from `__root__` sentinel tenant role grants

#### Scenario: Admin session endpoint rejects unauthenticated requests
- **WHEN** a request calls `POST /api/v1/auth/admin/session` without a valid bearer token
- **THEN** the server SHALL return HTTP 401 Unauthorized

### Requirement: Admin UI Build Integration
The deployment pipeline SHALL support building and serving the admin UI alongside the Aeterna server.

#### Scenario: Admin UI build step in CI
- **WHEN** the CI pipeline runs
- **THEN** the pipeline SHALL build the admin UI static assets via `npm ci && npm run build` in the `admin-ui/` directory
- **AND** the built assets SHALL be included in the container image or deployment artifact

#### Scenario: Admin UI development mode
- **WHEN** a developer runs the Vite dev server alongside the Aeterna server
- **THEN** the Vite dev server SHALL proxy API requests to the Aeterna server
- **AND** the CORS layer on the Aeterna server SHALL accept requests from the Vite dev server origin
