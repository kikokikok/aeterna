## MODIFIED Requirements

### Requirement: Backend-Specific Persistence Isolation
The system SHALL define and enforce tenant isolation explicitly for each persistence backend used in production-capable deployments. On PostgreSQL, the authoritative enforcement layer is the application's repository code; row-level security policies exist as an authored, CI-enforced artifact rather than a production runtime control.

#### Scenario: PostgreSQL tenant isolation is enforced at the application layer
- **WHEN** PostgreSQL stores tenant-scoped data
- **THEN** every repository-layer query against a tenant-scoped table SHALL include an explicit `WHERE tenant_id = $N` clause (or equivalent `company_id` clause for governance tables)
- **AND** the production database role MAY have `BYPASSRLS` or be the table owner without triggering an isolation violation
- **AND** documentation (`AGENTS.md`, `DEVELOPER_GUIDE.md`) SHALL state that the application layer is the authoritative enforcement point on PostgreSQL

#### Scenario: RLS policies are authored and kept valid
- **WHEN** a migration adds a tenant-scoped table to PostgreSQL
- **THEN** that migration SHALL `ENABLE ROW LEVEL SECURITY` on the table
- **AND** SHALL define at least one policy keyed on the canonical session variable `app.tenant_id` (or `app.company_id` for governance-scoped tables)
- **AND** the policies SHALL remain valid under the CI enforcement suite defined in "RLS Test-Time Enforcement"

#### Scenario: Qdrant tenant isolation is enforced by storage routing
- **WHEN** Qdrant stores tenant-scoped vectors or payloads
- **THEN** the storage layer SHALL route operations through tenant-scoped collections, mandatory tenant filters, or both
- **AND** it SHALL reject or prevent queries that could return vectors from another tenant

#### Scenario: Redis tenant isolation is enforced by storage namespaces
- **WHEN** Redis stores tenant-scoped working memory, session data, checkpoints, streams, or caches
- **THEN** the storage layer SHALL use tenant-scoped key namespaces and access wrappers for those keys
- **AND** callers SHALL NOT read or mutate another tenant's Redis data without an explicit authorized tenant context

## ADDED Requirements

### Requirement: RLS Test-Time Enforcement
The system SHALL maintain a dedicated non-`BYPASSRLS` PostgreSQL role used exclusively by the integration test suite, and SHALL exercise every RLS-enabled table under that role on every CI run. This converts the row-level security policies from a decorative production artifact into a CI-enforced guard against repository-layer queries missing their `WHERE tenant_id = ?` clause.

#### Scenario: Dedicated non-BYPASSRLS role exists
- **WHEN** the migration suite is applied against a fresh database
- **THEN** a role named `aeterna_app_rls` SHALL exist
- **AND** the role SHALL have `LOGIN` but SHALL NOT have `BYPASSRLS` or `SUPERUSER`
- **AND** the role SHALL hold `USAGE` on the application schema and DML (`SELECT`, `INSERT`, `UPDATE`, `DELETE`) on every table that has `ENABLE ROW LEVEL SECURITY`

#### Scenario: RLS isolation is verified end-to-end per table
- **WHEN** the integration test suite runs
- **THEN** for every table with `ENABLE ROW LEVEL SECURITY`, there SHALL be a test that connects as `aeterna_app_rls`, begins a transaction, seeds rows for two distinct tenants, sets `app.tenant_id` to one tenant via `set_config(..., true)`, and asserts that a `SELECT *` returns only that tenant's rows
- **AND** for every such table there SHALL be a test asserting that without setting `app.tenant_id`, the same `SELECT *` returns zero rows

#### Scenario: Schema introspection guards role grants
- **WHEN** the integration test suite starts
- **THEN** a pre-flight query SHALL enumerate every table in `pg_tables` where `rowsecurity = true`
- **AND** SHALL assert that `aeterna_app_rls` has been granted at minimum `SELECT` on each such table
- **AND** SHALL fail the run with a message naming the missing table if any grant is absent, so that adding a new RLS-protected table without updating the test grant fails CI

#### Scenario: Handler-level list paths are exercised under RLS
- **WHEN** the integration test suite runs
- **THEN** the `/user`, `/project`, `/org`, and `/govern/audit` list endpoints SHALL be exercised against a test server whose pool connects as `aeterna_app_rls`
- **AND** each list SHALL return only the authenticated tenant's rows
- **AND** attempting a request with no resolved tenant context SHALL either return `400 select_tenant` (at the middleware layer) or an empty list (at the RLS layer) — never rows from another tenant

### Requirement: Tenant Session Variable Hygiene
Every code path that writes the `app.tenant_id` (or `app.company_id`) PostgreSQL session variable SHALL use a transaction-scoped write so that the setting cannot survive on a pooled connection after the originating query completes. This guards against a pooled connection carrying a prior request's tenant context into a subsequent request.

#### Scenario: set_config uses transaction scope
- **WHEN** the application calls `set_config('app.tenant_id', $1, ?)` (or the equivalent `SET` / `SET LOCAL` statement)
- **THEN** the call SHALL pass `true` as the third argument (transaction-local) and SHALL be inside an explicit `BEGIN`
- **AND** the application SHALL NOT rely on session-scoped `set_config(..., false)` for tenant context, so that returning a connection to the pool cannot leak tenant state

#### Scenario: Test suite catches session-scope misuse
- **WHEN** a new query is introduced that writes `app.tenant_id` with session scope
- **THEN** a dedicated `cargo test` target SHALL fail by detecting a pooled-connection leak: it acquires a connection, sets context to tenant A, returns the connection to the pool, acquires a fresh connection, and asserts that `current_setting('app.tenant_id', true)` returns empty rather than tenant A's identifier
