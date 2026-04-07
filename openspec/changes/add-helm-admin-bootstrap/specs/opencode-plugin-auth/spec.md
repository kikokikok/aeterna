## MODIFIED Requirements

### Requirement: Authenticated Plugin Request Identity
The system SHALL ensure that plugin-originated authenticated requests carry validated user identity into Aeterna server request handling.

The plugin auth bootstrap endpoint SHALL require a configured default tenant ID (`AETERNA_PLUGIN_AUTH_TENANT` or `pluginAuth.defaultTenantId` in Helm values) to resolve the tenant for GitHub-authenticated users. When this value is absent, the endpoint SHALL fail closed rather than falling back to a default tenant.

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
