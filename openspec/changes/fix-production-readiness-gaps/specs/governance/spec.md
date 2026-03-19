## MODIFIED Requirements

### Requirement: High Availability Policy Engine
The governance engine (OPAL Server and Cedar components) SHALL operate in a highly available, multi-replica architecture without unsafe secret rotation or permissive production defaults.

#### Scenario: OPAL Pod Failure
- **WHEN** an OPAL server pod crashes
- **THEN** authorization decisions must continue unhindered via local caches and remaining replicas
- **AND** the system must utilize an HA Redis backend for PubSub state

#### Scenario: OPAL secret reuse during upgrade
- **WHEN** OPAL components are upgraded without an explicit credential change
- **THEN** master and client tokens SHALL remain stable across the upgrade
- **AND** connected agents and fetchers SHALL NOT be invalidated by chart-generated secret churn

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
