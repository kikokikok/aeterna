## MODIFIED Requirements

### Requirement: Persistent Sync State
The system SHALL maintain persistent state for tracking synchronization between memory and knowledge systems per tenant.

#### Scenario: Save state on successful sync with tenant context
- **WHEN** a sync operation completes successfully with TenantContext
- **THEN** system SHALL validate TenantContext authorization
- **AND** system SHALL save SyncState with lastSyncAt, scoped to tenant
- **AND** system SHALL save lastKnowledgeCommit hash with tenant metadata
- **AND** system SHALL save knowledgeHashes mapping with tenant isolation
- **AND** system SHALL save pointerMapping with tenant isolation

#### Scenario: Load state on startup with tenant context
- **WHEN** system starts and loads sync state for a specific tenant
- **THEN** system SHALL load existing SyncState for that tenant if available
- **AND** system SHALL initialize empty state for tenant if none exists
- **AND** system SHALL NOT load sync state from other tenants

### Requirement: Delta Detection
The system SHALL detect changes between knowledge repository and last sync state using hash-based comparison with tenant isolation.

#### Scenario: Detect new items within tenant
- **WHEN** knowledge manifest has new IDs not in stored hashes for the tenant
- **THEN** system SHALL add items to delta.added for that tenant
- **AND** system SHALL ignore items from other tenants

#### Scenario: Detect updated items within tenant
- **WHEN** knowledge manifest item ID exists but hash differs for the tenant
- **THEN** system SHALL add items to delta.updated for that tenant

#### Scenario: Detect deleted items within tenant
- **WHEN** stored hash ID not found in knowledge manifest for the tenant
- **THEN** system SHALL add ID to delta.deleted for that tenant

### Requirement: Pointer Memory Generation
The system SHALL generate pointer memories that summarize knowledge items for efficient storage and retrieval with tenant isolation.

#### Scenario: Create pointer content with tenant context
- **WHEN** creating a pointer for a knowledge item with TenantContext
- **THEN** system SHALL include knowledge title and summary in content
- **AND** system SHALL include type indicator ([ADR], [SPEC], etc.)
- **AND** system SHALL include knowledge item ID as reference
- **AND** system SHALL include tenant metadata in pointer memory
- **AND** system SHALL store pointer memory in tenant-isolated memory layer

#### Scenario: Create pointer without tenant context
- **WHEN** creating a pointer for a knowledge item without TenantContext
- **THEN** system SHALL return MISSING_TENANT_CONTEXT error

### Requirement: Multiple Sync Methods
The system SHALL provide multiple sync methods for different use cases with tenant isolation.

#### Scenario: Full sync execution with tenant context
- **WHEN** running full sync with TenantContext
- **THEN** system SHALL validate TenantContext authorization
- **AND** system SHALL create checkpoint before starting for that tenant
- **THEN** system SHALL process all additions, updates, and deletions within the tenant
- **THEN** system SHALL rollback on catastrophic failure for that tenant only

#### Scenario: Incremental sync execution with tenant context
- **WHEN** running incremental sync with TenantContext
- **THEN** system SHALL validate TenantContext authorization
- **AND** system SHALL only process changes since last sync for that tenant
- **AND** system SHALL ignore changes from other tenants

### Requirement: Conflict Detection
The system SHALL detect conflicts between memory and knowledge during sync with tenant-aware resolution.

#### Scenario: Detect policy conflict within tenant
- **WHEN** memory content contradicts knowledge policy within the same tenant
- **THEN** system SHALL flag conflict with tenant context
- **AND** system SHALL calculate drift score for tenant-specific policies

#### Scenario: Detect cross-tenant conflict (should not occur)
- **WHEN** memory content from Tenant A references knowledge from Tenant B
- **THEN** system SHALL treat as missing reference (since cross-tenant access prohibited)
- **AND** system SHALL NOT create conflict resolution entry

### Requirement: Automated Conflict Resolution
The system SHALL provide automated conflict resolution strategies with tenant-specific policy enforcement.

#### Scenario: Resolve conflict with tenant governance policies
- **WHEN** resolving a conflict with TenantContext
- **THEN** system SHALL apply tenant-specific conflict resolution policies
- **AND** system SHALL prioritize knowledge over memory when policy mandates
- **AND** system SHALL emit governance event for conflict resolution

#### Scenario: Resolve conflict without tenant context
- **WHEN** resolving a conflict without TenantContext
- **THEN** system SHALL return MISSING_TENANT_CONTEXT error

### Requirement: Sync Triggers
The system SHALL support multiple sync triggers with tenant-aware scheduling.

#### Scenario: Manual trigger with tenant context
- **WHEN** manually triggering sync with TenantContext
- **THEN** system SHALL validate TenantContext authorization
- **AND** system SHALL execute sync for that tenant only

#### Scenario: Scheduled trigger per tenant
- **WHEN** scheduled sync runs
- **THEN** system SHALL execute sync for each tenant independently
- **AND** system SHALL maintain separate schedules per tenant if configured

### Requirement: Atomic Checkpoints
The system SHALL create atomic checkpoints for sync operations with tenant isolation.

#### Scenario: Create checkpoint with tenant context
- **WHEN** creating a checkpoint with TenantContext
- **THEN** system SHALL store checkpoint data scoped to tenant
- **AND** system SHALL NOT include data from other tenants in checkpoint

#### Scenario: Rollback with tenant context
- **WHEN** rolling back with TenantContext
- **THEN** system SHALL restore state only for that tenant
- **AND** system SHALL NOT affect other tenants' sync state

### Requirement: Observability
The system SHALL provide observability metrics for sync operations with tenant segmentation.

#### Scenario: Track sync metrics per tenant
- **WHEN** sync operations occur
- **THEN** system SHALL emit metrics with tenant_id label
- **AND** system SHALL track sync duration, items processed, conflicts detected per tenant
- **AND** system SHALL NOT aggregate metrics across tenants without authorization

#### Scenario: Monitor cross-tenant sync attempts
- **WHEN** sync operation attempts to access cross-tenant data
- **THEN** system SHALL emit security alert metric
- **AND** system SHALL log the attempt with tenant context

### Requirement: Error Handling
The system SHALL handle sync errors with tenant-aware recovery strategies.

#### Scenario: Handle sync error with tenant context
- **WHEN** sync operation fails with TenantContext
- **THEN** system SHALL rollback changes for that tenant only
- **AND** system SHALL log error with tenant metadata
- **AND** system SHALL emit error metric with tenant_id label

#### Scenario: Handle cross-tenant access error
- **WHEN** sync operation attempts cross-tenant access
- **THEN** system SHALL return TENANT_ISOLATION_VIOLATION error
- **AND** system SHALL abort operation for that tenant

### Requirement: Performance Targets
The system SHALL meet performance targets for sync operations with tenant scaling considerations.

#### Scenario: Sync performance within tenant
- **WHEN** syncing large knowledge repositories within a tenant
- **THEN** system SHALL meet performance targets (latency, throughput) for that tenant
- **AND** system SHALL scale resources per tenant if configured

#### Scenario: Multi-tenant sync performance
- **WHEN** syncing multiple tenants concurrently
- **THEN** system SHALL maintain performance isolation between tenants
- **AND** system SHALL prevent tenant starvation via fair scheduling

## ADDED Requirements

### Requirement: Tenant Context Propagation
All sync operations SHALL require a TenantContext parameter for tenant isolation and authorization.

#### Scenario: Sync operation without tenant context
- **WHEN** any sync operation is attempted without TenantContext
- **THEN** system SHALL return MISSING_TENANT_CONTEXT error
- **AND** operation SHALL NOT proceed

#### Scenario: Tenant context validation for sync
- **WHEN** TenantContext contains invalid or expired credentials
- **THEN** system SHALL return INVALID_TENANT_CONTEXT error
- **AND** sync operation SHALL NOT proceed

### Requirement: Tenant Isolation Enforcement
The system SHALL enforce hard tenant isolation at all sync boundaries.

#### Scenario: Cross-tenant sync attempt
- **WHEN** sync operation attempts to access knowledge or memory from another tenant
- **THEN** system SHALL return TENANT_ISOLATION_VIOLATION error
- **AND** operation SHALL be aborted
- **AND** system SHALL log security event

#### Scenario: Tenant-specific pointer mapping
- **WHEN** creating pointer mappings between knowledge and memory
- **THEN** mappings SHALL be scoped to tenant
- **AND** pointer memories SHALL only reference knowledge items within the same tenant

### Requirement: Governance-Driven Sync Policies
Sync operations SHALL respect tenant governance policies for conflict resolution and drift management.

#### Scenario: Apply governance policies during conflict resolution
- **WHEN** resolving sync conflicts with TenantContext
- **THEN** system SHALL apply tenant-specific governance policies
- **AND** system SHALL prioritize knowledge items based on policy hierarchy
- **AND** system SHALL emit governance events for policy-driven decisions

#### Scenario: Enforce policy compliance during sync
- **WHEN** syncing knowledge items that violate tenant policies
- **THEN** system SHALL flag policy violations
- **AND** system SHALL optionally block sync based on policy configuration
- **AND** system SHALL emit PolicyViolation governance event