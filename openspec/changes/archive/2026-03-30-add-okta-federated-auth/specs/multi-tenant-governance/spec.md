## ADDED Requirements

### Requirement: Federated Identity Tenant Context Mapping
The system SHALL derive tenant context for authenticated interactive users from trusted Okta-backed identity claims and configured mapping rules.

#### Scenario: Authenticated user receives tenant context
- **WHEN** an authenticated interactive request reaches Aeterna with the required trusted identity fields
- **THEN** the system SHALL resolve the user's tenant context using configured claim and mapping rules
- **AND** the resolved tenant context SHALL be made available to downstream authorization checks

#### Scenario: Tenant mapping cannot be resolved
- **WHEN** the system cannot derive a valid tenant context from the authenticated user's trusted identity fields
- **THEN** the request SHALL be rejected as unauthorized
- **AND** the system SHALL NOT assign a default tenant

### Requirement: Okta Group Authorization Mapping
The system SHALL map trusted Okta group claims into Aeterna roles and policy inputs used for authorization.

#### Scenario: Group claims map to roles
- **WHEN** an authenticated user presents trusted Okta group claims
- **THEN** the system SHALL translate those groups into configured Aeterna roles or policy attributes
- **AND** authorization decisions SHALL use the translated roles or attributes

#### Scenario: Missing required group mapping fails closed
- **WHEN** an operation requires a role or policy attribute that cannot be derived from the authenticated user's trusted group claims
- **THEN** the system SHALL deny the operation
- **AND** the system SHALL record the authorization failure for audit or troubleshooting
