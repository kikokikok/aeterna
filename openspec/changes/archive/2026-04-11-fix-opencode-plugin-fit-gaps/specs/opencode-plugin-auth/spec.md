## MODIFIED Requirements

### Requirement: GitHub-OAuth-App Device-Code Plugin Authentication
The system SHALL provide an interactive authentication flow for the OpenCode plugin and CLI clients that uses a GitHub OAuth App device-code sign-in to obtain Aeterna-issued credentials for API access.

The plugin and CLI SHALL use the authenticated flow for interactive user access and SHALL NOT require users to manually provision a static `AETERNA_TOKEN` or GitHub PAT for normal sign-in.

#### Scenario: User signs in from OpenCode plugin
- **WHEN** the OpenCode plugin starts without a valid Aeterna plugin session
- **THEN** the plugin SHALL initiate the supported GitHub OAuth App device-code authentication flow
- **AND** the flow SHALL complete with Aeterna-issued credentials bound to the authenticated user identity
- **AND** the plugin SHALL present the verification URL and user code through the supported OpenCode user-visible interaction path for plugin sign-in

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

#### Scenario: Session reuse and refresh do not require duplicate startup state
- **WHEN** the plugin refreshes or reuses an existing authenticated session during startup
- **THEN** the plugin SHALL preserve a single coherent authenticated session context
- **AND** refresh/session reuse SHALL NOT require the user to restart OpenCode to recover normal authenticated tool usage
