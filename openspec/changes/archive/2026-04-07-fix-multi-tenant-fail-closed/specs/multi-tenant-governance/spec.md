## MODIFIED Requirements

### Requirement: Tenant Context Propagation
The system SHALL propagate tenant context through all operations using a TenantContext structure.

```rust
pub struct TenantContext {
    pub tenant_id: TenantId,
    pub user_id: UserId,
    pub roles: Vec<Role>,
    pub hierarchy_path: HierarchyPath,
}
```

#### Scenario: Context injection
- **WHEN** a request arrives at any API endpoint
- **THEN** the system SHALL extract and validate TenantContext from authenticated identity or trusted boundary data
- **AND** the context SHALL be available to all downstream operations

#### Scenario: Context validation failure
- **WHEN** a request lacks valid TenantContext
- **THEN** the system SHALL reject the request with 401 Unauthorized
- **AND** the system SHALL NOT process any data operations

#### Scenario: Caller-supplied tenant payload is not authoritative by itself
- **WHEN** a request includes tenant context in headers, JSON payloads, or MCP parameters
- **THEN** the runtime SHALL validate that tenant context against the authenticated or trusted identity for the request
- **AND** the request SHALL be rejected if the supplied tenant context cannot be verified

### Requirement: Tenant Context Safety (MT-H3)
The system SHALL enforce mandatory tenant context propagation.

#### Scenario: Middleware Enforcement
- **WHEN** requests are processed
- **THEN** middleware MUST require valid TenantContext
- **AND** requests without context MUST be rejected immediately

#### Scenario: Fail-Closed Policy
- **WHEN** TenantContext extraction fails
- **THEN** system MUST fail closed (reject request)
- **AND** MUST NOT fall back to default tenant
- **AND** MUST NOT assign a synthetic system user as a replacement caller identity

#### Scenario: Context Audit Trail
- **WHEN** operations are performed
- **THEN** TenantContext MUST be logged with each operation
- **AND** audit logs MUST enable forensic reconstruction

### Requirement: Tenant Data Isolation Security (MT-C1)
The system SHALL implement defense-in-depth for tenant data isolation.

#### Scenario: Query Parameterization
- **WHEN** database queries are executed
- **THEN** all queries MUST use parameterized statements
- **AND** tenant_id MUST be included in all WHERE clauses

#### Scenario: Row-Level Security
- **WHEN** PostgreSQL tables contain multi-tenant data
- **THEN** row-level security (RLS) policies MUST be enabled
- **AND** RLS MUST enforce tenant isolation at the database level
- **AND** runtime hot paths MUST activate the database tenant context needed for those RLS policies

#### Scenario: Penetration Testing
- **WHEN** new tenant isolation features are deployed
- **THEN** penetration testing MUST verify cross-tenant isolation
- **AND** test results MUST be documented
