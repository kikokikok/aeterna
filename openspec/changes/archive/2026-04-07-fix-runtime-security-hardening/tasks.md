## 1. Runtime browser and route boundary hardening

- [x] 1.1 Replace unconditional permissive CORS with production-safe configured allowlists and explicit local-only permissive mode.
- [x] 1.2 Audit exposed HTTP route groups and ensure production-only routes use the required auth and browser boundary settings.

## 2. Truthful health and readiness

- [x] 2.1 Implement real health/readiness checks for vector-store and other critical runtime dependencies.
- [x] 2.2 Add readiness coverage for the session/Redis or equivalent state backend required by the configured mode.
- [x] 2.3 Add regression tests ensuring degraded or unavailable dependencies produce truthful readiness results.

## 3. Database and webhook hardening

- [x] 3.1 Extend RLS coverage to all tenant-scoped governance and control-plane tables.
- [x] 3.2 Add tenant-isolation regression tests for the newly protected tables.
- [x] 3.3 Enforce webhook verification before processing sync-triggering events and add negative tests for unverified events.
- [x] 3.4 Replace fire-and-forget webhook-triggered mutations with retryable or durable failure semantics.
- [x] 3.5 Define and enforce tenant-scoped collection/filter isolation for Qdrant-backed persistence paths.
- [x] 3.6 Define and enforce tenant-scoped key namespace isolation for Redis-backed persistence and cache paths.

## 4. HA-safe session and auth state

- [x] 4.1 Replace in-memory-only refresh/session state used by production-capable auth flows with HA-safe persistence or explicit unsupported behavior.
- [x] 4.2 Add deployment/runtime documentation describing the required backing store for HA auth/session continuity.
- [x] 4.3 Add backend-specific persistence-isolation regression tests covering PostgreSQL, Qdrant, and Redis cross-tenant access attempts.
