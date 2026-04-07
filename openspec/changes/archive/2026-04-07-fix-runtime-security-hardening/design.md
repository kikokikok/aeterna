## Context

Independent audits found several runtime surfaces that still violate the production posture described in the specs. The HTTP router applies permissive CORS globally, health endpoints can report healthy even when critical dependencies are unavailable, row-level security covers only a subset of tenant-scoped tables, webhook processing can bypass or weaken expected verification semantics, and refresh/session state for plugin auth is still stored only in-memory.

## Goals / Non-Goals

**Goals:**
- Make the runtime fail closed or report degraded truthfully when required dependencies or security conditions are missing.
- Ensure browser and webhook boundaries are configured safely for production-capable deployments.
- Extend database-enforced tenant isolation to all tenant-scoped governance and operational data.
- Make auth/session state resilient enough for HA deployments or explicitly unsupported where HA is claimed.

**Non-Goals:**
- Replacing the entire deployment architecture or ingress stack.
- Implementing tenant-admin control plane features (tracked elsewhere).

## Decisions

### No permissive browser boundary in production-capable mode
Production-capable deployments must use configured origin allowlists. Permissive CORS can remain only as an explicitly gated local-development mode.

### Readiness must include critical downstreams
Readiness must fail or degrade when critical backing services such as the vector store, Redis/session backing store, or similar required dependencies are unavailable for the configured mode.

### Database isolation must cover governance/control-plane data too
RLS is not complete if governance and administrative tables remain outside tenant-scoped enforcement.

### Persistence isolation must be explicit per backend
PostgreSQL, Qdrant, and Redis do not provide the same isolation guarantees by default, so the spec and implementation must describe exactly how each backend enforces tenant boundaries. PostgreSQL should use row-level security and tenant session context on all tenant-scoped tables, Qdrant should use tenant-scoped collections or mandatory tenant filters enforced by the storage layer, and Redis should use tenant-scoped key namespaces wrapped by storage APIs so callers cannot read or write cross-tenant keys accidentally.

### Webhook processing must verify first and deliver reliably
Webhook-triggered control-plane mutations must not run before required verification, and delivery should provide retryable failure behavior instead of silent best-effort fire-and-forget semantics.

## Risks / Trade-offs

- **[Risk] Stricter CORS defaults break informal browser workflows** → Mitigation: provide explicit local-dev configuration and clear errors.
- **[Risk] Stronger readiness checks increase perceived instability** → Mitigation: expose degraded diagnostics clearly and only require dependencies needed by the configured mode.
- **[Risk] Broader RLS coverage exposes schema assumptions** → Mitigation: add migrations/tests table-by-table with tenant-isolation fixtures.
- **[Risk] Soft isolation in Qdrant/Redis drifts from intended tenant guarantees** → Mitigation: define backend-specific isolation contracts and add regression tests for cross-tenant leakage attempts.

## Migration Plan

1. Define runtime hardening requirements in specs.
2. Gate permissive CORS to local-only mode and implement production allowlists.
3. Add truthful readiness checks for critical dependencies.
4. Extend RLS coverage and tests to governance/control-plane tables.
5. Harden webhook verification/order and HA session persistence.

## Open Questions

- Which dependencies are required for readiness in each supported runtime mode?
- Should webhook delivery use durable queueing or retryable job persistence first?
