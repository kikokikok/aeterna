## MODIFIED Requirements

### Requirement: Admin UI SPA Fallback Status Code
When the server is configured with an admin UI dist directory, requests under the `/admin/*` path prefix that do not match a static asset SHALL return the `index.html` document with HTTP status `200 OK`. This behavior preserves client-side routing while avoiding monitoring false-alarms caused by `404 Not Found` responses.

#### Scenario: SPA route returns 200 with index.html
- **WHEN** an admin UI dist directory is configured
- **AND** a `GET` request is made to `/admin/<any-client-side-route>` that does not match a static asset
- **THEN** the server SHALL respond with HTTP status `200 OK`
- **AND** the response `Content-Type` SHALL be `text/html; charset=utf-8`
- **AND** the response body SHALL be the contents of the admin UI `index.html`

#### Scenario: Static asset takes precedence
- **WHEN** a `GET` request is made to `/admin/<path>` that matches an existing file in the dist directory (e.g., `/admin/main.js`, `/admin/favicon.ico`)
- **THEN** the server SHALL serve that file with its actual content and `Content-Type`
- **AND** the SPA fallback SHALL NOT be triggered

#### Scenario: Non-admin paths preserve 404
- **WHEN** a `GET` request is made to a path outside `/admin/*` that does not match any route
- **THEN** the server SHALL return the default `404 Not Found` response (unchanged behavior)

#### Scenario: Admin UI not configured
- **WHEN** `AETERNA_ADMIN_UI_PATH` is not set, or points to a nonexistent directory
- **THEN** the `/admin/*` route group SHALL NOT be registered
- **AND** requests to `/admin/*` SHALL fall through to the default `404 Not Found` response (no SPA fallback)
