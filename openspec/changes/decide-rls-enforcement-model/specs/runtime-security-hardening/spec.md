## MODIFIED Requirements

### Requirement: Backend-Specific Persistence Isolation
The system SHALL define and enforce tenant isolation explicitly for each persistence backend used in production-capable deployments. On PostgreSQL, the authoritative enforcement layer is PostgreSQL row-level security policies evaluated against a transaction-scoped `app.tenant_id` GUC; the repository layer's explicit `WHERE tenant_id = $N` clauses remain as a required second layer of defense in depth.

#### Scenario: PostgreSQL tenant isolation is enforced by row-level security
- **WHEN** PostgreSQL stores tenant-scoped data
- **THEN** the production application database role SHALL NOT have `BYPASSRLS` and SHALL NOT be the table owner of any tenant-scoped table
- **AND** every tenant-scoped table SHALL have `ENABLE ROW LEVEL SECURITY` with at least one policy keyed on `current_setting('app.tenant_id', true)`
- **AND** every request-scoped query path SHALL acquire its database connection through the `with_tenant_context` helper (or equivalent) that opens an explicit transaction, issues `SET LOCAL app.tenant_id = $1`, runs the query body, and commits or rolls back
- **AND** every repository-layer query against a tenant-scoped table SHALL additionally include an explicit `WHERE tenant_id = $N` clause (or equivalent `company_id` clause for governance tables) as defense in depth on top of the RLS policy
- **AND** documentation (`AGENTS.md`, `DEVELOPER_GUIDE.md`) SHALL state the two-layer model: RLS is authoritative, the `WHERE` clause is required defense in depth, neither is optional

#### Scenario: RLS policies are authored and kept valid
- **WHEN** a migration adds a tenant-scoped table to PostgreSQL
- **THEN** that migration SHALL `ENABLE ROW LEVEL SECURITY` on the table
- **AND** SHALL define at least one policy keyed on the canonical session variable `app.tenant_id` (or `app.company_id` for governance-scoped tables)
- **AND** SHALL grant `SELECT, INSERT, UPDATE, DELETE` on the table to the `aeterna_app` application role
- **AND** the policies SHALL remain valid under the CI enforcement suite defined in "RLS End-to-End Verification"

#### Scenario: Qdrant tenant isolation is enforced by storage routing
- **WHEN** Qdrant stores tenant-scoped vectors or payloads
- **THEN** the storage layer SHALL route operations through tenant-scoped collections, mandatory tenant filters, or both
- **AND** it SHALL reject or prevent queries that could return vectors from another tenant

#### Scenario: Redis tenant isolation is enforced by storage namespaces
- **WHEN** Redis stores tenant-scoped working memory, session data, checkpoints, streams, or caches
- **THEN** the storage layer SHALL use tenant-scoped key namespaces and access wrappers for those keys
- **AND** callers SHALL NOT read or mutate another tenant's Redis data without an explicit authorized tenant context

## ADDED Requirements

### Requirement: Non-BYPASSRLS Application Role in Production
The system SHALL provision a dedicated PostgreSQL role (`aeterna_app`) that does NOT have `BYPASSRLS` and SHALL use this role for all production request-scoped database connections. The legacy BYPASSRLS role remains available only behind a feature flag for emergency rollback.

#### Scenario: Production pool opens connections as aeterna_app
- **WHEN** the application starts in production
- **THEN** `AETERNA_DB_ROLE` SHALL resolve to `rls` (the default after Bundle A.4's rollout soak)
- **AND** the connection pool SHALL authenticate as `aeterna_app` using the password supplied via `APP_DB_PASSWORD`
- **AND** the startup log SHALL emit `db_role=rls` at `INFO` level so the effective role is visible in post-incident review

#### Scenario: aeterna_app lacks BYPASSRLS
- **WHEN** the migration suite is applied against a fresh database
- **THEN** the role `aeterna_app` SHALL exist
- **AND** `pg_roles.rolbypassrls` SHALL be `false` for `aeterna_app`
- **AND** `pg_roles.rolsuper` SHALL be `false` for `aeterna_app`
- **AND** `aeterna_app` SHALL hold `USAGE` on the application schema and `SELECT, INSERT, UPDATE, DELETE` on every table that has `ENABLE ROW LEVEL SECURITY`

#### Scenario: Rollback escape hatch is operationally documented
- **WHEN** an operator needs to revert to the legacy BYPASSRLS role
- **THEN** setting `AETERNA_DB_ROLE=bypassrls` and performing a rolling pool restart SHALL be sufficient to switch the effective role
- **AND** the rollback SHALL complete in under 5 minutes per region per `docs/ops/rls_rollback.md`
- **AND** the bypass path SHALL be removed in Bundle A.5 after a minimum 4-week prod soak under `rls`

### Requirement: Per-Request Transaction-Scoped Tenant Context
Every code path that reads from or writes to a tenant-scoped PostgreSQL table SHALL acquire its connection through a helper that opens an explicit transaction, sets `app.tenant_id` transaction-locally, and commits or rolls back before returning the connection to the pool. Direct `pool.acquire()` or `pool.begin()` on tenant-scoped paths SHALL be forbidden.

#### Scenario: Request handlers use with_tenant_context
- **WHEN** a request handler queries a tenant-scoped table
- **THEN** the handler SHALL call `with_tenant_context(&ctx, |tx| async { … })` (or an equivalent async-worker-scoped helper)
- **AND** the helper SHALL issue `BEGIN`, `SET LOCAL app.tenant_id = $1`, the handler body, and `COMMIT`/`ROLLBACK` in that order
- **AND** the handler SHALL NOT call `self.pool.acquire()` or `self.pool.begin()` directly on a tenant-scoped path

#### Scenario: Async workers carry their own tenant context
- **WHEN** an async worker (sync, webhook, backup, GDPR) processes a job that touches tenant-scoped tables
- **THEN** the worker SHALL read the tenant identifier from its job record
- **AND** SHALL construct a `TenantContext` from that identifier
- **AND** SHALL acquire its connection through `with_tenant_context` using that context
- **AND** a job that touches tenant-scoped tables without an associated tenant identifier SHALL fail fast rather than running unscoped

#### Scenario: Direct pool access is lint-enforced
- **WHEN** a contributor introduces a call to `self.pool.acquire()` or `self.pool.begin()` outside of `with_tenant_context`
- **THEN** the CI `deny_lint` SHALL fail the build unless the call site carries an `#[allow(direct_pool_access)]` attribute with a justification comment documenting why the path is tenant-context-free (e.g., health check, schema migration)

### Requirement: Tenant Session Variable Hygiene
Every code path that writes the `app.tenant_id` PostgreSQL session variable SHALL use transaction-scoped writes so that the setting cannot survive on a pooled connection after the originating request completes. The legacy `app.company_id` and `app.current_tenant_id` GUC names SHALL NOT be written by application code; migration 024 normalized the policies to `app.tenant_id` and Bundle A.1 normalizes the app-side.

#### Scenario: set_config uses transaction scope
- **WHEN** the application calls `set_config('app.tenant_id', $1, ?)` (or the equivalent `SET LOCAL` statement)
- **THEN** the call SHALL pass `true` as the third argument (transaction-local) and SHALL be inside an explicit `BEGIN`
- **AND** the application SHALL NOT pass `false` (session-scoped) anywhere outside of an explicitly-allowlisted diagnostic path

#### Scenario: Canonical GUC namespace is enforced
- **WHEN** application code reads or writes a tenant session variable
- **THEN** it SHALL use the name `app.tenant_id` exclusively
- **AND** SHALL NOT reference `app.company_id`, `app.current_tenant_id`, or `app.current_company_id` outside of migration files
- **AND** a compile-time grep test SHALL fail the build if a new reference to a legacy GUC name is introduced in non-migration source

#### Scenario: Test suite catches session-scope misuse
- **WHEN** a new query is introduced that writes `app.tenant_id` with session scope
- **THEN** the `session_variable_hygiene` test SHALL fail by detecting a pooled-connection leak: it acquires a connection, sets context to tenant A, returns the connection to the pool, acquires a fresh connection, and asserts that `current_setting('app.tenant_id', true)` returns empty rather than tenant A's identifier

### Requirement: RLS End-to-End Verification
The system SHALL maintain an integration test suite that exercises every RLS-enabled PostgreSQL table under the non-BYPASSRLS `aeterna_app` role on every CI run. This suite is the permanent regression guard that prevents a future refactor from silently regressing tenant isolation, and is the source of truth for whether the `with_tenant_context` helper correctly scopes every code path.

#### Scenario: Grant pre-flight enumerates RLS tables
- **WHEN** the integration test suite starts
- **THEN** a pre-flight query SHALL enumerate every table in `pg_tables` where `rowsecurity = true`
- **AND** SHALL assert that `aeterna_app` has been granted at minimum `SELECT` on each such table
- **AND** SHALL fail the run with a message naming the missing table if any grant is absent, so that adding a new RLS-protected table without updating the grant fails CI

#### Scenario: RLS isolation is verified per table
- **WHEN** the integration test suite runs
- **THEN** for every table with `ENABLE ROW LEVEL SECURITY`, there SHALL be a test that connects as `aeterna_app`, begins a transaction, seeds rows for two distinct tenants, sets `app.tenant_id` to one tenant via `set_config(..., true)`, and asserts that a `SELECT *` returns only that tenant's rows
- **AND** for every such table there SHALL be a test asserting that without setting `app.tenant_id`, the same `SELECT *` returns zero rows
- **AND** for every such table there SHALL be a test asserting that a query for rows belonging to a different tenant (by explicit `WHERE id = $foreign_id`) returns zero rows

#### Scenario: Handler-level list paths are exercised under RLS
- **WHEN** the integration test suite runs
- **THEN** the `/user`, `/project`, `/org`, and `/govern/audit` list endpoints SHALL be exercised against a test server whose pool connects as `aeterna_app`
- **AND** each list SHALL return only the authenticated tenant's rows
- **AND** attempting a request with no resolved tenant context SHALL either return `400 select_tenant` (at the middleware layer) or an empty list (at the RLS layer) — never rows from another tenant
