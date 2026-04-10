## MODIFIED Requirements

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
