## ADDED Requirements

### Requirement: Legal Entity Resource
The platform SHALL persist a first-class `LegalEntity` record at the same tier as `Tenant`. A legal entity has a stable UUID id, a unique slug, a human-readable name, and optional billing/contract metadata fields (`msa_reference`, `contract_start_date`, `contract_end_date`, `primary_billing_contact`). The legal entity record is governance metadata; it is NOT an RLS boundary, and storing data inside it is explicitly out of scope.

#### Scenario: Creating a legal entity
- **WHEN** a PlatformAdmin sends `POST /api/v1/legal-entities` with body `{ name: "Acme Holding", slug: "acme-holding" }`
- **THEN** the server SHALL persist a row in `legal_entities` with the supplied name and slug, a generated UUID id, and current timestamps
- **AND** the server SHALL return `201 Created` with the full `LegalEntityRecord` (camelCase, RFC 3339 timestamps)

#### Scenario: Slug uniqueness
- **WHEN** a PlatformAdmin sends `POST /api/v1/legal-entities` with a `slug` that already exists
- **THEN** the server SHALL return `409 Conflict` with error code `legal_entity_slug_taken`
- **AND** the server SHALL NOT mutate any existing row

#### Scenario: Listing legal entities is PlatformAdmin-only
- **WHEN** a non-PlatformAdmin user sends `GET /api/v1/legal-entities`
- **THEN** the server SHALL return `403 Forbidden` with error code `forbidden`
- **AND** the server SHALL NOT enumerate any legal entity

#### Scenario: Soft-deleting a legal entity preserves owned tenants
- **WHEN** a PlatformAdmin sends `DELETE /api/v1/legal-entities/{id}`
- **AND** the legal entity owns one or more tenants
- **THEN** the server SHALL mark the legal entity as deleted (timestamp on `deleted_at`)
- **AND** the server SHALL set `tenants.legal_entity_id` to `NULL` for every owned tenant
- **AND** the server SHALL NOT delete or otherwise modify any tenant data

### Requirement: Tenant Attachment to Legal Entity
A tenant SHALL have at most one legal entity. Attachment and detachment are explicit operations that mutate `tenants.legal_entity_id` and emit audit records. The relationship is 1:N (one legal entity owns N tenants); many-to-many is explicitly rejected.

#### Scenario: Attaching a tenant
- **WHEN** a PlatformAdmin sends `POST /api/v1/tenants/{slug}/legal-entity` with body `{ legalEntityId: "<uuid>" }`
- **AND** both the tenant and the legal entity exist and are not soft-deleted
- **THEN** the server SHALL set `tenants.legal_entity_id` to the supplied UUID
- **AND** the server SHALL emit an audit row with `action="tenant_attached_to_legal_entity"`
- **AND** the server SHALL return `200 OK` with the updated `TenantRecord`

#### Scenario: Detaching a tenant
- **WHEN** a PlatformAdmin sends `DELETE /api/v1/tenants/{slug}/legal-entity`
- **THEN** the server SHALL set `tenants.legal_entity_id` to `NULL`
- **AND** the server SHALL emit an audit row with `action="tenant_detached_from_legal_entity"`
- **AND** the server SHALL return `200 OK`

#### Scenario: Attempting to attach to a non-existent legal entity
- **WHEN** a PlatformAdmin sends `POST /api/v1/tenants/{slug}/legal-entity` with a `legalEntityId` that does not exist
- **THEN** the server SHALL return `404 Not Found` with error code `legal_entity_not_found`
- **AND** the server SHALL NOT mutate the tenant row

### Requirement: Cross-Tenant Rollup Without RLS Bypass
The legal-entity rollup endpoints SHALL aggregate read-only data across the tenants owned by a single legal entity. The aggregation SHALL be implemented as N separate per-tenant scoped queries, each running under that tenant's RLS context, with results combined in the handler. The handler SHALL NOT issue a privileged cross-tenant join, and SHALL NOT bypass any RLS policy.

#### Scenario: Summary endpoint aggregates per-tenant queries
- **WHEN** a caller sends `GET /api/v1/legal-entities/{id}/summary`
- **AND** the legal entity owns N tenants
- **THEN** the handler SHALL execute N independent queries, each scoped to one tenant's RLS context
- **AND** the handler SHALL combine the per-tenant results in memory
- **AND** the response SHALL contain aggregate counts (tenants, memories, storage bytes, open incidents, license seats) and a `tenants[]` array of per-tenant breakdowns

#### Scenario: One audit row per underlying tenant
- **WHEN** the rollup handler runs N per-tenant queries during a single request
- **THEN** the handler SHALL write N audit rows, one per tenant scope it observed
- **AND** each audit row SHALL carry `action="legal_entity_summary_read"` and the rollup endpoint's request id for correlation
- **AND** the per-tenant audit trail SHALL remain complete for compliance review

#### Scenario: Rollup is read-only
- **WHEN** any HTTP method other than `GET` is sent to `/api/v1/legal-entities/{id}/summary` or `/api/v1/legal-entities/{id}/tenants`
- **THEN** the server SHALL return `405 Method Not Allowed`
- **AND** the server SHALL NOT mutate any tenant or legal-entity row
