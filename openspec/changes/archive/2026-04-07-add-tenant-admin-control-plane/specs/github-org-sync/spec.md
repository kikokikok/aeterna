## ADDED Requirements

### Requirement: Sync Coexistence with Tenant Administration Control Plane
The system SHALL allow GitHub or IdP-managed hierarchy sync to coexist with manual tenant administration without overwriting admin-owned tenant configuration.

#### Scenario: Sync preserves tenant repository binding
- **WHEN** a GitHub organization sync updates a tenant, organization, or team hierarchy
- **THEN** the sync SHALL preserve the tenant's admin-managed knowledge repository binding
- **AND** the sync report SHALL identify that the binding was left unchanged

#### Scenario: Sync preserves verified tenant mappings unless explicitly sync-owned
- **WHEN** a GitHub organization sync updates tenant-linked identity data
- **THEN** admin-managed verified tenant mappings SHALL remain unchanged unless an explicit source-ownership rule marks them as sync-owned
- **AND** the sync report SHALL identify any skipped mapping mutations caused by source ownership
