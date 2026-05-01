## ADDED Requirements

### Requirement: LegalEntityAdmin Principal
The authentication and authorisation layer SHALL recognise a new principal subject `LegalEntityAdmin { legal_entity_id: Uuid }`. A LegalEntityAdmin SHALL be authorised to read across all tenants owned by their legal entity through the cross-tenant rollup endpoints, and SHALL NOT be authorised to read row-level data of any tenant directly, nor to write to any tenant. The principal SHALL be resolvable from a single agreed-upon token claim.

#### Scenario: Token claim resolves to LegalEntityAdmin
- **WHEN** an authenticated request carries a token with claim `aeterna_role = "legal_entity_admin:<uuid>"`
- **AND** the supplied UUID corresponds to an existing, non-deleted legal entity
- **THEN** the server SHALL resolve the principal to `LegalEntityAdmin { legal_entity_id: <uuid> }`
- **AND** the server SHALL proceed to authorisation checks using that principal

#### Scenario: LegalEntityAdmin can read summary of own legal entity
- **WHEN** a LegalEntityAdmin for legal entity X sends `GET /api/v1/legal-entities/X/summary`
- **THEN** the server SHALL authorise the request
- **AND** the response SHALL contain the cross-tenant rollup for X

#### Scenario: LegalEntityAdmin cannot read summary of other legal entities
- **WHEN** a LegalEntityAdmin for legal entity X sends `GET /api/v1/legal-entities/Y/summary` where Y ≠ X
- **THEN** the server SHALL return `403 Forbidden` with error code `forbidden_legal_entity`
- **AND** the server SHALL NOT execute any per-tenant query

#### Scenario: LegalEntityAdmin cannot directly query tenant data
- **WHEN** a LegalEntityAdmin sends any request to a tenant-scoped endpoint (e.g. `GET /api/v1/memory`, `POST /api/v1/org`) with `X-Tenant-ID` set to a tenant they own
- **THEN** the server SHALL return `403 Forbidden` with error code `forbidden`
- **AND** the server SHALL NOT enumerate any tenant data

#### Scenario: LegalEntityAdmin cannot mutate any resource
- **WHEN** a LegalEntityAdmin sends a `POST`, `PUT`, `PATCH`, or `DELETE` to any endpoint other than the LegalEntityAdmin self-profile endpoints (if any exist)
- **THEN** the server SHALL return `403 Forbidden`
- **AND** the server SHALL NOT mutate any database row

### Requirement: RLS Policies Remain Unchanged
The introduction of LegalEntityAdmin SHALL NOT loosen any existing PostgreSQL RLS policy. Cross-tenant aggregation SHALL be implemented in the handler layer by issuing N tenant-scoped queries, NOT by adding a new BYPASS clause or by widening any existing policy.

#### Scenario: RLS policy review confirms no policy is loosened
- **WHEN** the change is reviewed against the policy file `storage/migrations/*_rls_*.sql`
- **THEN** no existing policy SHALL be modified to grant access based on `legal_entity_id`
- **AND** no new BYPASS RLS grant SHALL be added for any role introduced by this change
