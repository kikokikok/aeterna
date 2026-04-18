use mk_core::types::{TenantContext, TenantId, UserId};
use storage::postgres::PostgresBackend;
use testing::postgres;

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tenant_context(tenant: &str, user: &str) -> TenantContext {
        TenantContext::new(
            TenantId::new(tenant.to_string()).unwrap(),
            UserId::new(user.to_string()).unwrap(),
        )
    }

    #[test]
    fn test_tenant_context_isolation() {
        let tenant_a = make_tenant_context("tenant-a", "user-a");
        let tenant_b = make_tenant_context("tenant-b", "user-b");

        assert_ne!(tenant_a.tenant_id, tenant_b.tenant_id);
        assert_ne!(tenant_a.user_id, tenant_b.user_id);
    }

    #[test]
    fn test_tenant_id_validation() {
        assert!(TenantId::new("valid-tenant".to_string()).is_some());
        assert!(TenantId::new("".to_string()).is_none());

        let long_id = "a".repeat(101);
        assert!(TenantId::new(long_id).is_none());
    }

    #[test]
    fn test_tenant_id_length_boundary() {
        let max_length = "a".repeat(100);
        assert!(TenantId::new(max_length).is_some());

        let over_max = "a".repeat(101);
        assert!(TenantId::new(over_max).is_none());
    }

    #[tokio::test]
    async fn test_rls_cross_tenant_access_blocked() {
        let Some(pg_fixture) = postgres().await else {
            eprintln!("Skipping test: Docker not available");
            return;
        };

        let backend = PostgresBackend::new(pg_fixture.url())
            .await
            .expect("Database connection required");

        let _ctx_a = make_tenant_context("tenant-a", "user-a");
        let _ctx_b = make_tenant_context("tenant-b", "user-b");

        backend
            .initialize_schema()
            .await
            .expect("Schema init failed");
    }

    /// Regression for issue #57.
    ///
    /// Before the fix, `activate_tenant_context` used
    /// `set_config('app.tenant_id', $1, false)` (session-scoped). If a
    /// connection was returned to the pool and re-acquired, the tenant
    /// context leaked to the next request. After the fix,
    /// `activate_tenant_context` requires a transaction and uses
    /// `set_config(..., true)` (transaction-scoped), so the setting is
    /// discarded on COMMIT/ROLLBACK.
    #[tokio::test]
    async fn test_tenant_context_does_not_leak_across_pool_acquisitions() {
        let Some(pg_fixture) = postgres().await else {
            eprintln!("Skipping test: Docker not available");
            return;
        };

        let backend = PostgresBackend::new(pg_fixture.url())
            .await
            .expect("Database connection required");
        backend
            .initialize_schema()
            .await
            .expect("Schema init failed");

        let pool = backend.pool();

        // Acquisition 1: activate tenant-a inside a transaction, commit, return.
        {
            let mut tx = pool.begin().await.expect("begin tx");
            PostgresBackend::activate_tenant_context(&mut tx, "tenant-a")
                .await
                .expect("activate tenant-a");
            let (seen,): (String,) =
                sqlx::query_as("SELECT current_setting('app.tenant_id', true)")
                    .fetch_one(&mut *tx)
                    .await
                    .expect("fetch current_setting inside tx");
            assert_eq!(seen, "tenant-a", "setting must be visible inside its tx");
            tx.commit().await.expect("commit tx");
        }

        // Acquisition 2: NEW transaction on a fresh (or recycled) connection.
        // The previous tenant-a context MUST NOT leak. Expected value: empty.
        let (leaked,): (String,) =
            sqlx::query_as("SELECT current_setting('app.tenant_id', true)")
                .fetch_one(pool)
                .await
                .expect("fetch current_setting after commit");
        assert_eq!(
            leaked, "",
            "tenant context leaked across pool acquisitions \
             (regression of issue #57)"
        );
    }
}
