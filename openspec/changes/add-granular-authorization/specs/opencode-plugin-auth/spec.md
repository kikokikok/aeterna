## MODIFIED Requirements

### Requirement: Authenticated Plugin Request Identity
The system SHALL ensure that plugin-originated authenticated requests carry validated user identity into Aeterna server request handling, with tenant context derived from the authenticated GitHub identity rather than hardcoded defaults.

#### Scenario: Authenticated sync request resolves real user identity
- **WHEN** the plugin sends a sync or API request using an Aeterna-issued plugin bearer token
- **THEN** the server SHALL validate the token before serving the request
- **AND** the server SHALL derive tenant and user context from validated claims rather than default system identity

#### Scenario: Invalid plugin token is rejected
- **WHEN** the plugin sends a bearer token that is missing, invalid, expired, or not issued for plugin API use
- **THEN** the server SHALL reject the request as unauthorized
- **AND** the server SHALL NOT fall back to default tenant or system-user context

#### Scenario: Tenant claim derived from GitHub identity
- **WHEN** a plugin bearer token is validated and the tenant claim needs to be resolved
- **THEN** the server SHALL derive the tenant from the authenticated GitHub user's configured tenant mapping
- **AND** the server SHALL NOT use a hardcoded default tenant value
- **AND** the server SHALL fail closed if no tenant mapping exists for the authenticated user
