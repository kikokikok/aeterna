## Context

The existing specs already require fail-closed tenant isolation, but the runtime still contains several request paths that manufacture a `default` tenant or `system` user instead of rejecting the request. These defaults appear in plugin auth token issuance, HTTP API helpers, webhook handling, and library-level header parsing. MCP adds a second class of risk: the caller can currently provide `tenantContext` directly in JSON-RPC params without the server binding that context to an authenticated identity.

This change is security-sensitive and cross-cutting. It affects tenant derivation, auth defaults, middleware boundaries, storage protections, and deployment assumptions. It also has operational impact because local/dev workflows may currently rely on permissive defaults that must no longer survive into production-capable modes.

## Goals / Non-Goals

**Goals:**
- Eliminate implicit default tenant/user assignment from production request-handling boundaries.
- Require tenant context to come from authenticated claims or trusted identity mappings, not caller-controlled defaults.
- Bind MCP tenant scope to the authenticated caller.
- Make storage-level tenant protections effective in real runtime hot paths, not only in isolated code paths.
- Preserve explicit, well-documented local/dev behavior without letting it masquerade as production-safe isolation.

**Non-Goals:**
- Expanding CLI feature parity or packaging; that belongs to the separate CLI control-plane change.
- Redesigning the organizational model or tenant hierarchy itself.
- Replacing Cedar/Permit/OPAL authorization architecture wholesale.

## Decisions

### Fail closed instead of manufacturing `default` tenant context
Request handlers, auth bootstrap flows, and supporting libraries will reject missing or unresolvable tenant context instead of assigning `default`/`system` identities.

**Alternatives considered:**
- **Keep `default` for convenience in dev**: rejected because it violates existing spec requirements and makes boundary mistakes look healthy.
- **Allow default only when auth disabled**: rejected because auth-disabled modes are exactly where accidental production drift is dangerous.

### Separate authenticated identity from caller-supplied tenant payloads
The runtime must derive or verify tenant scope from authenticated identity before accepting any caller-provided `tenantContext`. MCP payloads, headers, or webhook metadata cannot be trusted on their own.

**Alternatives considered:**
- **Trust `tenantContext` payload if present**: rejected because it allows caller-controlled tenant escalation.
- **Trust plugin token tenant blindly while leaving payload unchecked**: rejected because payload and token mismatch must be treated as an auth error.

### Make permissive auth defaults explicitly non-production
Allow-all auth backends or similarly permissive fallbacks must be explicitly constrained to non-production/dev contexts with loud signaling.

### Treat RLS as defense in depth, not a paper feature
Hot-path connections must activate tenant session context where RLS is expected to protect tenant-scoped tables, and migration/schema mismatches must be corrected.

**Alternatives considered:**
- **Rely only on app-layer WHERE clauses**: rejected because the existing specs already require stronger DB-level controls.

## Risks / Trade-offs

- **[Risk] Existing local/dev workflows break once defaults are removed** → Mitigation: introduce explicit dev-mode behavior and clear errors/documentation instead of implicit fallbacks.
- **[Risk] Some routes still hide unaudited fallback logic** → Mitigation: add route-level negative tests and broad audit coverage in this change.
- **[Risk] Tightening MCP context binding impacts current clients** → Mitigation: document the contract and provide migration guidance for authenticated callers.
- **[Risk] RLS activation changes expose hidden query assumptions** → Mitigation: stage with integration tests and verify table coverage before rollout.

## Migration Plan

1. Add spec deltas that forbid default-tenant fallbacks and require auth-bound tenant derivation.
2. Remove or gate default tenant helpers in auth/API/webhook paths.
3. Bind MCP tenant context to authenticated identity.
4. Correct RLS coverage/activation and add integration tests.
5. Update deployment/runtime docs to reflect explicit dev-only permissive modes if any remain.

## Open Questions

- What is the supported dev-mode contract when no tenant/auth boundary is configured: explicit local single-tenant mode, seeded fixture tenant, or mandatory config before startup?
- Which authenticated identity source should dominate for plugin-authenticated MCP calls when multiple potential tenant hints exist?
- Are there additional tenant-scoped tables beyond the currently audited hot paths that also need RLS activation in this change?
