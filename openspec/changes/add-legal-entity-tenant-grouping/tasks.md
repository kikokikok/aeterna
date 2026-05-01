## 1. Schema and migration

- [ ] 1.1 Create `storage/migrations/NNN_legal_entities.sql` adding the `legal_entities` table with columns `id UUID PK`, `name TEXT NOT NULL`, `slug TEXT UNIQUE NOT NULL`, `msa_reference TEXT`, `contract_start_date DATE`, `contract_end_date DATE`, `primary_billing_contact TEXT`, standard timestamps, `source_owner` (mirroring tenants).
- [ ] 1.2 In the same migration, `ALTER TABLE tenants ADD COLUMN legal_entity_id UUID REFERENCES legal_entities(id) ON DELETE SET NULL`.
- [ ] 1.3 Populate `legal_entities` from the existing v1.5.x text column: `INSERT INTO legal_entities (name, slug) SELECT DISTINCT legal_entity_name, slugify(legal_entity_name) FROM tenants WHERE legal_entity_name IS NOT NULL`.
- [ ] 1.4 Backfill `tenants.legal_entity_id` from the joined name match.
- [ ] 1.5 Drop `tenants.legal_entity_name` and the `idx_tenants_legal_entity_name` partial index added by migration 033.
- [ ] 1.6 Add a partial-index `idx_tenants_legal_entity_id ON tenants(legal_entity_id) WHERE legal_entity_id IS NOT NULL` for the rollup query path.
- [ ] 1.7 Mirror the schema additions in `storage/src/postgres.rs::initialize_schema()` so in-process test/dev DBs converge to the same shape.
- [ ] 1.8 Update `storage/migrations/README.md` with a note explaining how 033's text column was promoted (so future engineers don't search for a column that no longer exists).

## 2. Storage layer

- [ ] 2.1 Add `mk_core::types::LegalEntityRecord` mirroring `TenantRecord`'s shape (camelCase, ToSchema, RFC 3339 timestamps).
- [ ] 2.2 New `storage/src/legal_entity_store.rs` with `LegalEntityStore::{create, list, get, update, delete}` plus `list_tenants(legal_entity_id)` and `summarise(legal_entity_id) -> LegalEntitySummary`.
- [ ] 2.3 Extend `storage::tenant_store::TenantRecord` projection to JOIN `legal_entities` and surface `legal_entity: Option<LegalEntityRef>` on every read path.
- [ ] 2.4 Add `TenantStore::attach_legal_entity(slug, legal_entity_id)` and `detach_legal_entity(slug)` methods.
- [ ] 2.5 Storage tests: round-trip create/list/get for legal entities, attach/detach FK changes, deleting a legal entity sets owned tenants' FK to NULL (non-destructive), tenant queries surface the legal entity name.
- [ ] 2.6 Migration test: a fixture DB with the v1.5.x `legal_entity_name` text column populated produces an equivalent populated `legal_entities` table and `tenants.legal_entity_id` after running this migration; lossless round-trip.

## 3. API surface

- [ ] 3.1 New `cli/src/server/legal_entity_api.rs` with routes:
      - `GET /api/v1/legal-entities` — list (PlatformAdmin only).
      - `POST /api/v1/legal-entities` — create (PlatformAdmin only).
      - `GET /api/v1/legal-entities/{id}` — detail (PlatformAdmin or LegalEntityAdmin of that LE).
      - `PATCH /api/v1/legal-entities/{id}` — update metadata (PlatformAdmin only).
      - `DELETE /api/v1/legal-entities/{id}` — soft-delete; tenant FKs go to NULL.
      - `GET /api/v1/legal-entities/{id}/tenants` — list tenants in this LE.
      - `GET /api/v1/legal-entities/{id}/summary` — cross-tenant rollup (counts, health, storage usage, seat usage). Implemented as N per-tenant scoped queries.
- [ ] 3.2 New `POST /api/v1/tenants/{slug}/legal-entity` and `DELETE /api/v1/tenants/{slug}/legal-entity` to attach/detach.
- [ ] 3.3 Wire all routes into the Utopia OpenAPI registration alongside tenant routes.
- [ ] 3.4 Per-handler audit: every cross-tenant rollup logs one `audit_action` row *per underlying tenant* with `action="legal_entity_summary_read"`, so the per-tenant audit trail remains complete.
- [ ] 3.5 Integration tests against the testcontainer for happy path + auth boundaries (PlatformAdmin OK, LegalEntityAdmin scoped to own LE only, regular tenant user 403'd).

## 4. Auth

- [ ] 4.1 Extend the principal subject enum (`mk_core::types::Subject` or wherever it lives) with `LegalEntityAdmin { legal_entity_id: Uuid }`.
- [ ] 4.2 Token claims: agree on the JWT/IdP claim that maps to LegalEntityAdmin (existing convention follows the `aeterna_role` claim; reuse it with value `legal_entity_admin:<id>`).
- [ ] 4.3 Authorisation helper `require_legal_entity_admin_for(legal_entity_id)` in the same shape as `require_platform_admin`.
- [ ] 4.4 Verify in tests that LegalEntityAdmin cannot directly query a tenant's RLS-protected tables; they must go through the rollup endpoints.
- [ ] 4.5 RLS audit: confirm no policy is loosened by this work — the cross-tenant aggregation is *handler-side*, not policy-side.

## 5. Admin UI

- [ ] 5.1 New navigation level: when the user's principal is `LegalEntityAdmin`, the top nav shows the LE name and a tenant switcher restricted to its tenants.
- [ ] 5.2 New `LegalEntityDashboard` view with the cross-tenant summary cards (tenant count, total memories, total storage, open incidents, license seats).
- [ ] 5.3 Tenant detail panel surfaces `Legal Entity:` row with link.
- [ ] 5.4 New `LegalEntityListPage` for PlatformAdmin to manage all LEs, attach/detach tenants.
- [ ] 5.5 Visual regression coverage with Playwright: screenshots of new dashboard, attach/detach dialog, tenant detail with LE link.

## 6. CLI

- [ ] 6.1 New `cli/src/commands/legal_entity.rs` with subcommands: `create`, `list`, `show`, `update`, `delete`, `attach <tenant-slug>`, `detach <tenant-slug>`, `summary`.
- [ ] 6.2 Wire under `aeterna legal-entity ...` in the top-level CLI dispatch.
- [ ] 6.3 Help text and examples in `docs/cli/legal-entity.md`.

## 7. Documentation

- [ ] 7.1 New `docs/architecture/legal-entity-vs-tenant.md` explaining the boundary: tenant = data plane / RLS / migration / backup / quota; legal entity = corporate / billing / sales / cross-tenant rollup.
- [ ] 7.2 Update `docs/operations/onboarding.md` to mention the optional LE assignment step at tenant creation.
- [ ] 7.3 Update `docs/operations/tenant-provisioning.md` to reflect the new `legal_entity_id` field on tenant manifests (optional).
- [ ] 7.4 Update `openspec/specs/tenant-management/spec.md` (or equivalent) to reflect the new optional FK.

## 8. Decommissioning of the v1.5.x metadata seed

- [ ] 8.1 Remove the back-compat `legal_entity_name` references from `cli/src/server/tenant_api.rs::UpdateTenantRequest` (now superseded by the FK-based attach/detach endpoints).
- [ ] 8.2 Remove the `tenants_legal_entity_test.rs` file added in v1.5.x (the test scenarios are folded into `legal_entity_test.rs`).
- [ ] 8.3 Document the removal in the migration's header so an operator stepping through migrations linearly understands why 033 contributed a column that 0NN promoted then dropped.
