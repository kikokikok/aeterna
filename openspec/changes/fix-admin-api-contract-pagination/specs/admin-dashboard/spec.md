## ADDED Requirements

### Requirement: Admin UI Response Contract Alignment
All admin UI pages MUST correctly handle the actual backend response shapes. TypeScript interfaces MUST match the backend serde serialization (camelCase after standardization).

#### Scenario: TenantDetailPage unwraps response envelope
- **WHEN** the TenantDetailPage fetches `GET /api/v1/admin/tenants/{slug}`
- **THEN** the TanStack Query hook MUST type the response as `{ success: boolean, tenant: TenantRecord }`
- **AND** access `data.tenant` to get the tenant record
- **AND** NOT treat the raw response as a flat `TenantRecord`

#### Scenario: AuditLogPage handles array response and correct field names
- **WHEN** the AuditLogPage fetches `GET /api/v1/govern/audit`
- **THEN** the response MUST be consumed as a `PaginatedResponse<GovernanceAuditEntry>` (after pagination change)
- **AND** `GovernanceAuditEntry` fields MUST use camelCase: `actorEmail`, `targetType`, `targetId`, `createdAt`
- **AND** NOT `actor`, `resource_type`, `resource_id`, `timestamp`

#### Scenario: PolicyListPage uses standard paginated envelope
- **WHEN** the PolicyListPage fetches `GET /api/v1/govern/policies`
- **THEN** the response MUST be consumed as `PaginatedResponse<PolicyRecord>`
- **AND** items MUST be accessed via `data.items`, not `data.policies` or `Array.isArray(data)`

#### Scenario: LifecyclePage uses correct field names and response wrapper
- **WHEN** the LifecyclePage fetches `GET /api/v1/admin/lifecycle/remediations`
- **THEN** the response MUST be consumed as `{ items: RemediationRequest[], count: number }`
- **AND** `RemediationRequest` fields MUST use camelCase: `requestType`, `riskTier`, `entityType`, `entityIds`, `proposedAction`, `detectedBy`, `createdAt`

#### Scenario: UserDetailPage uses correct role field names
- **WHEN** the UserDetailPage fetches `GET /api/v1/user/{id}/roles`
- **THEN** each role entry MUST be typed as `{ role: string, scope: string, unitId: string }`
- **AND** scope display MUST parse `scope` (e.g., "org/uuid"), not look for `resource_type`/`resource_id`

#### Scenario: UserDetailPage sends correct grant-role body
- **WHEN** the admin UI grants a role
- **THEN** the request body MUST be `{ "role": "...", "scope": "..." }`
- **AND** NOT `{ "role": "...", "resource_type": "...", "resource_id": "..." }`

#### Scenario: KnowledgeSearchPage handles actual KnowledgeItem shape
- **WHEN** the KnowledgeSearchPage displays query results
- **THEN** it MUST display fields from the actual `KnowledgeItem`: `id`, `content`, `path`, `layer`, `tags`, `variantRole`
- **AND** MUST NOT expect non-existent fields: `kind`, `status`, `author`, `commitHash`, `updatedAt`

#### Scenario: MemorySearchPage sends correct feedback request
- **WHEN** a user clicks a feedback button on a memory entry
- **THEN** the admin UI MUST call `POST /api/v1/memory/{id}/feedback` with `{ "layer": "...", "rewardType": "positive", "score": 1.0 }`
- **AND** NOT call a non-existent path with `{ "feedback": "positive" }`

#### Scenario: KnowledgeDetailPage uses correct endpoint
- **WHEN** the KnowledgeDetailPage navigates to a knowledge item
- **THEN** it MUST call `GET /api/v1/knowledge/{id}` (which now exists)
- **AND** display the full `KnowledgeItem` with correct camelCase field mapping
