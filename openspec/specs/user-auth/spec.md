# user-auth Specification

## Purpose
TBD - created by archiving change add-okta-federated-auth. Update Purpose after archive.
## Requirements
### Requirement: Okta-Backed Product Authentication
The system SHALL provide end-user product authentication using Okta as the identity authority for interactive access.

Google or GitHub identities SHALL only be accepted when they are federated upstream into Okta and presented to Aeterna as Okta-issued identity.

#### Scenario: User authenticates with Okta-managed identity
- **WHEN** a user accesses a protected Aeterna product endpoint without an active session
- **THEN** the supported authentication layer SHALL redirect the user to the configured Okta login flow
- **AND** successful authentication SHALL return the user to Aeterna with trusted identity derived from Okta-issued session state or claims

#### Scenario: Upstream federated identity still resolves through Okta
- **WHEN** a corporate user authenticates with Google or GitHub through an upstream federation configured in Okta
- **THEN** Aeterna SHALL treat the resulting identity as Okta-authoritative
- **AND** Aeterna SHALL NOT require a separate Google-specific or GitHub-specific login integration

### Requirement: Trusted Identity Contract
The system SHALL normalize authenticated user identity into a trusted contract that includes a stable subject identifier, email, issuer, and group membership for downstream Aeterna services.

#### Scenario: Trusted identity is forwarded to Aeterna
- **WHEN** an authenticated request reaches Aeterna through the supported authentication boundary
- **THEN** the request SHALL include the normalized trusted identity fields required by Aeterna
- **AND** Aeterna SHALL use those fields as the source of truth for interactive user identity

#### Scenario: Missing trusted identity fails closed
- **WHEN** an interactive request reaches Aeterna without the required trusted identity fields
- **THEN** the request SHALL be rejected as unauthorized
- **AND** the system SHALL NOT fall back to anonymous or default-user access

#### Scenario: Tenant mapping cannot be derived
- **WHEN** an authenticated interactive flow cannot map the user to a valid tenant context
- **THEN** the request or session bootstrap SHALL be rejected as unauthorized
- **AND** the system SHALL NOT assign a default tenant claim to complete the flow

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

### Requirement: GitHub Identity as Primary Authentication Source
The system SHALL use GitHub identities authenticated via the device-code flow as the primary authentication source for interactive users, deriving tenant context, user identity, and initial role assignments from GitHub identity attributes.

#### Scenario: GitHub user maps to Aeterna tenant
- **WHEN** a user completes GitHub device-code authentication
- **THEN** the system SHALL look up the user's tenant mapping from their GitHub login, organization memberships, or configured mapping rules
- **AND** the system SHALL fail closed if no tenant mapping can be resolved

#### Scenario: New GitHub user without mapping is rejected
- **WHEN** a GitHub user authenticates but has no configured tenant mapping in Aeterna
- **THEN** the system SHALL reject the authentication with a clear error message
- **AND** the system SHALL NOT create a default tenant or assign a default identity

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

