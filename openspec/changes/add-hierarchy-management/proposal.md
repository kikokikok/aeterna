## Why

Project is a first-class `UnitType` in the codebase (Company→Org→Team→Project) but has zero REST API endpoints — no CRUD, no member management, no team-project assignment. The `current_scope_ids()` function in governance is hardcoded to company scope, meaning project-level authorization context never resolves. Role inheritance through the hierarchy is not computed — `get_user_roles()` returns flat assignments with no ancestor walking. This blocks any meaningful project-scoped authorization or governance.

## What Changes

- Add `project_api.rs` with full CRUD and member management, following the existing `org_api.rs` / `team_api.rs` patterns
- Add `project_team_assignments` table for many-to-many team↔project relationships with assignment types (`owner`, `contributor`)
- Fix `current_scope_ids()` to resolve project context (currently hardcoded to company)
- Implement `get_effective_roles_at_scope()` that walks ancestor hierarchy to compute inherited roles
- Enforce `CreateProject` Cedar action on the new API (action exists in schema but nothing uses it)
- Reconcile the two disconnected project ID namespaces (`organizational_units` UUIDs vs `drift_results`/`drift_configs` text slugs)

**NOT in scope:**
- No new `UnitType` variants (no Domain/Sub-domain) — use metadata/tags if portfolio taxonomy is needed later
- No changes to the 4-level hierarchy depth constraint
- No changes to authentication — this is purely authorization/hierarchy
- No dynamic role definitions (deferred to `add-dynamic-roles`)

## Capabilities

### New Capabilities
- `project-management`: Project CRUD API, team-project assignments, project member management, project-scoped authorization context

### Modified Capabilities
- `multi-tenant-governance`: Fix `current_scope_ids()` to support project-level scope resolution; add effective role computation through hierarchy ancestors

## Impact

- **New files**: `cli/src/server/project_api.rs`, migration for `project_team_assignments` table
- **Modified files**: `cli/src/server/router.rs` (mount project routes), `cli/src/server/govern_api.rs` (fix scope resolution), `storage/src/postgres.rs` (new queries), `mk_core/src/traits.rs` (add `get_effective_roles_at_scope` to `StorageBackend` or `AuthorizationService`)
- **Cedar**: Add project-scoped permit/forbid policies for new CRUD actions
- **OPAL**: opal-fetcher entity output may need project membership data
- **Breaking**: None — purely additive
