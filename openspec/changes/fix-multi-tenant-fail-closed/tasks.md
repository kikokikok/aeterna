## 1. Fail-closed tenant/auth boundary hardening

- [ ] 1.1 Remove hardcoded default tenant claims from plugin auth bootstrap and require verified tenant mapping before token issuance.
- [ ] 1.2 Remove `default_tenant_context()` / default-user fallbacks from knowledge and sync HTTP request handling.
- [ ] 1.3 Remove default tenant fallback from shared knowledge API header parsing and reject unresolved tenant context.
- [ ] 1.4 Replace webhook default tenant usage with a verified tenant derivation path or explicit rejection path.

## 2. MCP and runtime authorization tightening

- [ ] 2.1 Bind MCP `tenantContext` payloads to authenticated caller identity in the HTTP transport and dispatcher path.
- [ ] 2.2 Audit mounted route groups for tenant-scoped operations and add fail-closed request validation where missing.
- [ ] 2.3 Restrict allow-all auth defaults so production-capable modes require explicit non-permissive auth configuration.
- [ ] 2.4 Complete or explicitly guard unimplemented JWT auth paths on agent-facing surfaces.

## 3. Storage and isolation enforcement

- [ ] 3.1 Reconcile RLS migration/schema mismatches for tenant-scoped tables.
- [ ] 3.2 Activate database tenant context in hot-path Postgres connections where RLS is expected to protect runtime queries.
- [ ] 3.3 Verify tenant-scoped query coverage across knowledge, sync, and governance storage operations.

## 4. Tests and operational hardening

- [ ] 4.1 Add negative integration tests for missing tenant context, forged tenant payloads, and auth-disabled production-capable paths.
- [ ] 4.2 Add MCP-specific tests proving payload tenant scope cannot exceed authenticated tenant scope.
- [ ] 4.3 Update deployment/runtime documentation for any explicit dev-only permissive modes and fail-closed production expectations.
- [ ] 4.4 Validate the entire change with strict OpenSpec checks and targeted runtime tests before implementation sign-off.
