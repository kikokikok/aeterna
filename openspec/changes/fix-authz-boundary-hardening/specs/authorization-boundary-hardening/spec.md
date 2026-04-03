## ADDED Requirements

### Requirement: Validated Tenant Identity Boundary
The system SHALL derive tenant-scoped request identity only from validated authentication context or explicitly trusted server-side boundaries.

#### Scenario: Caller-supplied headers alone are rejected
- **WHEN** a tenant-scoped API request supplies tenant or user identifiers only through caller-controlled HTTP headers
- **THEN** the system SHALL reject the request unless those headers were produced by an explicitly trusted boundary for the deployment
- **AND** the system SHALL NOT treat raw caller-supplied headers as authoritative identity in production-capable modes

#### Scenario: Validated identity produces tenant context
- **WHEN** a request presents a validated authenticated identity
- **THEN** the system SHALL derive the request's tenant context from that validated identity or its verified mappings
- **AND** downstream handlers SHALL consume the derived context rather than reparsing caller-controlled identity fields

### Requirement: Runtime Authorization Must Use Real Policy Enforcement
The system SHALL use the configured authorization backend or fail closed for tenant-scoped operations.

#### Scenario: Authorization backend unavailable
- **WHEN** a tenant-scoped operation requires authorization and no valid authorization backend or local policy fallback is available
- **THEN** the system SHALL fail closed for that operation
- **AND** it SHALL NOT silently substitute allow-all authorization

#### Scenario: Role mutation path unsupported
- **WHEN** a role lookup, assignment, or revocation path is not implemented for the active authorization backend
- **THEN** the system SHALL return an explicit unsupported error
- **AND** it SHALL NOT return an empty role list or pretend the mutation succeeded
