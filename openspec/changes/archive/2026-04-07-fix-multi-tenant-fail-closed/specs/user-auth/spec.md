## MODIFIED Requirements

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
