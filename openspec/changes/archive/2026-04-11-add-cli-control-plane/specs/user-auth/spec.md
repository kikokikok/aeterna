## ADDED Requirements

### Requirement: CLI Interactive Authentication
The system SHALL support a dedicated interactive authentication flow for the `aeterna` CLI that is separate from browser ingress authentication and plugin-only UX.

#### Scenario: CLI login establishes authenticated CLI session
- **WHEN** a user runs the supported CLI login command for a remote Aeterna server
- **THEN** the CLI SHALL complete the supported interactive authentication flow for that target
- **AND** the resulting Aeterna credentials SHALL be stored for later CLI use
- **AND** the user SHALL be able to run authenticated backend-facing commands without manually exporting a bearer token for normal usage

#### Scenario: CLI auth status resolves connected identity
- **WHEN** a user runs the supported CLI auth status or identity command while connected to a remote server
- **THEN** the CLI SHALL show the authenticated identity resolved for the selected target
- **AND** the command SHALL distinguish between authenticated, unauthenticated, and expired-session states

#### Scenario: CLI auth failure is explicit
- **WHEN** a backend-facing CLI command requires authentication and no valid credentials are available
- **THEN** the CLI SHALL fail explicitly with an authentication error
- **AND** the CLI SHALL guide the user toward the supported login flow rather than suggesting only raw environment-variable setup

## MODIFIED Requirements

### Requirement: Service Authentication Separation
The system SHALL preserve separate supported authentication paths for browser-based user access, interactive OpenCode plugin access, interactive CLI usage, and service-to-service or automation traffic.

#### Scenario: Machine client continues using service credentials
- **WHEN** a non-browser automation client accesses Aeterna using the supported service authentication method
- **THEN** the request SHALL continue to authenticate without requiring an Okta browser login
- **AND** interactive user SSO requirements SHALL NOT break existing service-to-service authentication flows

#### Scenario: OpenCode plugin uses dedicated interactive client authentication
- **WHEN** an end user authenticates to Aeterna from the OpenCode plugin
- **THEN** the plugin SHALL use the supported plugin authentication flow rather than the browser-oriented Okta ingress flow
- **AND** the resulting authenticated requests SHALL resolve the end user's identity for downstream Aeterna services

#### Scenario: CLI uses dedicated interactive client authentication
- **WHEN** an end user authenticates to Aeterna from the `aeterna` CLI
- **THEN** the CLI SHALL use the supported CLI authentication flow rather than the browser-oriented Okta ingress flow
- **AND** the resulting authenticated requests SHALL resolve the end user's identity for downstream Aeterna services

#### Scenario: Browser authentication remains Okta-backed
- **WHEN** a user accesses protected Aeterna browser endpoints
- **THEN** browser authentication SHALL continue to use the supported Okta-backed interactive path
- **AND** plugin-specific or CLI-specific authentication changes SHALL NOT replace the browser login authority
