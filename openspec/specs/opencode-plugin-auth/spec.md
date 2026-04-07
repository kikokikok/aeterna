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

