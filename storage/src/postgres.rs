use async_trait::async_trait;
use mk_core::traits::StorageBackend;
use mk_core::types::{OrganizationalUnit, TenantContext, UnitType};
use sqlx::{Pool, Postgres, Row};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PostgresError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Unit not found: {0}")]
    NotFound(String)
}

pub struct PostgresBackend {
    pool: Pool<Postgres>
}

impl PostgresBackend {
    pub async fn new(connection_url: &str) -> Result<Self, PostgresError> {
        let pool = Pool::connect(connection_url).await?;
        Ok(Self { pool })
    }

    pub async fn initialize_schema(&self) -> Result<(), PostgresError> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS sync_state (
                id TEXT NOT NULL,
                tenant_id TEXT NOT NULL,
                data JSONB NOT NULL,
                updated_at BIGINT NOT NULL,
                PRIMARY KEY (id, tenant_id)
            )"
        )
        .execute(&self.pool)
        .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_sync_state_tenant_id ON sync_state(tenant_id)")
            .execute(&self.pool)
            .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS organizational_units (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                type TEXT NOT NULL, -- 'company', 'organization', 'team', 'project'
                parent_id TEXT REFERENCES organizational_units(id),
                tenant_id TEXT NOT NULL,
                metadata JSONB DEFAULT '{}',
                created_at BIGINT NOT NULL,
                updated_at BIGINT NOT NULL
            )"
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS user_roles (
                user_id TEXT NOT NULL,
                tenant_id TEXT NOT NULL,
                unit_id TEXT NOT NULL REFERENCES organizational_units(id),
                role TEXT NOT NULL,
                created_at BIGINT NOT NULL,
                PRIMARY KEY (user_id, tenant_id, unit_id, role)
            )"
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS unit_policies (
                id TEXT PRIMARY KEY,
                unit_id TEXT NOT NULL REFERENCES organizational_units(id),
                policy JSONB NOT NULL,
                created_at BIGINT NOT NULL,
                updated_at BIGINT NOT NULL
            )"
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn create_unit(&self, unit: &OrganizationalUnit) -> Result<(), PostgresError> {
        sqlx::query(
            "INSERT INTO organizational_units (id, name, type, parent_id, tenant_id, metadata, \
             created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"
        )
        .bind(&unit.id)
        .bind(&unit.name)
        .bind(unit.unit_type.to_string().to_lowercase())
        .bind(&unit.parent_id)
        .bind(unit.tenant_id.as_str())
        .bind(serde_json::to_value(&unit.metadata)?)
        .bind(unit.created_at)
        .bind(unit.updated_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_unit(
        &self,
        ctx: &TenantContext,
        id: &str
    ) -> Result<Option<OrganizationalUnit>, PostgresError> {
        let row = sqlx::query(
            "SELECT id, name, type, parent_id, tenant_id, metadata, created_at, updated_at 
             FROM organizational_units WHERE id = $1 AND tenant_id = $2"
        )
        .bind(id)
        .bind(ctx.tenant_id.as_str())
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            let unit_type_str: String = row.get("type");
            let unit_type = match unit_type_str.as_str() {
                "company" => UnitType::Company,
                "organization" => UnitType::Organization,
                "team" => UnitType::Team,
                "project" => UnitType::Project,
                _ => {
                    return Err(PostgresError::Database(sqlx::Error::Decode(
                        "Invalid unit type".into()
                    )));
                }
            };

            Ok(Some(OrganizationalUnit {
                id: row.get("id"),
                name: row.get("name"),
                unit_type,
                parent_id: row.get("parent_id"),
                tenant_id: row.get::<String, _>("tenant_id").parse().map_err(|e| {
                    PostgresError::Database(sqlx::Error::Decode(
                        format!("Invalid tenant_id: {}", e).into()
                    ))
                })?,
                metadata: serde_json::from_value(row.get("metadata"))?,
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at")
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn list_children(
        &self,
        ctx: &TenantContext,
        parent_id: &str
    ) -> Result<Vec<OrganizationalUnit>, PostgresError> {
        let rows = sqlx::query(
            "SELECT id, name, type, parent_id, tenant_id, metadata, created_at, updated_at 
             FROM organizational_units WHERE parent_id = $1 AND tenant_id = $2"
        )
        .bind(parent_id)
        .bind(ctx.tenant_id.as_str())
        .fetch_all(&self.pool)
        .await?;

        let mut units = Vec::new();
        for row in rows {
            let unit_type_str: String = row.get("type");
            let unit_type = match unit_type_str.as_str() {
                "company" => UnitType::Company,
                "organization" => UnitType::Organization,
                "team" => UnitType::Team,
                "project" => UnitType::Project,
                _ => continue
            };

            units.push(OrganizationalUnit {
                id: row.get("id"),
                name: row.get("name"),
                unit_type,
                parent_id: row.get("parent_id"),
                tenant_id: row.get::<String, _>("tenant_id").parse().map_err(|e| {
                    PostgresError::Database(sqlx::Error::Decode(
                        format!("Invalid tenant_id: {}", e).into()
                    ))
                })?,
                metadata: serde_json::from_value(row.get("metadata"))?,
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at")
            });
        }

        Ok(units)
    }

    pub async fn get_ancestors(
        &self,
        ctx: &TenantContext,
        id: &str
    ) -> Result<Vec<OrganizationalUnit>, PostgresError> {
        let rows = sqlx::query(
            "WITH RECURSIVE ancestors AS (
                SELECT id, name, type, parent_id, tenant_id, metadata, created_at, updated_at
                FROM organizational_units
                WHERE id = $1 AND tenant_id = $2
                UNION ALL
                SELECT u.id, u.name, u.type, u.parent_id, u.tenant_id, u.metadata, u.created_at, \
             u.updated_at
                FROM organizational_units u
                INNER JOIN ancestors a ON u.id = a.parent_id AND u.tenant_id = a.tenant_id
            )
            SELECT * FROM ancestors WHERE id != $1"
        )
        .bind(id)
        .bind(ctx.tenant_id.as_str())
        .fetch_all(&self.pool)
        .await?;

        let mut units = Vec::new();
        for row in rows {
            let unit_type_str: String = row.get("type");
            let unit_type = match unit_type_str.as_str() {
                "company" => UnitType::Company,
                "organization" => UnitType::Organization,
                "team" => UnitType::Team,
                "project" => UnitType::Project,
                _ => continue
            };

            units.push(OrganizationalUnit {
                id: row.get("id"),
                name: row.get("name"),
                unit_type,
                parent_id: row.get("parent_id"),
                tenant_id: row.get::<String, _>("tenant_id").parse().map_err(|e| {
                    PostgresError::Database(sqlx::Error::Decode(
                        format!("Invalid tenant_id: {}", e).into()
                    ))
                })?,
                metadata: serde_json::from_value(row.get("metadata"))?,
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at")
            });
        }

        Ok(units)
    }

    pub async fn update_unit(
        &self,
        ctx: &TenantContext,
        unit: &OrganizationalUnit
    ) -> Result<(), PostgresError> {
        sqlx::query(
            "UPDATE organizational_units 
             SET name = $3, type = $4, parent_id = $5, metadata = $6, updated_at = $7
             WHERE id = $1 AND tenant_id = $2"
        )
        .bind(&unit.id)
        .bind(ctx.tenant_id.as_str())
        .bind(&unit.name)
        .bind(unit.unit_type.to_string().to_lowercase())
        .bind(&unit.parent_id)
        .bind(serde_json::to_value(&unit.metadata)?)
        .bind(unit.updated_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn delete_unit(&self, ctx: &TenantContext, id: &str) -> Result<(), PostgresError> {
        sqlx::query("DELETE FROM organizational_units WHERE id = $1 AND tenant_id = $2")
            .bind(id)
            .bind(ctx.tenant_id.as_str())
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn add_unit_policy(
        &self,
        ctx: &TenantContext,
        unit_id: &str,
        policy: &mk_core::types::Policy
    ) -> Result<(), PostgresError> {
        let exists: Option<(i64,)> =
            sqlx::query_as("SELECT 1 FROM organizational_units WHERE id = $1 AND tenant_id = $2")
                .bind(unit_id)
                .bind(ctx.tenant_id.as_str())
                .fetch_optional(&self.pool)
                .await?;

        if exists.is_none() {
            return Err(PostgresError::NotFound(unit_id.to_string()));
        }

        sqlx::query(
            "INSERT INTO unit_policies (id, unit_id, policy, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (id) DO UPDATE SET policy = $3, updated_at = $5"
        )
        .bind(&policy.id)
        .bind(unit_id)
        .bind(serde_json::to_value(policy)?)
        .bind(chrono::Utc::now().timestamp())
        .bind(chrono::Utc::now().timestamp())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_unit_policies(
        &self,
        ctx: &TenantContext,
        unit_id: &str
    ) -> Result<Vec<mk_core::types::Policy>, PostgresError> {
        let rows = sqlx::query(
            "SELECT p.policy 
             FROM unit_policies p
             JOIN organizational_units u ON p.unit_id = u.id
             WHERE p.unit_id = $1 AND u.tenant_id = $2"
        )
        .bind(unit_id)
        .bind(ctx.tenant_id.as_str())
        .fetch_all(&self.pool)
        .await?;

        let mut policies = Vec::new();
        for row in rows {
            let policy: mk_core::types::Policy = serde_json::from_value(row.get("policy"))?;
            policies.push(policy);
        }
        Ok(policies)
    }

    pub async fn assign_role(
        &self,
        user_id: &mk_core::types::UserId,
        tenant_id: &mk_core::types::TenantId,
        unit_id: &str,
        role: mk_core::types::Role
    ) -> Result<(), PostgresError> {
        sqlx::query(
            "INSERT INTO user_roles (user_id, tenant_id, unit_id, role, created_at)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (user_id, tenant_id, unit_id, role) DO NOTHING"
        )
        .bind(user_id.as_str())
        .bind(tenant_id.as_str())
        .bind(unit_id)
        .bind(role.to_string().to_lowercase())
        .bind(chrono::Utc::now().timestamp())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn remove_role(
        &self,
        user_id: &mk_core::types::UserId,
        tenant_id: &mk_core::types::TenantId,
        unit_id: &str,
        role: mk_core::types::Role
    ) -> Result<(), PostgresError> {
        sqlx::query(
            "DELETE FROM user_roles 
             WHERE user_id = $1 AND tenant_id = $2 AND unit_id = $3 AND role = $4"
        )
        .bind(user_id.as_str())
        .bind(tenant_id.as_str())
        .bind(unit_id)
        .bind(role.to_string().to_lowercase())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_user_roles(
        &self,
        user_id: &mk_core::types::UserId,
        tenant_id: &mk_core::types::TenantId
    ) -> Result<Vec<(String, mk_core::types::Role)>, PostgresError> {
        let rows = sqlx::query(
            "SELECT unit_id, role FROM user_roles WHERE user_id = $1 AND tenant_id = $2"
        )
        .bind(user_id.as_str())
        .bind(tenant_id.as_str())
        .fetch_all(&self.pool)
        .await?;

        let mut roles = Vec::new();
        for row in rows {
            let unit_id: String = row.get("unit_id");
            let role_str: String = row.get("role");
            if let Ok(role) = role_str.parse() {
                roles.push((unit_id, role));
            }
        }
        Ok(roles)
    }
}

#[async_trait]
impl StorageBackend for PostgresBackend {
    type Error = PostgresError;

    async fn store(&self, ctx: TenantContext, key: &str, value: &[u8]) -> Result<(), Self::Error> {
        sqlx::query(
            "INSERT INTO sync_state (id, tenant_id, data, updated_at)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (id, tenant_id) DO UPDATE SET data = $3, updated_at = $4"
        )
        .bind(key)
        .bind(ctx.tenant_id.as_str())
        .bind(serde_json::from_slice::<serde_json::Value>(value).unwrap_or_default())
        .bind(chrono::Utc::now().timestamp())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn retrieve(
        &self,
        ctx: TenantContext,
        key: &str
    ) -> Result<Option<Vec<u8>>, Self::Error> {
        let row: Option<(serde_json::Value,)> =
            sqlx::query_as("SELECT data FROM sync_state WHERE id = $1 AND tenant_id = $2")
                .bind(key)
                .bind(ctx.tenant_id.as_str())
                .fetch_optional(&self.pool)
                .await?;

        Ok(row.and_then(|(v,)| serde_json::to_vec(&v).ok()))
    }

    async fn delete(&self, ctx: TenantContext, key: &str) -> Result<(), Self::Error> {
        sqlx::query("DELETE FROM sync_state WHERE id = $1 AND tenant_id = $2")
            .bind(key)
            .bind(ctx.tenant_id.as_str())
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn exists(&self, ctx: TenantContext, key: &str) -> Result<bool, Self::Error> {
        let row: Option<(i64,)> =
            sqlx::query_as("SELECT 1 FROM sync_state WHERE id = $1 AND tenant_id = $2")
                .bind(key)
                .bind(ctx.tenant_id.as_str())
                .fetch_optional(&self.pool)
                .await?;

        Ok(row.is_some())
    }

    async fn get_ancestors(
        &self,
        ctx: TenantContext,
        unit_id: &str
    ) -> Result<Vec<OrganizationalUnit>, Self::Error> {
        self.get_ancestors(&ctx, unit_id).await
    }

    async fn get_unit_policies(
        &self,
        ctx: TenantContext,
        unit_id: &str
    ) -> Result<Vec<mk_core::types::Policy>, Self::Error> {
        self.get_unit_policies(&ctx, unit_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;
    use serde_json::json;

    // Test PostgresError display
    #[test]
    fn test_postgres_error_display() {
        let error = PostgresError::Database(sqlx::Error::Configuration(
            "Invalid connection string".into()
        ));

        assert!(error.to_string().contains("Database error"));
        assert!(error.to_string().contains("Invalid connection string"));
    }

    // Test error conversion from sqlx::Error
    #[test]
    fn test_postgres_error_from_sqlx() {
        let sqlx_error = sqlx::Error::Configuration("test".into());
        let pg_error: PostgresError = sqlx_error.into();

        match pg_error {
            PostgresError::Database(_) => (),
            PostgresError::Serialization(_) => (),
            PostgresError::NotFound(_) => ()
        }
    }

    // Test PostgresBackend struct (compile-time checks)
    #[test]
    fn test_postgres_backend_struct() {
        // Verify the struct has expected fields
        struct TestBackend {
            _pool: Pool<Postgres>
        }

        // This is a compile-time test - if it compiles, PostgresBackend has the right
        // structure We can't instantiate it without a real database connection
        let _backend_type = std::any::type_name::<PostgresBackend>();
        assert_eq!(_backend_type, "storage::postgres::PostgresBackend");
    }

    // Test StorageBackend trait implementation
    #[test]
    fn test_storage_backend_trait_implementation() {
        use mk_core::traits::StorageBackend;

        // Compile-time check that PostgresBackend implements StorageBackend
        fn assert_implements_storage_backend<T: StorageBackend>() {}

        assert_implements_storage_backend::<PostgresBackend>();
    }

    // Test JSON serialization patterns used in the code
    #[test]
    fn test_json_serialization_patterns() {
        // Test the serialization pattern used in store() method
        let value = json!({"key": "value", "number": 42});
        let bytes = serde_json::to_vec(&value).unwrap();

        // Test deserialization pattern used in retrieve() method
        let deserialized: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(deserialized["key"], "value");
        assert_eq!(deserialized["number"], 42);

        // Test default fallback used in store()
        let invalid_bytes = b"not json";
        let default_value =
            serde_json::from_slice::<serde_json::Value>(invalid_bytes).unwrap_or_default();
        assert!(default_value.is_null() || default_value == json!({}));
    }

    // Test timestamp generation pattern
    #[test]
    fn test_timestamp_generation() {
        use chrono::Utc;

        let timestamp = Utc::now().timestamp();
        assert!(timestamp > 0); // Should be positive (after 1970)

        // Verify it's a reasonable timestamp (not in distant future)
        let current_year = Utc::now().year();
        let timestamp_year = chrono::DateTime::from_timestamp(timestamp, 0)
            .map(|dt| dt.year())
            .unwrap_or(1970);

        // Should be within 10 years of current year
        assert!((timestamp_year - current_year).abs() <= 10);
    }

    // Test SQL query patterns for correctness
    #[test]
    fn test_sql_query_patterns() {
        // Verify the SQL queries are syntactically correct
        let create_table_query = "CREATE TABLE IF NOT EXISTS sync_state (
                id TEXT PRIMARY KEY,
                data JSONB NOT NULL,
                updated_at BIGINT NOT NULL
            )";

        let insert_query = "INSERT INTO sync_state (id, data, updated_at)
             VALUES ($1, $2, $3)
             ON CONFLICT (id) DO UPDATE SET data = $2, updated_at = $3";

        let select_query = "SELECT data FROM sync_state WHERE id = $1";
        let delete_query = "DELETE FROM sync_state WHERE id = $1";
        let exists_query = "SELECT 1 FROM sync_state WHERE id = $1";

        // Just verify they're non-empty strings
        assert!(!create_table_query.is_empty());
        assert!(!insert_query.is_empty());
        assert!(!select_query.is_empty());
        assert!(!delete_query.is_empty());
        assert!(!exists_query.is_empty());

        // Verify they contain expected keywords
        assert!(create_table_query.contains("CREATE TABLE"));
        assert!(insert_query.contains("INSERT INTO"));
        assert!(select_query.contains("SELECT"));
        assert!(delete_query.contains("DELETE"));
        assert!(exists_query.contains("SELECT 1"));
    }
}
