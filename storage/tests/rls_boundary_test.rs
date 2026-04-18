//! #44.d §4.2 — RLS boundary regression guard.
//!
//! The cross-tenant listing feature (#44.d) depends on PlatformAdmin code
//! paths being able to read ACROSS tenant boundaries via a direct
//! `SELECT ... FROM table` with no implicit tenant filter. This works today
//! because none of the affected tables have PostgreSQL Row-Level Security
//! (RLS) enabled.
//!
//! If a future migration silently RLS-enables one of these tables, the
//! cross-tenant listing endpoints would return empty results for
//! PlatformAdmins — and tests would still pass because they'd just see
//! empty `items[]` arrays (vacuously satisfying the §4.1 contract).
//!
//! This test fails loudly if that ever happens, forcing a redesign of the
//! cross-tenant reader (options then would be: policy that whitelists
//! PlatformAdmin session vars, or switching to a service-role connection).
//!
//! Tables covered (the exact set the #44.d cross-tenant readers touch):
//!
//!   - `tenants`                 → /admin/tenants (§2.1)
//!   - `users`                   → /user         (§2.2)
//!   - `organizational_units`    → /project · /org (§2.3 · §2.4)
//!   - `governance_audit_log`    → /govern/audit (§2.5)
//!   - `referential_audit_log`   → companion audit stream, queried by the
//!                                 same reader path in future PRs
//!
//! This list MUST stay in sync with the actual cross-tenant readers. If a
//! new endpoint adds another table to the cross-tenant-readable set, add it
//! here. If a table is removed from the cross-tenant set, also remove it
//! here (stale entries would false-positive if the table is later legitimately
//! RLS-gated for tenant-scoped reads).

use sqlx::Row;
use testing::postgres;

/// Every table the #44.d cross-tenant reader path touches.
///
/// If any of these has `relrowsecurity = true` in `pg_class`, the
/// cross-tenant listing endpoints are silently broken for PlatformAdmins
/// and this test fails.
const CROSS_TENANT_READABLE_TABLES: &[&str] = &[
    "tenants",
    "users",
    "organizational_units",
    "governance_audit_log",
    "referential_audit_log",
];

#[tokio::test]
async fn cross_tenant_readable_tables_have_rls_disabled() {
    let Some(pg_fixture) = postgres().await else {
        eprintln!("Skipping RLS boundary test: Docker not available");
        return;
    };

    // Initialize schema so all tables exist (migrations run on connect).
    use storage::postgres::PostgresBackend;
    let backend = PostgresBackend::new(pg_fixture.url())
        .await
        .expect("connect to test postgres");
    backend
        .initialize_schema()
        .await
        .expect("run schema migrations");

    let pool = sqlx::PgPool::connect(pg_fixture.url())
        .await
        .expect("pool for RLS query");

    let mut violations = Vec::<(String, bool, bool)>::new();
    for table in CROSS_TENANT_READABLE_TABLES {
        // Query pg_class for the exact RLS flags of this table in the
        // public schema. `relrowsecurity` = RLS enabled at all.
        // `relforcerowsecurity` = RLS bypass disabled even for table
        // owner (neither can be true for the cross-tenant reader to work).
        let row = sqlx::query(
            r#"
            SELECT c.relrowsecurity, c.relforcerowsecurity
            FROM pg_class c
            JOIN pg_namespace n ON n.oid = c.relnamespace
            WHERE n.nspname = 'public' AND c.relname = $1
            "#,
        )
        .bind(table)
        .fetch_optional(&pool)
        .await
        .unwrap_or_else(|e| panic!("pg_class lookup failed for {table}: {e}"));

        let row = row.unwrap_or_else(|| {
            panic!(
                "Table `{table}` is declared cross-tenant-readable in \
                 CROSS_TENANT_READABLE_TABLES but does not exist in the schema. \
                 Either a migration is missing or the constant is out of date."
            )
        });
        let rls_enabled: bool = row.get(0);
        let rls_forced: bool = row.get(1);
        if rls_enabled || rls_forced {
            violations.push((table.to_string(), rls_enabled, rls_forced));
        }
    }

    assert!(
        violations.is_empty(),
        "#44.d RLS boundary violation: the following cross-tenant-readable \
         tables have Row-Level Security enabled, which silently breaks \
         PlatformAdmin cross-tenant listing endpoints (/admin/tenants, /user, \
         /project, /org, /govern/audit):\n{:#?}\n\n\
         Resolution options:\n\
           1. If the RLS was added by accident, disable it: \
              `ALTER TABLE <t> DISABLE ROW LEVEL SECURITY;`\n\
           2. If RLS is intentional, add a policy that whitelists the \
              cross-tenant reader (e.g. `USING (true)` when a \
              `app.is_platform_admin` session var is set), or switch \
              the cross-tenant readers to a service role connection.\n\
           3. If the table should no longer be cross-tenant-readable, \
              remove it from `CROSS_TENANT_READABLE_TABLES` AND from \
              the cross-tenant endpoints' SELECTs.",
        violations
    );
}
