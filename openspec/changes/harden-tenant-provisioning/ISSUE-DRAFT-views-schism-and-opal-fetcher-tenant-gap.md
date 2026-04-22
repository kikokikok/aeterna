# Draft issue — v_hierarchy / v_user_permissions schism + opal-fetcher tenant gap

**For**: new GitHub issue on `kikokikok/aeterna`.
**Suggested labels**: `area/tenants`, `area/authz`, `priority/high`, `type/architecture`.
**Drafted**: 2026-04-23 during PR #129 (§2.2-B) B3 analysis.
**Context**: surfaced while planning §2.2-B3; blocks §2.2-B4 (reverse-render)
and §2.2-C (roles).

---

## Title

    v_hierarchy / v_user_permissions have two incompatible definitions;
    opal-fetcher has no tenant isolation

## Body

Surfaced during §2.2-B (PR #129) blast-radius analysis. Three compounding
architectural issues in the Cedar/OPAL authz feed that together block
§2.2-B3+ from coherent progress and constitute a latent cross-tenant
authz gap.

### Context

§2.2-B (PR #129) added `companies.tenant_id` (migration
`028_tenant_scoped_hierarchy.sql`) and rewrote `v_hierarchy` /
`v_user_permissions` to surface `tenant_id`. While planning §2.2-B3
(thread tenant_id through the readers), I discovered the view rewrite
is being silently undone on every IdP sync, and the main consumer
doesn't filter by tenant anyway.

### Problem 1 — Views defined twice, in two incompatible shapes

`v_hierarchy` and `v_user_permissions` are created by two different
code paths:

| Source | Backs against | Column shape | When it runs |
|---|---|---|---|
| `storage/migrations/009_organizational_referential.sql` + `028_tenant_scoped_hierarchy.sql` | `companies` / `organizations` / `teams` (real UUID PKs) | `c.tenant_id, c.id AS company_id, c.slug AS company_slug, …` | On `aeterna admin migrate up` |
| `idp-sync/src/github.rs::initialize_opal_views()` (lines 636 and 691) | `organizational_units` (legacy TEXT-id table, via recursive CTE) | `uuid_generate_v5(NS, ou.id) AS company_id`, `metadata->>'git_remote'`, tenant_id from `org_units.tenant_id` (TEXT) | On every `aeterna admin sync github` call |

Both use `CREATE OR REPLACE VIEW`. **Whichever runs last wins.**
Concretely: any GitHub sync after a fresh migration will silently
revert migration 028's view definition and point the view at
synthesized UUIDs that do not match the real `companies.id` values.

Search:

    grep -rn 'CREATE.*VIEW v_hierarchy\|CREATE.*VIEW v_user_permissions' \
      --include='*.sql' --include='*.rs'

returns both sources.

### Problem 2 — `opal-fetcher` is tenant-unaware

`opal-fetcher/src/handlers.rs`:

  - `get_hierarchy` → `SELECT ... FROM v_hierarchy` (no WHERE)
  - `get_users` → `SELECT ... FROM v_user_permissions` (no WHERE)
  - `get_agents` → `SELECT ... FROM agents WHERE deleted_at IS NULL
    AND status='active'` (no tenant filter)

All three emit the globally-merged entity set to OPAL → Cedar. Under
migration 028's multi-tenant assumption this is a cross-tenant
authorization leak: Cedar will evaluate policies against entities
from every tenant in the DB. Latent because dev envs are
single-tenant, but the entire point of harden-tenant-provisioning
is to make Aeterna actually multi-tenant.

### Problem 3 — Root cause: two unreconciled org-hierarchy implementations

| Implementation | Tables | PK | Tenant column | Status |
|---|---|---|---|---|
| Legacy | `organizational_units` | TEXT | `tenant_id TEXT` | Still written by `PostgresBackend::initialize_schema()`; still read by idp-sync's view override |
| Modern | `companies` / `organizations` / `teams` / `projects` | UUID | (post-028) `companies.tenant_id UUID FK → tenants(id)` | Written by bootstrap (commit `36d2c51b`); read by migration's view |

Both are alive. The view layer is the fault line where they collide.
§2.2-A/B/C have all built on the modern tables; idp-sync is the last
major writer still targeting the legacy one.

### Proposed resolution (“Option X” in #129 discussion, 2026-04-23)

1. Delete `initialize_opal_views()` from `idp-sync/src/github.rs`.
   Migration 028 becomes the single canonical definition.

2. Migrate idp-sync writes from `organizational_units` to
   `companies` / `organizations` / `teams`. Possible intermediate
   step: dual-write behind a feature flag until parity is proven in
   staging.

3. Tenant-filter opal-fetcher handlers. Add a `tenant_id` query
   parameter (or header, matching the rest of the auth stack) to
   `/v1/hierarchy`, `/v1/users`, `/v1/agents`. Filter by
   `WHERE tenant_id = $1`. Requires OPAL's client config to thread
   tenant context; check `opal-fetcher/README.md` for current
   client contract.

4. After dual-write parity is verified, delete `organizational_units`
   writes from `PostgresBackend::initialize_schema()` and drop the
   table in a follow-up migration.

### Sequencing with §2.2-B / §2.2-C

PR #129 (§2.2-B) pauses here with 4 commits merging: docs +
register-027 + bootstrap-fix + migration 028. The reverse-render
step (§2.2-B4 — `NOT_RENDERED_SECTIONS` shrink, `manifest_render.rs`
hierarchy reader) is blocked on this issue: reading `v_hierarchy`
is meaningless while the view's shape flips between two schemas,
and writing tenant-scoped data is meaningless while opal-fetcher
discards tenant isolation.

§2.2-C (roles reverse-render) has the same dependency.

### Acceptance criteria

  - [ ] Only one `CREATE VIEW v_hierarchy` in the repo (the migration).
  - [ ] Only one `CREATE VIEW v_user_permissions` in the repo.
  - [ ] opal-fetcher handlers filter every query by `tenant_id`.
  - [ ] Integration test: two tenants in DB; opal-fetcher called with
        tenant A's context returns only tenant A's entities.
  - [ ] idp-sync writes to the modern tables exclusively, OR the
        legacy tables have a documented deprecation path with a
        concrete removal milestone.

### References

  - PR #129 (§2.2-B migration)
  - `openspec/changes/harden-tenant-provisioning/NOTES-hierarchy-migration-blast-radius.md` §6
    (partial forecast of this issue, scoped only to `email_domain_patterns`)
  - `openspec/changes/harden-tenant-provisioning/FINDINGS-2-2.md` Finding 2
    (original schema reconciliation decision)
  - `idp-sync/src/github.rs` lines 633–720 (`initialize_opal_views`)
  - `opal-fetcher/src/handlers.rs` lines 80–170 (tenant-unaware handlers)
  - `storage/migrations/009_organizational_referential.sql` lines 259–300
  - `storage/migrations/028_tenant_scoped_hierarchy.sql`
