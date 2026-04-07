## MODIFIED Requirements

### Requirement: Service Authentication Separation
The system SHALL preserve a separate supported authentication path for service-to-service and automation traffic, while using GitHub device-code login as the primary interactive authentication source.

#### Scenario: Machine client continues using service credentials
- **WHEN** a non-browser automation client accesses Aeterna using the supported service authentication method
- **THEN** the request SHALL continue to authenticate without requiring a GitHub device-code login
- **AND** interactive user authentication requirements SHALL NOT break existing service-to-service authentication flows

#### Scenario: Interactive users authenticate via GitHub device-code
- **WHEN** an interactive user accesses Aeterna without an active session
- **THEN** the system SHALL initiate the GitHub OAuth App device-code authentication flow
- **AND** the system SHALL derive tenant and user identity from the authenticated GitHub identity

## ADDED Requirements

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
