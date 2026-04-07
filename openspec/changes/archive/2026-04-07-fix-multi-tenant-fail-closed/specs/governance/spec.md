## MODIFIED Requirements

### Requirement: Production Authentication Defaults
The governance and agent-facing runtime surfaces SHALL enforce fail-closed authentication behavior in production-capable deployments.

#### Scenario: Auth enabled without valid configuration
- **WHEN** authentication is enabled for an agent-facing service
- **AND** no valid API key, JWT verifier, or equivalent auth backend is configured
- **THEN** service startup or request processing SHALL fail closed
- **AND** the system SHALL NOT treat missing auth configuration as authenticated access

#### Scenario: JWT-backed request context
- **WHEN** a request presents a JWT-backed Authorization header
- **THEN** the system SHALL validate the token before deriving tenant or user context
- **AND** invalid or unimplemented JWT handling SHALL return an authentication error

#### Scenario: Production CORS behavior
- **WHEN** production deployment mode is configured
- **THEN** CORS origins, methods, and headers SHALL be restricted to configured allowlists
- **AND** wildcard permissive defaults SHALL NOT be used in production

#### Scenario: Production-capable mode does not use allow-all auth by accident
- **WHEN** a production-capable deployment starts without an explicitly supported permissive development mode
- **THEN** the runtime SHALL NOT default to allow-all authorization for tenant-scoped surfaces
- **AND** the operator SHALL receive an actionable configuration error instead
