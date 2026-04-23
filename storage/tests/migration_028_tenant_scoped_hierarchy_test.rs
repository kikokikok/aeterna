//! Targeted smoke test for migration
//! `028_tenant_scoped_hierarchy.sql` (PR #129, §2.2-B).
//!
//! Covered invariants:
//!
//!   1. `companies.tenant_id` exists, is UUID, is NOT NULL, and has a FK
//!      to `tenants(id)` with ON DELETE CASCADE.
//!   2. The pre-028 `companies_slug_key` (global UNIQUE on slug) is gone.
//!   3. `companies_tenant_slug_key` UNIQUE (tenant_id, slug) is present.
//!   4. Bootstrap pattern works: inserting two companies with the same
//!      `slug` under two different tenants succeeds; inserting two with
//!      the same `(tenant_id, slug)` fails.
//!   5. `v_hierarchy` and `v_user_permissions` both surface `tenant_id`
//!      as a column.
//!   6. ON DELETE CASCADE from tenant works: dropping a tenant removes
//!      its companies.
//!
//! Docker-gated; no-op with stderr notice when Docker is unavailable,
//! matching storage/tests/rls_enforcement_test.rs convention.

use sqlx::Row;
use sqlx::postgres::PgPoolOptions;
use storage::migrations::apply_all;
use storage::postgres::PostgresBackend;
use testing::postgres;
use uuid::Uuid;

#[tokio::test]
async fn migration_028_establishes_tenant_scoped_companies() {
    let Some(pg) = postgres().await else {
        eprintln!("Skipping migration 028 smoke test: Docker not available");
        return;
    };

    // Bring a fresh schema up to current HEAD. `testing::postgres()`
    // already does initialize_schema + apply_all once at fixture init,
    // but we re-connect with our own pool to run the introspection queries.
    let pool = PgPoolOptions::new()
        .max_connections(3)
        .connect(pg.url())
        .await
        .expect("connect to fixture");

    // Ensure schema is in fact at HEAD for this connection's DB
    // (idempotent call — the fixture already ran these).
    let backend = PostgresBackend::new(pg.url())
        .await
        .expect("backend connect");
    backend
        .initialize_schema()
        .await
        .expect("initialize_schema idempotent");
    apply_all(&pool).await.expect("apply_all idempotent");

    // ---- Invariant 1: companies.tenant_id shape ----
    let row = sqlx::query(
        "SELECT data_type, is_nullable
           FROM information_schema.columns
          WHERE table_name = 'companies' AND column_name = 'tenant_id'",
    )
    .fetch_one(&pool)
    .await
    .expect("companies.tenant_id must exist");
    let data_type: String = row.get("data_type");
    let is_nullable: String = row.get("is_nullable");
    assert_eq!(data_type, "uuid", "companies.tenant_id must be uuid");
    assert_eq!(
        is_nullable, "NO",
        "companies.tenant_id must be NOT NULL after migration 028"
    );

    // FK + ON DELETE CASCADE
    let fk_action: String = sqlx::query_scalar(
        "SELECT confdeltype::text FROM pg_constraint
          WHERE conrelid = 'companies'::regclass
            AND confrelid = 'tenants'::regclass
            AND contype = 'f'
          LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .expect("companies -> tenants FK must exist");
    assert_eq!(fk_action, "c", "FK must be ON DELETE CASCADE ('c')");

    // ---- Invariant 2: old companies_slug_key is gone ----
    let old_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS (
             SELECT 1 FROM pg_constraint
              WHERE conname = 'companies_slug_key'
                AND conrelid = 'companies'::regclass
         )",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        !old_exists,
        "companies_slug_key (pre-028 global UNIQUE on slug) must be dropped"
    );

    // ---- Invariant 3: new companies_tenant_slug_key is present ----
    let new_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS (
             SELECT 1 FROM pg_constraint
              WHERE conname = 'companies_tenant_slug_key'
                AND conrelid = 'companies'::regclass
                AND contype = 'u'
         )",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        new_exists,
        "companies_tenant_slug_key UNIQUE (tenant_id, slug) must exist"
    );

    // ---- Invariant 4: per-tenant slug collision rules ----
    // Unique scenario tags so parallel test runs don't stomp each other.
    let tag = format!("m028-{}", Uuid::new_v4().simple());
    let tenant_a_slug = format!("{tag}-a");
    let tenant_b_slug = format!("{tag}-b");

    let tenant_a: Uuid =
        sqlx::query_scalar("INSERT INTO tenants (slug, name) VALUES ($1, $1) RETURNING id")
            .bind(&tenant_a_slug)
            .fetch_one(&pool)
            .await
            .expect("insert tenant A");
    let tenant_b: Uuid =
        sqlx::query_scalar("INSERT INTO tenants (slug, name) VALUES ($1, $1) RETURNING id")
            .bind(&tenant_b_slug)
            .fetch_one(&pool)
            .await
            .expect("insert tenant B");

    // Same slug under two different tenants should now succeed.
    sqlx::query(
        "INSERT INTO companies (tenant_id, slug, name, settings) VALUES ($1, 'shared', 'A', '{}')",
    )
    .bind(tenant_a)
    .execute(&pool)
    .await
    .expect("tenant A company with slug 'shared' must succeed");
    sqlx::query(
        "INSERT INTO companies (tenant_id, slug, name, settings) VALUES ($1, 'shared', 'B', '{}')",
    )
    .bind(tenant_b)
    .execute(&pool)
    .await
    .expect("tenant B company with slug 'shared' must also succeed");

    // Same (tenant_id, slug) must fail.
    let conflict = sqlx::query(
        "INSERT INTO companies (tenant_id, slug, name, settings) VALUES ($1, 'shared', 'dup', '{}')",
    )
    .bind(tenant_a)
    .execute(&pool)
    .await;
    assert!(
        conflict.is_err(),
        "duplicate (tenant_id, slug) must violate companies_tenant_slug_key"
    );

    // ---- Invariant 5: views surface tenant_id ----
    for view in ["v_hierarchy", "v_user_permissions"] {
        let has_tenant_id: bool = sqlx::query_scalar(
            "SELECT EXISTS (
                 SELECT 1 FROM information_schema.columns
                  WHERE table_name = $1 AND column_name = 'tenant_id'
             )",
        )
        .bind(view)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(
            has_tenant_id,
            "{view} must expose tenant_id after migration 028"
        );
    }

    // ---- Invariant 6: ON DELETE CASCADE from tenant ----
    sqlx::query("DELETE FROM tenants WHERE id = $1")
        .bind(tenant_a)
        .execute(&pool)
        .await
        .expect("delete tenant A");
    let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM companies WHERE tenant_id = $1")
        .bind(tenant_a)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        remaining, 0,
        "deleting tenant must cascade-delete its companies"
    );

    // Clean up tenant_b side so parallel runs don't accumulate.
    sqlx::query("DELETE FROM tenants WHERE id = $1")
        .bind(tenant_b)
        .execute(&pool)
        .await
        .ok();

    pool.close().await;
}
