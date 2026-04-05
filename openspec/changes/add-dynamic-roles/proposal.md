## Why

The `Role` enum is deeply baked into ~25+ call sites across the codebase (`tenant_api.rs`, `AuthorizationService` trait, `GovernanceEvent`, `TenantContext`, auth middleware). This makes it impossible to define custom roles without recompiling Rust. Meanwhile, the Cedar schema already declares `User in [Team, Role]` entity membership, but all 1354 lines of `rbac.cedar` policies use flat `principal.role == "string"` checks instead — duplicating role logic in both Rust and Cedar with no single source of truth. Three separate Cedar evaluation paths (CedarAuthorizer, CedarClient, CedarPolicyEvaluator) compound the inconsistency. Critical bugs exist: `assign_role`/`remove_role` on `CedarAuthorizer` are stubs returning `Ok(())`, and `auth_middleware.rs` silently drops unknown role strings via `.parse::<Role>().ok()`.

## What Changes

- Introduce `RoleIdentifier` newtype (`Known(Role)` | `Custom(String)`) at service boundaries, gradually replacing `Role` enum in trait signatures
- **BREAKING**: `AuthorizationService` trait methods change from `Role` to `RoleIdentifier`
- Rewrite Cedar policies from `principal.role == "string"` to `principal in Role::"X"` (entity membership pattern) — making OPAL entity data the authority for role grants
- Implement real `assign_role`/`remove_role` in `CedarAuthorizer` (currently stubs)
- Fix `auth_middleware.rs` to handle unknown/custom roles instead of silently dropping them
- Collapse toward a single primary Cedar evaluation path to eliminate cross-path inconsistency
- Add contract tests: DB row → OPAL entity → Cedar authorization decision
- Generate the Rust `role_permission_matrix()` from Cedar policies (or remove the static duplicate)

**NOT in scope:**
- No changes to authentication — this is purely authorization
- No hierarchy changes (covered by `add-hierarchy-management`)
- No UI/dashboard changes

## Capabilities

### New Capabilities
- `dynamic-roles`: Dynamic role definition via Cedar entity membership, `RoleIdentifier` newtype, custom role support without Rust recompilation

### Modified Capabilities
- `multi-tenant-governance`: `AuthorizationService` trait evolves from `Role` to `RoleIdentifier`; Cedar policies rewritten to entity membership pattern; static permission matrix replaced with Cedar-derived matrix

## Impact

- **Core types**: `mk_core/src/types.rs` (new `RoleIdentifier`), `mk_core/src/traits.rs` (`AuthorizationService` signature change — **BREAKING**)
- **~25+ call sites**: `tenant_api.rs`, `role_grants.rs`, `auth_middleware.rs`, `bootstrap.rs`, `org_api.rs`, `team_api.rs`, `user_api.rs`, `govern_api.rs`, `admin_sync.rs`, `redis_publisher.rs`, `adapters/auth/cedar.rs`, `adapters/auth/permit.rs`
- **Cedar**: Full rewrite of `rbac.cedar` (1354 lines) from string attribute checks to entity membership
- **OPAL**: `opal-fetcher/src/entities.rs` already uses `role: String` — needs hardening with contract tests
- **Tests**: `adapters/tests/rbac_matrix_doc_test.rs` must be updated for new matrix generation approach
- **Breaking**: `AuthorizationService` trait signature change affects all implementors
