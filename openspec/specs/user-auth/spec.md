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

### Requirement: Service Authentication Separation
The system SHALL preserve a separate supported authentication path for service-to-service and automation traffic.

#### Scenario: Machine client continues using service credentials
- **WHEN** a non-browser automation client accesses Aeterna using the supported service authentication method
- **THEN** the request SHALL continue to authenticate without requiring an Okta browser login
- **AND** interactive user SSO requirements SHALL NOT break existing service-to-service authentication flows

