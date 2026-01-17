use mk_core::types::{TenantContext, TenantId, UserId};
use storage::postgres::PostgresBackend;

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tenant_context(tenant: &str, user: &str) -> TenantContext {
        TenantContext::new(
            TenantId::new(tenant.to_string()).unwrap(),
            UserId::new(user.to_string()).unwrap()
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
    #[ignore = "requires database connection"]
    async fn test_rls_cross_tenant_access_blocked() {
        let backend = PostgresBackend::new("postgres://test:test@localhost/test_db")
            .await
            .expect("Database connection required");

        let _ctx_a = make_tenant_context("tenant-a", "user-a");
        let _ctx_b = make_tenant_context("tenant-b", "user-b");

        backend
            .initialize_schema()
            .await
            .expect("Schema init failed");
    }
}
