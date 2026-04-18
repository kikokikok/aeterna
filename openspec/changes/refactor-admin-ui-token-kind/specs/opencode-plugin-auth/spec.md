## MODIFIED Requirements

### Requirement: Plugin-Access Token Scope
The `plugin-access` token kind issued by `POST /api/v1/auth/plugin/bootstrap` SHALL be used exclusively for OpenCode plugin API calls. Admin UI sessions SHALL use the separate `admin-ui` token kind. During a one-release grace period, admin UI endpoints MAY accept `plugin-access` tokens when `ADMIN_UI_ACCEPT_LEGACY_PLUGIN_TOKENS=true`; after that period, such acceptance is removed.

#### Scenario: Plugin bootstrap still issues plugin-access tokens
- **WHEN** a valid bootstrap request is made to `POST /api/v1/auth/plugin/bootstrap`
- **THEN** the server SHALL issue a JWT with `aud: aeterna-plugin` and a refresh token stored with `token_kind = 'plugin-access'`
- **AND** TTLs SHALL remain at the existing plugin values (access 1 hour, refresh 30 days unless otherwise configured)

#### Scenario: Plugin refresh rejects admin-ui tokens
- **WHEN** an `admin-ui` refresh token is presented to `POST /api/v1/auth/plugin/refresh`
- **THEN** the server SHALL return `401 Unauthorized` with error code `wrong_audience`

#### Scenario: Plugin revoke does not affect admin-ui sessions
- **WHEN** a user calls `POST /api/v1/auth/plugin/revoke`
- **THEN** the server SHALL delete only `token_kind = 'plugin-access'` refresh tokens for that user
- **AND** any active admin-ui browser sessions SHALL remain valid

#### Scenario: Admin UI endpoints reject plugin tokens after grace period
- **WHEN** an admin UI endpoint receives a `plugin-access` token
- **AND** `ADMIN_UI_ACCEPT_LEGACY_PLUGIN_TOKENS` is `false`
- **THEN** the server SHALL return `401 wrong_audience`
