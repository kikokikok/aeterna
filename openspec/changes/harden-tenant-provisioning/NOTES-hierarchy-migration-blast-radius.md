# NOTES — Hierarchy Migration Blast Radius (§2.2-B)

**Date:** 2026-04-22
**Scope:** PR #129 follow-up commits
**Status:** Pre-migration analysis — SQL not yet written
**Owner decision:** Christian Klat selected option **(A) Manifest model wins** on 2026-04-22 22:16 UTC.

---

## Why this document exists

My original `FINDINGS-2-2.md` Finding 2 stated that `companies` /
`organizations` / `teams` tables **do not exist**. That was wrong —
migration `009_organizational_referential.sql` (pre-dating the
`tenants` table by 8 migrations) creates all three, plus `projects`,
`users`, `agents`, `memberships`, auxiliary pattern tables, views,
triggers, and an audit log.

The original finding slipped because I searched for the tables
scoped to the harden-tenant-provisioning work. Migration 009 is
from a **different design generation** that pre-dates multi-tenancy.

The tables exist but under an incompatible design assumption:

> **Migration 009, line 8 comment:** *"Each company is a separate
> tenant. Root of the organizational hierarchy."*

vs.

> **`TenantManifest.hierarchy: Vec<ManifestCompany>`** — one
> tenant owns arbitrarily many companies.

The two schemas were never reconciled. The `tenants` table landed
in migration `017_tenants_tables.sql` as an **independent**
multi-tenant isolation concept, but no cross-reference to
`companies` was ever introduced.

---

## Blast radius of option (A) — add `companies.tenant_id`

### Schema changes

**`companies`**
- `ADD COLUMN tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE`
- `DROP CONSTRAINT companies_slug_key` (the `UNIQUE(slug)` from migration 009)
- `ADD CONSTRAINT companies_tenant_slug_key UNIQUE (tenant_id, slug)`
- `DROP INDEX idx_companies_slug`
- `CREATE INDEX idx_companies_tenant_slug ON companies(tenant_id, slug) WHERE deleted_at IS NULL`

**`organizations` / `teams` / `projects`** — structurally unchanged.
They inherit tenant scoping transitively through `company_id` →
`companies.tenant_id`. The UNIQUE constraints (`UNIQUE(company_id, slug)`
etc.) already enforce per-tenant uniqueness through the company link.

**Views** — all three need the tenant column surfaced:
- `v_hierarchy` — add `c.tenant_id` as first column
- `v_user_permissions` — add `c.tenant_id`
- `v_agent_permissions` — already tenant-neutral (agents carry
  `allowed_company_ids`); no change, but worth documenting

### Existing FK-carrying tables that gain implicit tenant scoping

Not structurally changed, but semantically now tenant-scoped:

| Table | Column | Migration | Notes |
|---|---|---|---|
| `governance_roles` | `company_id`, `org_id`, `team_id` | 010 | Scope tuple already checked by `role_has_exactly_one_scope` constraint; tenant-awareness is free. |
| `governance_configs` | `company_id`, `org_id`, `team_id` | 010 | Per-scope config; tenant-awareness is free. |
| `approval_workflows` | `company_id`, `org_id`, `team_id` | 010 | Same. |
| `email_domain_patterns` | `company_id`, `default_org_id`, `default_team_id` | 009 | ⚠️ `email_domain_patterns.domain UNIQUE` is global. With multi-tenant companies, the same domain could legitimately point at different tenants' companies. **Follow-up §2.2-B2:** decide whether `UNIQUE(domain)` becomes `UNIQUE(tenant_id, domain)` or stays global. Not blocking §2.2-B but needs a decision before multi-tenant domain bootstrap. |
| `git_remote_patterns` | `company_id` | 009 | Tenant-scoped implicitly via company. |
| `agents` | `allowed_company_ids[]` | 009 | UUIDs continue to resolve; no change. |
| `organizational_units` | legacy | 009 | Legacy table; backfill step at bottom of 009 no longer runs (already applied once per DB). No change needed — but see **bootstrap.rs** below. |

19 `REFERENCES (companies|organizations|teams)` FKs total across the
migration tree. None require structural changes; all inherit tenant
scoping transitively. Audited via
`grep -rn 'REFERENCES (companies|organizations|teams)' storage/migrations/*.sql`.

### Rust call sites that need updating

**Must change:**

1. `cli/src/server/bootstrap.rs:943` — `INSERT INTO companies ... ON CONFLICT (slug)`
   - Breaks the moment `UNIQUE(slug)` is dropped.
   - Fix: `INSERT ... (tenant_id, slug, ...) ... ON CONFLICT (tenant_id, slug)`.
   - Caller passes a tenant_id. Bootstrap currently has a
     `cfg.company_slug` — needs the bootstrap tenant's UUID threaded
     in. The bootstrap tenant's ID is already resolved earlier in the
     function (bootstrap emits a default tenant row).

2. **No other direct company/org/team SQL in Rust** — confirmed via
   `grep -rln 'FROM companies|INSERT INTO companies' cli/ storage/`.
   Only match: `bootstrap.rs`. Good.

**Need review but probably unchanged:**

- `cli/src/server/team_api.rs`, `org_api.rs` equivalents — operate on
  `company_id` UUIDs passed in from HTTP; unaffected by slug uniqueness.
- `cli/src/commands/team.rs`, `org.rs` CLI — call server handlers;
  unaffected.
- `cli/src/server/govern_api.rs` — writes `governance_roles`; the
  scope-tuple constraint is unchanged.

### Backfill strategy for existing `companies` rows

Three possible states of an existing DB:

**State 1 — fresh DB (no companies rows).** Migration is a pure
DDL change. Trivial.

**State 2 — bootstrap has run (single `Default` company with
`slug = bootstrap cfg.company_slug`).** Match by slug against
`tenants.slug`. The bootstrap function already ensures these align
(bootstrap tenant slug == bootstrap company slug — read from the
same `cfg`). Safe 1:1 backfill.

**State 3 — arbitrary legacy data.** Unknown in dev; unknown in
shared envs. Migration will abort if any `companies.slug` has no
matching `tenants.slug`. Two options:

- **Option 3a (strict):** migration fails loudly with the list of
  orphan slugs; operator runs a cleanup SQL before retrying.
- **Option 3b (lax):** migration creates a `migration-orphan` tenant
  row and assigns unmatched companies to it. Reversible but hides
  real data issues.

**Recommendation: 3a.** Abort with a clear error message. The cost
of a failed migration in dev is a `DROP CASCADE`; the cost of a
silent orphan-tenant assignment in prod is invisible data corruption.

Backfill SQL skeleton:

```sql
-- Add the column nullable first so existing rows aren't rejected.
ALTER TABLE companies ADD COLUMN IF NOT EXISTS tenant_id UUID
    REFERENCES tenants(id) ON DELETE CASCADE;

-- Populate.
UPDATE companies c
   SET tenant_id = t.id
  FROM tenants t
 WHERE t.slug = c.slug
   AND c.tenant_id IS NULL;

-- Fail fast if any row couldn't be matched.
DO $$
DECLARE
    orphan_count INT;
BEGIN
    SELECT COUNT(*) INTO orphan_count FROM companies WHERE tenant_id IS NULL;
    IF orphan_count > 0 THEN
        RAISE EXCEPTION 'Migration 028 aborted: % companies rows have no matching tenants.slug. Clean up orphans before re-running.', orphan_count;
    END IF;
END $$;

-- Lock it down.
ALTER TABLE companies ALTER COLUMN tenant_id SET NOT NULL;

-- Constraint surgery.
ALTER TABLE companies DROP CONSTRAINT IF EXISTS companies_slug_key;
ALTER TABLE companies ADD CONSTRAINT companies_tenant_slug_key
    UNIQUE (tenant_id, slug);
DROP INDEX IF EXISTS idx_companies_slug;
CREATE INDEX IF NOT EXISTS idx_companies_tenant_slug
    ON companies(tenant_id, slug) WHERE deleted_at IS NULL;

-- View rewrites.
CREATE OR REPLACE VIEW v_hierarchy AS ...  -- add c.tenant_id first col
CREATE OR REPLACE VIEW v_user_permissions AS ...  -- add c.tenant_id
```

---

## Commit plan on PR #129

Breaking §2.2-B into four reviewable commits on top of the current
branch. Each stops at a naturally testable seam so a reviewer can
stop at any commit without leaving the tree broken.

### B1 — FINDINGS correction + this doc *(THIS COMMIT)*

- Correct `FINDINGS-2-2.md` Finding 2 ("tables don't exist" → "tables
  exist under pre-tenant design").
- Land this blast-radius doc so the migration has a rationale trail.
- No code. Pure documentation.

### B2 — Migration `028_tenant_scoped_hierarchy.sql` + `bootstrap.rs` update

- DDL as designed above (strict backfill, orphan-abort).
- Update `bootstrap.rs:943` `INSERT INTO companies` to carry
  `tenant_id` and switch `ON CONFLICT (slug)` → `ON CONFLICT (tenant_id, slug)`.
- Register the migration in `storage/src/migrations.rs`.
- Smoke test: `apply_all` green on a fresh DB.
- **Recommended stop-point for a migration review** — the largest
  commit of this chain, reviewer gets to eyeball the SQL without
  downstream noise.

### B3 — `HierarchyStore` trait + Postgres implementation

- `HierarchyStore::upsert_hierarchy(tenant_id, companies: Vec<Company>) -> Result<...>`
  with prune semantics (soft-delete rows no longer in manifest).
- `HierarchyStore::get_hierarchy(tenant_id) -> Result<Vec<Company>>`
  using a single `v_hierarchy` query.
- Rust types `Company` / `Org` / `Team` mirroring `ManifestCompany` et al.
- Wired onto `AppState` like the other stores.
- Unit tests on a local Postgres testcontainer (matching the
  `tenant_store` test pattern).

### B4 — Apply + reverse-render wiring + `NOT_RENDERED_SECTIONS` update

- `provision_tenant` Step N+1 calls `HierarchyStore::upsert_hierarchy`
  after config upsert (same-txn semantics if we can plumb a tx down;
  if not, documented best-effort with prune-on-retry).
- `manifest_render.rs` adds `render_hierarchy` reading `v_hierarchy`.
- `NOT_RENDERED_SECTIONS` shrinks to:
  `["secrets", "roles", "domainMappings", "providers.memoryLayers"]`
  (`hierarchy` removed).
- Round-trip integration test in `tenant_api.rs` tests submitting a
  hierarchy and reading it back identical.

---

## Non-goals explicitly deferred

- **§2.2-C roles reverse-render.** Still sequenced after §2.2-B.
  Scoping row shape `(company_id, org_id, team_id)` is fine as-is.
- **Members (`ManifestMember`) at org/team level.** The manifest
  carries them but `memberships` rows require resolving `user_id`
  against the `users` table. Deferred to a §2.2-B5 follow-up so
  §2.2-B can land without entangling user-resolution semantics.
  `validate_manifest` will be tightened in B4 to reject non-empty
  `members` arrays with a clear "not yet implemented" error rather
  than silently dropping them.
- **`email_domain_patterns.domain UNIQUE`** global vs tenant-scoped.
  Documented as follow-up; current `tenant_domain_mappings` (from
  migration 017) already handles the tenant-scoped case for the
  manifest pathway, so migration 009's table is effectively unused
  by the new provisioning flow.
- **`organizational_units` legacy table** in `bootstrap.rs`. Continues
  to be written alongside `companies` for bc. Removal is its own
  cleanup PR.
