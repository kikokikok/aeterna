## 1. Implementation

### Phase 1: Core Type Migration
- [x] 1.1 Define `RoleIdentifier` newtype in `mk_core/src/types.rs` as `Known(Role) | Custom(String)`.
- [x] 1.2 Add `From<Role>` for `RoleIdentifier` conversion.
- [x] 1.3 Add `Display`, `Serialize`, `Deserialize`, `PartialEq`, and `Hash` implementations for `RoleIdentifier`.
- [x] 1.4 Update `AuthorizationService` trait: `get_user_roles` returns `Vec<RoleIdentifier>`.
- [x] 1.5 Update `AuthorizationService` trait: `assign_role` and `remove_role` accept `RoleIdentifier`.
- [x] 1.6 Update `TenantContext` role field type to `Vec<RoleIdentifier>`.

### Phase 2: Call Site Migration (~25+ files)
- [x] 2.1 Update `tenant_api.rs` role handling (including existing match sites).
- [x] 2.2 Update `role_grants.rs` `ScopedRoleGrant` role type usage.
- [x] 2.3 Update `auth_middleware.rs` to handle custom roles instead of silently dropping unknown roles.
- [x] 2.4 Update role references in `org_api.rs`, `team_api.rs`, and `user_api.rs`.
- [x] 2.5 Update role references in `govern_api.rs`.
- [x] 2.6 Update role wiring in `bootstrap.rs`.
- [x] 2.7 Update role handling in `admin_sync.rs`.
- [x] 2.8 Update `context/src/redis_publisher.rs` `GovernanceEvent` role fields.
- [x] 2.9 Update `adapters/src/auth/cedar.rs` `CedarAuthorizer` role handling.
- [x] 2.10 Update `adapters/src/auth/permit.rs` `PermitAuthorizationService` role handling.

### Phase 3: Cedar Policy Rewrite
- [x] 3.1 Rewrite `policies/cedar/rbac.cedar` from `principal.role == "string"` checks to `principal in Role::"X"` membership checks.
- [x] 3.2 Update `policies/cedar/aeterna.cedarschema` where needed for membership-based evaluation.
- [x] 3.3 Implement real `assign_role`/`remove_role` in `CedarAuthorizer` (replace stubs).
- [x] 3.4 Update Cedar test fixtures for membership-based role evaluation.

### Phase 4: OPAL Pipeline Hardening
- [x] 4.1 Update `opal-fetcher` entity output to Cedar entity membership format.
- [x] 4.2 Add contract tests for pipeline: DB row -> OPAL entity -> Cedar decision.
- [x] 4.3 Harden entity sync behavior for custom role data.

### Phase 5: Evaluator Consolidation
- [x] 5.1 Identify and document primary Cedar evaluation path (`CedarAuthorizer`).
- [x] 5.2 Deprecate/remove redundant evaluators or define explicit ownership boundaries.
- [x] 5.3 Generate `role_permission_matrix()` from Cedar policies.
- [x] 5.4 Update `adapters/tests/rbac_matrix_doc_test.rs` for policy-derived matrix generation.

### Phase 6: Testing
- [x] 6.1 Add unit tests for `RoleIdentifier` type behavior.
- [x] 6.2 Add integration tests for role assignment/removal flows.
- [x] 6.3 Add Cedar policy integration tests using entity membership.
- [x] 6.4 Add contract tests for full authorization pipeline.
- [x] 6.5 Add migration/backward-compatibility tests.
