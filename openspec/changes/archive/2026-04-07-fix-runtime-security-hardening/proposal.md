## Why

The runtime still has production-blocking security and operational hardening gaps: global permissive CORS, incomplete readiness checks, incomplete row-level security coverage, weak session persistence for HA, and unsafe webhook processing semantics. These gaps make the running service less secure and less truthful about its actual dependency state than the production specs require.

## What Changes

- Replace permissive runtime defaults with production-safe CORS and route exposure behavior.
- Extend readiness and health checks to reflect real dependency reachability and fail degraded services honestly.
- Expand row-level security coverage to all tenant-scoped governance and operational tables.
- Add explicit backend isolation requirements for PostgreSQL, Qdrant, and Redis so tenant isolation is enforced consistently at the persistence layer instead of relying on informal conventions.
- Harden webhook processing so signature verification and delivery semantics match the security requirements.
- Replace in-memory-only auth/session state used by production-capable flows with HA-safe persistence or explicit unsupported behavior.

## Capabilities

### New Capabilities
- `runtime-security-hardening`: production security and operational-hardening requirements for runtime surfaces, health semantics, and HA-safe control-plane behavior.

### Modified Capabilities
- `governance`: tighten production CORS and runtime auth-related deployment semantics.
- `runtime-operations`: require truthful health/readiness behavior for all critical dependencies.
- `multi-tenant-governance`: extend row-level security coverage for tenant-scoped governance data.
- `github-org-sync`: harden webhook verification and delivery guarantees for sync-triggering events.
- `storage`: define explicit tenant-isolation requirements for PostgreSQL, Qdrant, and Redis backends.

## Impact

- Affected code: `cli/src/server/{router,health,webhooks,plugin_auth,bootstrap}.rs`, storage RLS migrations, governance/sync persistence, session storage, and deployment docs/tests.
- Affected systems: browser-exposed APIs, readiness/liveness checks, GitHub webhook processing, HA session continuity, and database-level tenant isolation.
