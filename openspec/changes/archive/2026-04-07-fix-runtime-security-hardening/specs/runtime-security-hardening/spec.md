## ADDED Requirements

### Requirement: Production Browser Boundary Hardening
The system SHALL restrict browser-origin access to configured allowlists in production-capable deployments.

#### Scenario: Production deployment uses origin allowlist
- **WHEN** the runtime is configured for a production-capable deployment
- **THEN** the HTTP server SHALL restrict allowed origins, methods, and headers to configured safe values
- **AND** it SHALL NOT apply a globally permissive CORS policy to all routes

#### Scenario: Local-only permissive mode is explicit
- **WHEN** a local development deployment uses permissive browser access
- **THEN** that mode SHALL require an explicit development configuration
- **AND** the runtime SHALL signal clearly that the deployment is not using production-safe browser boundaries

### Requirement: Truthful Dependency Readiness
The runtime SHALL report readiness and degradation based on the actual reachability of dependencies required by the configured mode.

#### Scenario: Required dependency unavailable
- **WHEN** a configured critical dependency such as the vector store or session backing store is unavailable
- **THEN** readiness SHALL report degraded or unavailable status
- **AND** the response SHALL identify the failing dependency category

#### Scenario: Optional dependency omitted for current mode
- **WHEN** a dependency is not required for the configured runtime mode
- **THEN** readiness SHALL not fail solely because that optional dependency is absent
- **AND** the response SHALL distinguish optional from required components

### Requirement: Verified and Reliable Webhook Processing
The system SHALL verify webhook authenticity before triggering tenant-affecting control-plane behavior and SHALL not silently drop failed control-plane mutations.

#### Scenario: Unverified sync-triggering webhook
- **WHEN** a webhook event that can affect tenant hierarchy or memberships fails required verification
- **THEN** the system SHALL reject the event before starting any sync or mutation workflow
- **AND** no tenant-affecting background work SHALL be scheduled

#### Scenario: Webhook-triggered mutation fails
- **WHEN** a verified webhook-triggered mutation cannot complete successfully
- **THEN** the system SHALL record the failure in a retryable or auditable way
- **AND** it SHALL NOT silently acknowledge the mutation as processed if no durable outcome exists

### Requirement: Backend-Specific Persistence Isolation
The system SHALL define and enforce tenant isolation explicitly for each persistence backend used in production-capable deployments.

#### Scenario: PostgreSQL tenant isolation uses enforced database policy
- **WHEN** PostgreSQL stores tenant-scoped data
- **THEN** the system SHALL enforce tenant isolation with tenant-aware schema constraints, tenant session context, and row-level security on all tenant-scoped tables
- **AND** cross-tenant reads or writes SHALL be rejected even if an application query path is malformed

#### Scenario: Qdrant tenant isolation is enforced by storage routing
- **WHEN** Qdrant stores tenant-scoped vectors or payloads
- **THEN** the storage layer SHALL route operations through tenant-scoped collections, mandatory tenant filters, or both
- **AND** it SHALL reject or prevent queries that could return vectors from another tenant

#### Scenario: Redis tenant isolation is enforced by storage namespaces
- **WHEN** Redis stores tenant-scoped working memory, session data, checkpoints, streams, or caches
- **THEN** the storage layer SHALL use tenant-scoped key namespaces and access wrappers for those keys
- **AND** callers SHALL NOT read or mutate another tenant's Redis data without an explicit authorized tenant context
