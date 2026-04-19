//! Verifies the BYPASSRLS admin pool bypasses RLS and that
//! `with_admin_context` records an audit row with `admin_scope = TRUE`.
//!
//! (Bundle A.2, task 3.3.5 — simplified scope: direct backend exercise
//! rather than full test-server wiring. Handler-level admin-surface
//! coverage lands alongside the A.3 call-site refactor where the
//! handlers themselves switch to `with_admin_context`.)

use mk_core::types::{TenantContext, TenantId, UserId};
use sqlx::Row;
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;
use storage::migrations::apply_all;
use storage::postgres::PostgresBackend;
use testing::postgres;

#[tokio::test]
async fn admin_pool_bypasses_rls_and_audits() {
    let Some(pg) = postgres().await else {
        eprintln!("Skipping admin surface test: Docker not available");
        return;
    };

    // Setup: migrations + seed two tenants' rows in sync_state.
    let super_pool = PgPoolOptions::new()
        .max_connections(3)
        .connect(pg.url())
        .await
        .expect("super pool");
    apply_all(&super_pool).await.expect("migrations");

    sqlx::query(
        "INSERT INTO sync_state (id, tenant_id, data, updated_at) \
         VALUES ('admin-a', 'admin-t-a', '{}'::jsonb, 0), \
                ('admin-b', 'admin-t-b', '{}'::jsonb, 0) \
         ON CONFLICT DO NOTHING",
    )
    .execute(&super_pool)
    .await
    .expect("seed");

    // Build a backend where the admin pool == super pool (BYPASSRLS).
    // The tenant pool is also super here since we're not testing that leg.
    let tenant_pool = PgPoolOptions::new()
        .max_connections(3)
        .acquire_timeout(Duration::from_secs(10))
        .connect(pg.url())
        .await
        .expect("tenant pool");
    let admin_pool = PgPoolOptions::new()
        .max_connections(2)
        .acquire_timeout(Duration::from_secs(10))
        .connect(pg.url())
        .await
        .expect("admin pool");
    let backend = PostgresBackend::from_pools(tenant_pool, admin_pool);

    // PlatformAdmin context, no target tenant ⇒ cross-tenant-all.
    let ctx = TenantContext::new(
        TenantId::new("__root__".to_string()).unwrap(),
        UserId::new("pa-user".to_string()).unwrap(),
    );

    // Read both tenants' rows through with_admin_context.
    let total: i64 = backend
        .with_admin_context(&ctx, "admin.sync_state.list", |tx| {
            Box::pin(async move {
                let row = sqlx::query(
                    "SELECT COUNT(*) AS c FROM sync_state \
                     WHERE id IN ('admin-a', 'admin-b')",
                )
                .fetch_one(&mut **tx)
                .await?;
                Ok::<i64, storage::postgres::PostgresError>(row.get("c"))
            })
        })
        .await
        .expect("with_admin_context");

    assert_eq!(
        total, 2,
        "admin pool did not see both tenants' rows — BYPASSRLS is not in effect"
    );

    // Audit row must have landed, with admin_scope = TRUE.
    let audits: i64 = sqlx::query(
        "SELECT COUNT(*) AS c FROM governance_audit_log \
         WHERE admin_scope = TRUE AND action = 'admin.sync_state.list'",
    )
    .fetch_one(&super_pool)
    .await
    .expect("audit count")
    .get("c");
    assert_eq!(
        audits, 1,
        "with_admin_context did not record an admin_scope=TRUE audit row"
    );
}
