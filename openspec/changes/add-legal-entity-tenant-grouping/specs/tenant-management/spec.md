## ADDED Requirements

### Requirement: Optional Legal Entity Reference on Tenant
The `tenants` table SHALL carry a nullable `legal_entity_id` foreign key referencing `legal_entities(id)` with `ON DELETE SET NULL`. The `TenantRecord` projection SHALL surface a nullable `legalEntity: { id, name, slug }` reference on every read path. The v1.5.x `tenants.legal_entity_name` text column added by migration 033 SHALL be promoted into rows of `legal_entities` as part of this change's migration and then dropped. Tenant CRUD SHALL continue to function unchanged when no legal entity is attached.

#### Scenario: Tenant projection includes legal entity reference
- **WHEN** a caller sends `GET /api/v1/tenants/{slug}` for a tenant that has `legal_entity_id` set
- **THEN** the response body SHALL include a `legalEntity: { id, name, slug }` object alongside the existing tenant fields
- **AND** the existing tenant fields (id, slug, name, status, timestamps) SHALL be unchanged in shape

#### Scenario: Tenant projection when no legal entity is attached
- **WHEN** a caller sends `GET /api/v1/tenants/{slug}` for a tenant with `legal_entity_id` NULL
- **THEN** the response body SHALL include `legalEntity: null`
- **AND** all other tenant fields SHALL be returned unchanged

#### Scenario: Migration promotes legacy text column losslessly
- **WHEN** the migration that introduces `legal_entities` runs against a database where `tenants.legal_entity_name` has been populated under v1.5.x
- **THEN** for every distinct non-NULL value of `legal_entity_name`, exactly one row SHALL be inserted into `legal_entities` with that value as `name` and a slugified form as `slug`
- **AND** every tenant whose `legal_entity_name` matched SHALL have its `legal_entity_id` populated to point at the corresponding new row
- **AND** the `tenants.legal_entity_name` column SHALL be dropped
- **AND** the `idx_tenants_legal_entity_name` index added by migration 033 SHALL be dropped

#### Scenario: Tenant creation without a legal entity continues to work
- **WHEN** a PlatformAdmin sends `POST /api/v1/tenants` without a `legalEntityId` field
- **THEN** the tenant SHALL be created with `legal_entity_id` NULL
- **AND** the response SHALL include `legalEntity: null`
