## ADDED Requirements

### Requirement: Admin UI Token Kind
The server SHALL support an `admin-ui` JWT token kind distinct from the existing `plugin-access` kind. Admin UI sessions SHALL be issued, refreshed, and revoked through dedicated endpoints under `/api/v1/auth/admin-ui/*`, with an audience claim of `aeterna-admin-ui`.

#### Scenario: Admin UI bootstrap issues admin-ui tokens
- **WHEN** a valid GitHub OAuth code is presented to `POST /api/v1/auth/admin-ui/bootstrap`
- **THEN** the server SHALL issue a JWT access token with `aud: "aeterna-admin-ui"` and a refresh token stored with `token_kind = 'admin-ui'`
- **AND** the access token TTL SHALL be 30 minutes and the refresh token TTL SHALL be 14 days (configurable, shorter than plugin-access defaults)

#### Scenario: Admin UI refresh rotates only admin-ui tokens
- **WHEN** a valid admin-ui refresh token is presented to `POST /api/v1/auth/admin-ui/refresh`
- **THEN** the server SHALL issue a new admin-ui access/refresh pair, invalidate the presented refresh token (single-use rotation), and return the new pair

#### Scenario: Admin UI refresh rejects plugin tokens
- **WHEN** a `plugin-access` refresh token is presented to `POST /api/v1/auth/admin-ui/refresh`
- **THEN** the server SHALL return `401 Unauthorized` with error code `wrong_audience`

#### Scenario: Admin UI revoke is scoped
- **WHEN** an authenticated user calls `POST /api/v1/auth/admin-ui/revoke`
- **THEN** the server SHALL delete every `oauth_refresh_tokens` row for that user with `token_kind = 'admin-ui'`
- **AND** the server SHALL NOT touch rows with `token_kind = 'plugin-access'`
- **AND** subsequent admin UI calls from the user's browser SHALL result in `401` until re-bootstrap

### Requirement: Audience Claim Enforcement
The server SHALL validate the JWT `aud` claim against the endpoint's expected audience. Endpoints under `/api/v1/auth/admin-ui/*` SHALL require `aud: aeterna-admin-ui`; endpoints under `/api/v1/auth/plugin/*` SHALL require `aud: aeterna-plugin`; general API endpoints SHALL accept either audience during normal operation.

#### Scenario: Mismatched audience rejected
- **WHEN** a request to `/api/v1/auth/admin-ui/<action>` carries a token with `aud: aeterna-plugin`
- **AND** the legacy acceptance flag `ADMIN_UI_ACCEPT_LEGACY_PLUGIN_TOKENS` is `false`
- **THEN** the server SHALL return `401 Unauthorized` with error code `wrong_audience`

#### Scenario: Legacy acceptance flag permits plugin tokens on admin-ui endpoints
- **WHEN** a request to `/api/v1/auth/admin-ui/<action>` carries a token with `aud: aeterna-plugin`
- **AND** `ADMIN_UI_ACCEPT_LEGACY_PLUGIN_TOKENS` is `true`
- **THEN** the server SHALL accept the token
- **AND** the server SHALL emit a structured log event `deprecated-token-kind` with fields `user_id`, `endpoint`, `token_aud`

#### Scenario: General endpoints accept either audience
- **WHEN** a request to a tenant-scoped API endpoint (e.g., `GET /api/v1/user`) carries a token with either `aud: aeterna-admin-ui` or `aud: aeterna-plugin`
- **THEN** the server SHALL accept the token and authorize the request normally
