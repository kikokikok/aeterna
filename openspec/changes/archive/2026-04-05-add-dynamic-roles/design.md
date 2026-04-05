## Context

The `Role` enum is deeply baked into ~25+ call sites across the codebase, and the `AuthorizationService` trait is typed directly to `Role`. The current authorization stack has three Cedar evaluators (`CedarAuthorizer` in-process, `CedarClient` via sidecar HTTP, and `CedarPolicyEvaluator`), which increases the risk of inconsistent authorization decisions. Cedar schema already models role membership (`User in [Team, Role]`), but production policies still rely on flat string checks (`principal.role == "string"`). The `assign_role` and `remove_role` operations on `CedarAuthorizer` are currently stubs. `auth_middleware` silently drops unknown roles. At the data layer, `user_roles` already stores role values as `TEXT`, and `opal-fetcher` uses `role: String`, which is the existing dynamic seam.

## Goals / Non-Goals

- Goals:
  - Support dynamic role definitions via Cedar entity membership.
  - Introduce a `RoleIdentifier` newtype for service boundaries.
  - Allow custom roles without Rust recompilation.
  - Establish a single primary Cedar evaluation path.
  - Add contract-tested OPAL authorization pipeline coverage.
- Non-Goals:
  - No authentication changes.
  - No hierarchy model changes (handled by a separate change).
  - No UI/dashboard delivery in this change.

## Decisions

- Decision 1: Introduce `RoleIdentifier` newtype with `Known(Role)` and `Custom(String)` variants for gradual migration.
  - Why: Avoids breaking all ~25+ call sites at once and provides a backward-compatible bridge.
  - Alternatives considered:
    - Big-bang enum-to-String replacement: rejected due to high blast radius and migration risk.
    - Add `DynamicRole(String)` enum variant: rejected because it pollutes pattern matching and leaks migration complexity everywhere.

- Decision 2: Rewrite Cedar policies from `principal.role == "string"` to `principal in Role::"X"` entity membership.
  - Why: Aligns implementation with existing Cedar schema and makes OPAL entity data the single authority for role grants.
  - Alternatives considered:
    - Keep string-attribute checks: rejected because it perpetuates split authority between Rust enum parsing and Cedar policy strings.

- Decision 3: Collapse to a single primary Cedar evaluation path (`CedarAuthorizer`).
  - Why: Three evaluators create multiple opportunities for inconsistent authorization outcomes.
  - Alternatives considered:
    - Keep all three evaluators active: rejected due to increasing operational inconsistency and maintenance cost.

- Decision 4: Generate `role_permission_matrix()` from Cedar policies instead of maintaining a static Rust duplicate.
  - Why: Eliminates ongoing manual synchronization debt and drift.
  - Alternatives considered:
    - Delete matrix artifacts entirely: rejected because matrix output remains valuable for documentation and regression testing.

- Decision 5: Keep the eight system roles (`CompanyAdmin`, `OrgAdmin`, `TeamAdmin`, `ProjectAdmin`, `Architect`, `TechLead`, `Developer`, `Viewer`) as `Known` during transition, and let custom roles flow via Cedar membership.
  - Why: Provides phased migration safety while enabling immediate dynamic role extensibility.

## Risks / Trade-offs

- Breaking all `AuthorizationService` implementors during signature migration.
- OPAL synchronization latency may affect time-sensitive governance decisions.
- Silent role loss risk during migration if unknown/custom role handling is not consistently upgraded.

## Migration Plan

1. Phase 1: Add `RoleIdentifier` and bridging conversions.
2. Phase 2: Update trait signatures and dependent call sites.
3. Phase 3: Rewrite Cedar policies to entity-membership checks.
4. Phase 4: Collapse evaluator paths to one primary implementation.
5. Phase 5: Enable and verify custom-role flow end-to-end.
