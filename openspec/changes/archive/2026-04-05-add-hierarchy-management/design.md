## Context

Project already exists as a first-class `UnitType` in the enforced Company → Org → Team → Project hierarchy, but there is currently no REST surface for project lifecycle or membership operations. Existing `org_api.rs` and `team_api.rs` handlers establish the API and middleware patterns that project APIs should follow.

Governance scope resolution is currently incomplete for project scope: `current_scope_ids()` in `govern_api.rs` is effectively hardcoded to company-level assumptions, and `get_user_roles()` returns flat assignments rather than inherited/effective roles across ancestors.

The system also has two disconnected project identifier namespaces:
- `organizational_units` UUID-backed project IDs (hierarchy source of truth)
- text slug project IDs in drift-related tables (for example drift results/configs)

This split prevents reliable project-scoped joins, policy checks, and governance computations.

## Goals

- Provide project CRUD REST APIs aligned with existing org/team API conventions
- Support team-project assignments for cross-team collaboration without changing core hierarchy depth constraints
- Make governance scope resolution project-aware
- Compute effective roles at scope by walking hierarchy ancestors

## Non-Goals

- No new Domain or Sub-domain `UnitType`
- No recursive project nesting
- No authentication changes
- No dynamic role definitions (handled in a separate change)

## Decisions

### Decision 1: Mirror existing org/team API structure for projects
Implement `project_api.rs` by following `org_api.rs` and `team_api.rs` patterns for route layout, middleware, handler structure, and response conventions.

Alternative considered: introduce a new generic unit API abstraction first.
Rejected because it adds refactor scope and risk; this change needs parity with proven patterns and fast delivery.

### Decision 2: Model collaboration with `project_team_assignments` many-to-many table
Add a dedicated `project_team_assignments` table with `assignment_type` (`owner`/`contributor`) instead of representing collaboration as parent-child hierarchy edges.

Alternative considered: encode cross-team collaboration directly in the organizational tree.
Rejected because hierarchy semantics remain strict Company→Org→Team→Project and collaboration is orthogonal.

### Decision 3: Derive project leadership from owner assignment edges
Implement "team lead inherits project leadership" via owner team→project assignment edges, not by introducing recursive/nested project structures.

Alternative considered: nested project hierarchies for ownership delegation.
Rejected because recursive project trees violate current depth constraints and complicate scope/path guarantees.

### Decision 4: Fix `current_scope_ids()` using existing hierarchy ancestor queries
Extend scope resolution in `govern_api.rs` to resolve project context using ancestor CTE queries already available in `postgres.rs`.

Alternative considered: keep company-only context and infer project permissions in policy layer only.
Rejected because upstream scope resolution must be correct for consistent authorization and governance decisions.

### Decision 5: Add `get_effective_roles_at_scope()` to `StorageBackend`
Add a new trait method to compute effective roles at a target scope by walking ancestors with existing recursive CTE infrastructure.

Alternative considered: compute inheritance ad hoc in API handlers.
Rejected because role computation belongs in storage/backend domain logic and must be reusable across API and governance paths.

### Decision 6: Reconcile project ID namespaces with UUID as canonical
Use `organizational_units` UUID project IDs as canonical identifiers across subsystems and add foreign key references from drift tables.

Alternative considered: keep text slugs as canonical and map hierarchy IDs to slugs at runtime.
Rejected because runtime translation is brittle, expensive, and weakens referential integrity.

## Risks / Trade-offs

- Data migration risk while reconciling UUID and slug namespaces
- Backward compatibility risk for existing drift data that currently references text project identifiers
- Temporary dual-read/translation complexity during migration windows

## Migration Plan

1. Add migration to create `project_team_assignments` with constraints and indexes.
2. Add UUID project foreign key columns to drift-related tables and backfill from existing slug mappings.
3. Validate backfill completeness and enforce referential integrity constraints.
4. Update codepaths to read/write canonical UUID project IDs.
5. Preserve compatibility for existing records during transition, then remove obsolete slug dependency once verified.
