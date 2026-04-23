//! Tenant-isolation contract tests for the opal-fetcher SQL layer.
//!
//! Validates the invariant PR #130 establishes: every entity-returning
//! handler filters rows by `tenant_id = $1` at the SQL layer, not at a
//! policy layer downstream. This test exercises the EXACT SQL the
//! handlers issue (`opal-fetcher/src/handlers.rs`) against a live
//! Postgres fixture seeded with two distinct tenants and asserts zero
//! cross-tenant leakage.
//!
//! Covered invariants:
//!
//!   1. `v_hierarchy WHERE tenant_id = A` returns only tenant A's rows,
//!      even when tenant B has a company with the identical slug
//!      (post-028 uniqueness is per-tenant).
//!   2. `v_user_permissions WHERE tenant_id = A` returns only tenant
//!      A's user/role/team rows.
//!   3. `project_team_assignments WHERE tenant_id = A` isolates per-
//!      tenant assignments.
//!   4. An unknown tenant UUID returns the empty set (no fallthrough
//!      to a default/global set).
//!
//! Out of scope (tracked as follow-up to #130):
//!
//!   - Agent-permissions isolation. The `agents` table has no
//!     `tenant_id` column today; see `handlers::get_agents` docstring.
//!
//! Docker-gated; no-op with stderr notice when Docker is unavailable,
//! matching `storage/tests/migration_028_tenant_scoped_hierarchy_test.rs`.

use sqlx::Row;
use sqlx::postgres::PgPoolOptions;
use storage::migrations::apply_all;
use storage::postgres::PostgresBackend;
use testing::postgres;
use uuid::Uuid;

async fn seed_tenant(pool: &sqlx::PgPool, slug_prefix: &str) -> (Uuid, Uuid) {
    let tenant_id = Uuid::new_v4();
    let tenant_slug = format!("{slug_prefix}-tenant");

    sqlx::query(
        "INSERT INTO tenants (id, slug, name, status, source_owner)
         VALUES ($1, $2, $2, 'active', 'test')",
    )
    .bind(tenant_id)
    .bind(&tenant_slug)
    .execute(pool)
    .await
    .expect("insert tenant");

    // Same bare slug across tenants on purpose: exercises the
    // (tenant_id, slug) uniqueness from migration 028.
    let company_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO companies (id, tenant_id, slug, name)
         VALUES ($1, $2, 'acme', 'Acme')",
    )
    .bind(company_id)
    .bind(tenant_id)
    .execute(pool)
    .await
    .expect("insert company");

    let org_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO organizations (id, company_id, slug, name)
         VALUES ($1, $2, 'platform', 'Platform')",
    )
    .bind(org_id)
    .bind(company_id)
    .execute(pool)
    .await
    .expect("insert organization");

    let team_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO teams (id, org_id, slug, name)
         VALUES ($1, $2, 'api', 'API')",
    )
    .bind(team_id)
    .bind(org_id)
    .execute(pool)
    .await
    .expect("insert team");

    let user_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO users (id, email, name, status)
         VALUES ($1, $2, $3, 'active')",
    )
    .bind(user_id)
    .bind(format!("alice+{slug_prefix}@acme.com"))
    .bind(format!("Alice-{slug_prefix}"))
    .execute(pool)
    .await
    .expect("insert user");

    sqlx::query(
        "INSERT INTO memberships (user_id, team_id, role, permissions, status)
         VALUES ($1, $2, 'developer', '[]'::jsonb, 'active')",
    )
    .bind(user_id)
    .bind(team_id)
    .execute(pool)
    .await
    .expect("insert membership");

    (tenant_id, team_id)
}

async fn fresh_pool() -> Option<sqlx::PgPool> {
    let pg = postgres().await?;
    let pool = PgPoolOptions::new()
        .max_connections(3)
        .connect(pg.url())
        .await
        .expect("pool connect");
    let backend = PostgresBackend::new(pg.url())
        .await
        .expect("backend connect");
    backend
        .initialize_schema()
        .await
        .expect("initialize_schema");
    apply_all(&pool).await.expect("apply_all");
    // Hold the fixture alive by leaking it; test processes are short-lived
    // and the PgEmbed-style harness cleans up on drop at process exit.
    Box::leak(Box::new(pg));
    Some(pool)
}

#[tokio::test]
async fn v_hierarchy_tenant_filter_isolates_cross_tenant_rows() {
    let Some(pool) = fresh_pool().await else {
        eprintln!("Skipping opal-fetcher tenant isolation test: Docker unavailable");
        return;
    };

    let (tenant_a, _) = seed_tenant(&pool, "alpha").await;
    let (tenant_b, _) = seed_tenant(&pool, "beta").await;

    let rows_a: Vec<(Uuid, Option<String>)> =
        sqlx::query_as("SELECT tenant_id, company_slug FROM v_hierarchy WHERE tenant_id = $1")
            .bind(tenant_a)
            .fetch_all(&pool)
            .await
            .expect("query tenant A");

    let rows_b: Vec<(Uuid, Option<String>)> =
        sqlx::query_as("SELECT tenant_id, company_slug FROM v_hierarchy WHERE tenant_id = $1")
            .bind(tenant_b)
            .fetch_all(&pool)
            .await
            .expect("query tenant B");

    assert!(!rows_a.is_empty(), "tenant A should see its own rows");
    assert!(!rows_b.is_empty(), "tenant B should see its own rows");
    assert!(
        rows_a.iter().all(|(t, _)| *t == tenant_a),
        "tenant A query returned cross-tenant rows: {rows_a:?}",
    );
    assert!(
        rows_b.iter().all(|(t, _)| *t == tenant_b),
        "tenant B query returned cross-tenant rows: {rows_b:?}",
    );
    assert!(rows_a.iter().any(|(_, s)| s.as_deref() == Some("acme")));
    assert!(rows_b.iter().any(|(_, s)| s.as_deref() == Some("acme")));
}

#[tokio::test]
async fn v_user_permissions_tenant_filter_isolates_cross_tenant_rows() {
    let Some(pool) = fresh_pool().await else {
        eprintln!("Skipping opal-fetcher tenant isolation test: Docker unavailable");
        return;
    };

    let (tenant_a, _) = seed_tenant(&pool, "gamma").await;
    let (tenant_b, _) = seed_tenant(&pool, "delta").await;

    let emails_a: Vec<(Uuid, String)> =
        sqlx::query_as("SELECT tenant_id, email FROM v_user_permissions WHERE tenant_id = $1")
            .bind(tenant_a)
            .fetch_all(&pool)
            .await
            .expect("query tenant A users");

    let emails_b: Vec<(Uuid, String)> =
        sqlx::query_as("SELECT tenant_id, email FROM v_user_permissions WHERE tenant_id = $1")
            .bind(tenant_b)
            .fetch_all(&pool)
            .await
            .expect("query tenant B users");

    assert!(!emails_a.is_empty());
    assert!(!emails_b.is_empty());
    assert!(emails_a.iter().all(|(t, _)| *t == tenant_a));
    assert!(emails_b.iter().all(|(t, _)| *t == tenant_b));
    assert!(emails_a.iter().any(|(_, e)| e.contains("+gamma@")));
    assert!(emails_b.iter().any(|(_, e)| e.contains("+delta@")));
    assert!(emails_a.iter().all(|(_, e)| !e.contains("+delta@")));
    assert!(emails_b.iter().all(|(_, e)| !e.contains("+gamma@")));
}

#[tokio::test]
async fn project_team_assignments_tenant_filter_isolates_cross_tenant_rows() {
    let Some(pool) = fresh_pool().await else {
        eprintln!("Skipping opal-fetcher tenant isolation test: Docker unavailable");
        return;
    };

    let (tenant_a, team_a) = seed_tenant(&pool, "epsilon").await;
    let (tenant_b, team_b) = seed_tenant(&pool, "zeta").await;

    sqlx::query(
        "INSERT INTO project_team_assignments (project_id, team_id, tenant_id, assignment_type)
         VALUES ($1, $2, $3, 'primary')",
    )
    .bind("proj-a")
    .bind(team_a.to_string())
    .bind(tenant_a.to_string())
    .execute(&pool)
    .await
    .expect("insert assignment A");

    sqlx::query(
        "INSERT INTO project_team_assignments (project_id, team_id, tenant_id, assignment_type)
         VALUES ($1, $2, $3, 'primary')",
    )
    .bind("proj-b")
    .bind(team_b.to_string())
    .bind(tenant_b.to_string())
    .execute(&pool)
    .await
    .expect("insert assignment B");

    let assignments_a =
        sqlx::query("SELECT project_id FROM project_team_assignments WHERE tenant_id = $1")
            .bind(tenant_a.to_string())
            .fetch_all(&pool)
            .await
            .expect("query assignments A");

    assert_eq!(assignments_a.len(), 1);
    assert_eq!(assignments_a[0].get::<String, _>("project_id"), "proj-a");
}

#[tokio::test]
async fn v_hierarchy_unknown_tenant_returns_empty_set_not_global_fallthrough() {
    let Some(pool) = fresh_pool().await else {
        eprintln!("Skipping opal-fetcher tenant isolation test: Docker unavailable");
        return;
    };

    let _ = seed_tenant(&pool, "eta").await;
    let unknown_tenant = Uuid::new_v4();

    let rows: Vec<(Uuid,)> =
        sqlx::query_as("SELECT tenant_id FROM v_hierarchy WHERE tenant_id = $1")
            .bind(unknown_tenant)
            .fetch_all(&pool)
            .await
            .expect("query unknown tenant");

    assert!(
        rows.is_empty(),
        "unknown tenant must return empty set, got {} rows",
        rows.len()
    );
}
