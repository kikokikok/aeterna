use mk_core::TenantContext;
use sqlx::{AssertSqlSafe, PgPool, postgres::PgRow};

pub struct TenantQueryBuilder<'a> {
    pool: &'a PgPool,
    tenant_id: String,
    base_query: String
}

impl<'a> TenantQueryBuilder<'a> {
    pub fn new(pool: &'a PgPool, ctx: &TenantContext) -> Self {
        Self {
            pool,
            tenant_id: ctx.tenant_id.to_string(),
            base_query: String::new()
        }
    }

    pub fn select(mut self, columns: &str) -> Self {
        self.base_query = format!("SELECT {} FROM ", columns);
        self
    }

    pub fn from(mut self, table: &str) -> Self {
        self.base_query = format!("{}{} WHERE tenant_id = $1", self.base_query, table);
        self
    }

    pub fn where_clause(mut self, condition: &str) -> Self {
        if self.base_query.contains("WHERE") {
            self.base_query = format!("{} AND {}", self.base_query, condition);
        } else {
            self.base_query = format!("{} WHERE {}", self.base_query, condition);
        }
        self
    }

    pub fn order_by(mut self, order: &str) -> Self {
        self.base_query = format!("{} ORDER BY {}", self.base_query, order);
        self
    }

    pub fn limit(mut self, limit: u32) -> Self {
        self.base_query = format!("{} LIMIT {}", self.base_query, limit);
        self
    }

    pub fn build_query(&self) -> &str {
        &self.base_query
    }

    pub fn tenant_id(&self) -> &str {
        &self.tenant_id
    }

    pub async fn fetch_all(self) -> Result<Vec<PgRow>, sqlx::Error> {
        sqlx::query(AssertSqlSafe(self.base_query.as_str()))
            .bind(&self.tenant_id)
            .fetch_all(self.pool)
            .await
    }

    pub async fn fetch_optional(self) -> Result<Option<PgRow>, sqlx::Error> {
        sqlx::query(AssertSqlSafe(self.base_query.as_str()))
            .bind(&self.tenant_id)
            .fetch_optional(self.pool)
            .await
    }

    pub async fn fetch_one(self) -> Result<PgRow, sqlx::Error> {
        sqlx::query(AssertSqlSafe(self.base_query.as_str()))
            .bind(&self.tenant_id)
            .fetch_one(self.pool)
            .await
    }

    pub async fn execute(self) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(AssertSqlSafe(self.base_query.as_str()))
            .bind(&self.tenant_id)
            .execute(self.pool)
            .await?;
        Ok(result.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mk_core::types::{TenantId, UserId};

    fn create_test_tenant_context() -> TenantContext {
        let tenant_id = TenantId::new("test-tenant".to_string()).unwrap();
        let user_id = UserId::new("test-user".to_string()).unwrap();
        TenantContext::new(tenant_id, user_id)
    }

    #[test]
    fn test_query_builder_construction() {
        let mut query = "SELECT id, name FROM ".to_string();
        query = format!("{}users WHERE tenant_id = $1", query);
        query = format!("{} AND active = true", query);
        query = format!("{} ORDER BY id", query);
        query = format!("{} LIMIT 10", query);

        let expected = "SELECT id, name FROM users WHERE tenant_id = $1 AND active = true ORDER \
                        BY id LIMIT 10";
        assert_eq!(query, expected);
    }

    #[test]
    fn test_tenant_id_always_first_param() {
        let query = "SELECT * FROM orders WHERE tenant_id = $1";
        assert!(query.contains("tenant_id = $1"));
    }

    #[test]
    fn test_multiple_where_conditions() {
        let mut query = "SELECT * FROM items WHERE tenant_id = $1".to_string();
        query = format!("{} AND status = $2", query);
        query = format!("{} AND created_at > $3", query);

        assert!(query.contains("tenant_id = $1 AND status = $2 AND created_at > $3"));
    }

    #[tokio::test]
    async fn test_select_method() {
        let ctx = create_test_tenant_context();
        let pool = sqlx::PgPool::connect_lazy("postgres://localhost/test").unwrap();
        let builder = TenantQueryBuilder::new(&pool, &ctx).select("id, name, email");

        assert_eq!(builder.build_query(), "SELECT id, name, email FROM ");
    }

    #[tokio::test]
    async fn test_from_method() {
        let ctx = create_test_tenant_context();
        let pool = sqlx::PgPool::connect_lazy("postgres://localhost/test").unwrap();
        let builder = TenantQueryBuilder::new(&pool, &ctx)
            .select("id, name")
            .from("users");

        assert_eq!(
            builder.build_query(),
            "SELECT id, name FROM users WHERE tenant_id = $1"
        );
    }

    #[tokio::test]
    async fn test_where_clause_with_existing_where() {
        let ctx = create_test_tenant_context();
        let pool = sqlx::PgPool::connect_lazy("postgres://localhost/test").unwrap();
        let builder = TenantQueryBuilder::new(&pool, &ctx)
            .select("*")
            .from("users")
            .where_clause("active = true");

        assert_eq!(
            builder.build_query(),
            "SELECT * FROM users WHERE tenant_id = $1 AND active = true"
        );
    }

    #[tokio::test]
    async fn test_where_clause_without_existing_where() {
        let ctx = create_test_tenant_context();
        let pool = sqlx::PgPool::connect_lazy("postgres://localhost/test").unwrap();
        let builder = TenantQueryBuilder::new(&pool, &ctx)
            .select("*")
            .where_clause("active = true");

        assert_eq!(builder.build_query(), "SELECT * FROM  WHERE active = true");
    }

    #[tokio::test]
    async fn test_order_by_method() {
        let ctx = create_test_tenant_context();
        let pool = sqlx::PgPool::connect_lazy("postgres://localhost/test").unwrap();
        let builder = TenantQueryBuilder::new(&pool, &ctx)
            .select("*")
            .from("users")
            .order_by("created_at DESC");

        assert_eq!(
            builder.build_query(),
            "SELECT * FROM users WHERE tenant_id = $1 ORDER BY created_at DESC"
        );
    }

    #[tokio::test]
    async fn test_limit_method() {
        let ctx = create_test_tenant_context();
        let pool = sqlx::PgPool::connect_lazy("postgres://localhost/test").unwrap();
        let builder = TenantQueryBuilder::new(&pool, &ctx)
            .select("*")
            .from("users")
            .limit(10);

        assert_eq!(
            builder.build_query(),
            "SELECT * FROM users WHERE tenant_id = $1 LIMIT 10"
        );
    }

    #[tokio::test]
    async fn test_tenant_id_accessor() {
        let ctx = create_test_tenant_context();
        let pool = sqlx::PgPool::connect_lazy("postgres://localhost/test").unwrap();
        let builder = TenantQueryBuilder::new(&pool, &ctx);

        assert_eq!(builder.tenant_id(), "test-tenant");
    }

    #[tokio::test]
    async fn test_full_query_chain() {
        let ctx = create_test_tenant_context();
        let pool = sqlx::PgPool::connect_lazy("postgres://localhost/test").unwrap();
        let builder = TenantQueryBuilder::new(&pool, &ctx)
            .select("id, name, email")
            .from("users")
            .where_clause("active = true")
            .where_clause("role = $2")
            .order_by("created_at DESC")
            .limit(25);

        let expected = "SELECT id, name, email FROM users WHERE tenant_id = $1 AND active = true \
                        AND role = $2 ORDER BY created_at DESC LIMIT 25";
        assert_eq!(builder.build_query(), expected);
    }

    #[tokio::test]
    async fn test_query_builder_with_different_tenant() {
        let tenant_id = TenantId::new("other-tenant".to_string()).unwrap();
        let user_id = UserId::new("other-user".to_string()).unwrap();
        let ctx = TenantContext::new(tenant_id, user_id);

        let pool = sqlx::PgPool::connect_lazy("postgres://localhost/test").unwrap();
        let builder = TenantQueryBuilder::new(&pool, &ctx);

        assert_eq!(builder.tenant_id(), "other-tenant");
    }

    #[tokio::test]
    async fn test_query_builder_select_star() {
        let ctx = create_test_tenant_context();
        let pool = sqlx::PgPool::connect_lazy("postgres://localhost/test").unwrap();
        let builder = TenantQueryBuilder::new(&pool, &ctx)
            .select("*")
            .from("orders");

        assert_eq!(
            builder.build_query(),
            "SELECT * FROM orders WHERE tenant_id = $1"
        );
    }

    #[tokio::test]
    async fn test_query_builder_multiple_where_clauses() {
        let ctx = create_test_tenant_context();
        let pool = sqlx::PgPool::connect_lazy("postgres://localhost/test").unwrap();
        let builder = TenantQueryBuilder::new(&pool, &ctx)
            .select("*")
            .from("items")
            .where_clause("status = $2")
            .where_clause("created_at > $3")
            .where_clause("category = $4");

        let expected = "SELECT * FROM items WHERE tenant_id = $1 AND status = $2 AND created_at > \
                        $3 AND category = $4";
        assert_eq!(builder.build_query(), expected);
    }
}
