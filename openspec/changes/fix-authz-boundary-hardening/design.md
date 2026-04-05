## Context

Recent audits showed that the repository still has serious authorization-boundary drift relative to its specs. Some HTTP APIs trust raw `x-tenant-id` and `x-user-id` headers when plugin auth is disabled, `MemoryManager` is wired with allow-all auth, `TenantContext` lacks the role and hierarchy information described by the governance specs, and the Cedar authorizer still returns empty roles and no-op assignment behavior. Even with tests passing, these issues mean the effective tenant and authorization boundary is weaker than intended.

## Goals / Non-Goals

**Goals:**
- Ensure tenant-scoped APIs derive identity only from validated or explicitly trusted request sources.
- Eliminate allow-all authorization behavior from production-capable memory and control-plane paths.
- Align `TenantContext`, role catalog, and role operations with the governance specs and active policy bundle.
- Prevent self-approval and similar governance integrity violations.

**Non-Goals:**
- Implementing the full tenant administration control plane (tracked by `add-tenant-admin-control-plane`).
- Replacing Cedar or Permit with a new policy engine.

## Decisions

### Fail closed on missing validated identity for tenant-scoped APIs
Tenant-scoped APIs must reject requests when they do not carry validated plugin-auth identity or another explicitly trusted server-side identity source. Raw caller-controlled headers are not sufficient in production-capable modes.

### Use one canonical role catalog and propagated tenant context
Runtime types, API schemas, CLI validation, and policy evaluation must all use the same role catalog. `TenantContext` must carry the data needed for downstream enforcement rather than forcing every subsystem to rediscover role and hierarchy state ad hoc.

### Treat no-op role operations as unsupported until real persistence exists
Authorization adapters must either perform real role reads/writes or fail explicitly. Returning empty roles or pretending assignment succeeded is not acceptable.

## Risks / Trade-offs

- **[Risk] Stricter identity enforcement breaks local/dev shortcuts** → Mitigation: keep any local-only shortcuts explicitly gated to non-production modes with loud diagnostics.
- **[Risk] Expanding TenantContext increases middleware complexity** → Mitigation: centralize context construction in auth middleware and keep downstream consumers read-only.
- **[Risk] Role-catalog alignment touches many surfaces** → Mitigation: add compatibility tests and update one canonical definition first.

## Migration Plan

1. Define the hardening requirements and model changes in specs.
2. Update canonical role and context types.
3. Replace allow-all and header-only identity fallback behavior with validated paths.
4. Implement real role operations and governance approval protections.
5. Add negative tests for spoofing, self-approval, and missing-role enforcement.

## Open Questions

- Should dev-mode plain-header identity be removed entirely or explicitly gated behind a separate unsafe development flag?
- Should role lookup be fully cached in middleware or resolved lazily on first authorized operation per request?
