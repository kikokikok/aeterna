## MODIFIED Requirements

### Requirement: Tenant Data Isolation Security (MT-C1)
The system SHALL implement defense-in-depth for tenant data isolation.

#### Scenario: Row-Level Security
- **WHEN** PostgreSQL tables contain tenant-scoped governance, administration, or operational data
- **THEN** row-level security policies MUST be enabled for those tables
- **AND** RLS MUST enforce tenant isolation at the database level
- **AND** runtime hot paths MUST activate the database tenant context needed for those RLS policies
