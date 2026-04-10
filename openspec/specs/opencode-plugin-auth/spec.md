# opencode-plugin-auth Specification

## Purpose
TBD - created by archiving change add-opencode-github-app-auth. Update Purpose after archive.
## Requirements
### Requirement: GitHub-OAuth-App Device-Code Plugin Authentication
The system SHALL provide an interactive authentication flow for the OpenCode plugin and CLI clients that uses a GitHub OAuth App device-code sign-in to obtain Aeterna-issued credentials for API access.

The plugin and CLI SHALL use the authenticated flow for interactive user access and SHALL NOT require users to manually provision a static `AETERNA_TOKEN` or GitHub PAT for normal sign-in.

#### Scenario: User signs in from OpenCode plugin
- **WHEN** the OpenCode plugin starts without a valid Aeterna plugin session
- **THEN** the plugin SHALL initiate the supported GitHub OAuth App device-code authentication flow
- **AND** the flow SHALL complete with Aeterna-issued credentials bound to the authenticated user identity

#### Scenario: User signs in from CLI
- **WHEN** a user runs `aeterna auth login` without providing a `--github-token` flag
- **THEN** the CLI SHALL initiate the same GitHub OAuth App device-code authentication flow
- **AND** the flow SHALL complete with Aeterna-issued credentials exchanged through the same bootstrap endpoint

#### Scenario: Existing valid session is reused
- **WHEN** the OpenCode plugin or CLI starts with a valid previously issued Aeterna session
- **THEN** the client SHALL reuse the existing credentials
- **AND** the user SHALL NOT be prompted to sign in again until refresh or expiry requires it

### Requirement: Plugin Token Refresh
The system SHALL support refresh of Aeterna-issued plugin credentials without requiring the user to restart OpenCode or manually replace configuration values.

#### Scenario: Access token nears expiry
- **WHEN** the plugin detects that its current Aeterna-issued access token is expired or nearing expiry
- **THEN** the plugin SHALL refresh the session using the supported refresh mechanism
- **AND** subsequent API requests SHALL use the refreshed token automatically

#### Scenario: Refresh fails
- **WHEN** token refresh fails because the session is revoked, invalid, or otherwise not refreshable
- **THEN** the plugin SHALL fail the authenticated request explicitly
- **AND** the plugin SHALL require the user to sign in again before continuing authenticated operations

### Requirement: Authenticated Plugin Request Identity
The system SHALL ensure that plugin-originated authenticated requests carry validated user identity into Aeterna server request handling.

The plugin auth bootstrap endpoint SHALL require a configured default tenant ID (`AETERNA_DEFAULT_TENANT_ID` or top-level `defaultTenantId` in Helm values) to resolve the tenant for GitHub-authenticated users. When this value is absent, the endpoint SHALL fail closed rather than falling back to a default tenant.

When authentication is enabled, the server SHALL resolve user roles from the `user_roles` database table after JWT validation, rather than trusting client-asserted `X-User-Role` headers. The server SHALL map the JWT subject (GitHub login) to the internal user identity via `users.idp_subject`, then query the `user_roles` table to populate the request context with database-backed roles.

For instance-scoped roles (e.g., PlatformAdmin), the `user_roles` table SHALL use `tenant_id = '__root__'` as the explicit sentinel value for instance scope. The server SHALL query `user_roles` with both the JWT's resolved tenant_id and `tenant_id = '__root__'`, so that instance-scoped role grants are visible regardless of the caller's resolved tenant.

The `X-User-Role` header SHALL be ignored when authentication is enabled. It SHALL remain trusted only when authentication is disabled (development/service-to-service mode) for backward compatibility.

#### Scenario: Authenticated sync request resolves real user identity
- **WHEN** the plugin sends a sync or API request using an Aeterna-issued plugin bearer token
- **THEN** the server SHALL validate the token before serving the request
- **AND** the server SHALL derive tenant and user context from validated claims rather than default system identity

#### Scenario: Invalid plugin token is rejected
- **WHEN** the plugin sends a bearer token that is missing, invalid, expired, or not issued for plugin API use
- **THEN** the server SHALL reject the request as unauthorized
- **AND** the server SHALL NOT fall back to default tenant or system-user context

#### Scenario: Plugin auth tenant not configured
- **WHEN** the plugin auth bootstrap endpoint receives a valid GitHub authentication callback but no default tenant ID is configured
- **THEN** the server SHALL return an error indicating tenant configuration is required
- **AND** the server SHALL NOT fall back to a hardcoded or synthetic tenant value

#### Scenario: Authenticated user roles resolved from database
- **WHEN** an authenticated request arrives with a valid JWT bearer token
- **AND** authentication is enabled
- **THEN** the server SHALL look up the user's roles from the `user_roles` database table using the user's internal ID (resolved from `users.idp_subject` matching the JWT subject)
- **AND** the server SHALL populate the request context with the database-backed roles
- **AND** the server SHALL NOT use any client-asserted `X-User-Role` header value

#### Scenario: PlatformAdmin role is visible across tenants
- **WHEN** an authenticated user has a PlatformAdmin role granted with `tenant_id = '__root__'` (instance scope)
- **AND** the user's JWT resolves to a different tenant (e.g., `acme-corp`)
- **THEN** the server SHALL still recognize the PlatformAdmin role in the request context
- **AND** the user SHALL be able to perform PlatformAdmin operations such as tenant creation

#### Scenario: User without database roles has empty role set
- **WHEN** an authenticated user has no entries in the `user_roles` table
- **THEN** the server SHALL populate the request context with an empty role set
- **AND** the user SHALL be denied access to role-protected endpoints

