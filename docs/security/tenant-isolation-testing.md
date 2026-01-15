# Tenant Isolation Penetration Testing

## Overview

This document describes procedures for testing tenant data isolation in Aeterna's multi-tenant architecture.

## Test Categories

### 1. Cross-Tenant Access Tests

Location: `storage/tests/tenant_isolation_test.rs`

| Test | Description | Attack Vector |
|------|-------------|---------------|
| `test_tenant_isolation_cross_tenant_node_access` | Verify tenant B cannot see tenant A's nodes | Direct access attempt |
| `test_tenant_isolation_cross_tenant_edge_creation_blocked` | Verify edges cannot span tenants | Relationship bypass |
| `test_tenant_isolation_find_related_cross_tenant` | Verify graph traversal respects boundaries | Transitive access |
| `test_tenant_isolation_stats_per_tenant` | Verify stats scoped to tenant | Information disclosure |

### 2. SQL Injection Tests

Location: `storage/tests/tenant_isolation_test.rs`

| Test | Description | Payload |
|------|-------------|---------|
| `test_tenant_id_validation_sql_injection_single_quote` | Single quote injection | `tenant'; DROP TABLE`|
| `test_tenant_id_validation_sql_injection_double_dash` | Comment injection | `tenant--comment` |
| `test_tenant_id_validation_sql_injection_union` | UNION SELECT injection | `tenantUNIONSELECT` |
| `test_tenant_id_validation_sql_injection_semicolon` | Statement termination | `tenant;DELETE` |

### 3. Tenant ID Validation Tests

Location: `storage/tests/tenant_isolation_test.rs`

| Test | Description | Validation |
|------|-------------|------------|
| `test_tenant_id_validation_empty` | Empty string rejected | Length validation |
| `test_tenant_id_validation_too_long` | >100 chars rejected | Max length |
| `test_tenant_id_validation_valid_patterns` | Valid IDs accepted | Allowlist patterns |

### 4. RLS Policy Tests

Location: `storage/tests/rls_policy_test.rs`

| Test | Description | Target |
|------|-------------|--------|
| `test_tenant_context_isolation` | Contexts are independent | TenantContext |
| `test_rls_cross_tenant_access_blocked` | Database-level RLS | PostgreSQL policies |

## Running Tests

```bash
cargo test -p storage --test tenant_isolation_test
cargo test -p storage --test rls_policy_test

cargo test -p storage -- --ignored
```

## Test Result Format

Pass criteria:
- All cross-tenant access attempts return empty results or errors
- All SQL injection attempts are rejected at TenantId validation
- All tenant ID boundary conditions are enforced

Failure indicators:
- `TenantViolation` error not returned on cross-tenant access
- SQL injection payload reaches database layer
- Data leak across tenant boundaries

## Protected Tables

RLS policies are applied to:

| Table | Policy Name |
|-------|-------------|
| `sync_states` | `sync_states_tenant_isolation` |
| `memory_entries` | `memory_entries_tenant_isolation` |
| `knowledge_items` | `knowledge_items_tenant_isolation` |

## Query Builder Protection

All SQL queries MUST use `TenantQueryBuilder` from `storage/src/query_builder.rs`:

```rust
let builder = TenantQueryBuilder::new(&pool, &tenant_context);
let rows = builder
    .select("*")
    .from("memory_entries")
    .fetch_all()
    .await?;
```

The builder automatically:
1. Requires `TenantContext` at construction
2. Adds `tenant_id = $1` to all WHERE clauses
3. Binds tenant_id as first parameter

## Adding New Tests

When adding tenant-scoped tables:

1. Add RLS policy in `storage/migrations/004_enable_rls.sql`
2. Add table to `TENANT_TABLES` in `storage/src/rls_migration.rs`
3. Add cross-tenant access test in `tenant_isolation_test.rs`
4. Verify `TenantQueryBuilder` is used for all queries

## CI Integration

Tenant isolation tests run in CI via `.github/workflows/tenant-isolation.yml`:
- Triggered on all PRs modifying `storage/` or `mk_core/`
- Requires all tests pass before merge
- Runs against PostgreSQL 16 with RLS enabled
