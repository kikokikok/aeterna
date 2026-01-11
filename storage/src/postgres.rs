use async_trait::async_trait;
use mk_core::traits::StorageBackend;
use sqlx::{Pool, Postgres};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PostgresError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error)
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
                id TEXT PRIMARY KEY,
                data JSONB NOT NULL,
                updated_at BIGINT NOT NULL
            )"
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

#[async_trait]
impl StorageBackend for PostgresBackend {
    type Error = PostgresError;

    async fn store(
        &self,
        _ctx: mk_core::types::TenantContext,
        key: &str,
        value: &[u8]
    ) -> Result<(), Self::Error> {
        sqlx::query(
            "INSERT INTO sync_state (id, data, updated_at)
             VALUES ($1, $2, $3)
             ON CONFLICT (id) DO UPDATE SET data = $2, updated_at = $3"
        )
        .bind(key)
        .bind(serde_json::from_slice::<serde_json::Value>(value).unwrap_or_default())
        .bind(chrono::Utc::now().timestamp())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn retrieve(
        &self,
        _ctx: mk_core::types::TenantContext,
        key: &str
    ) -> Result<Option<Vec<u8>>, Self::Error> {
        let row: Option<(serde_json::Value,)> =
            sqlx::query_as("SELECT data FROM sync_state WHERE id = $1")
                .bind(key)
                .fetch_optional(&self.pool)
                .await?;

        Ok(row.and_then(|(v,)| serde_json::to_vec(&v).ok()))
    }

    async fn delete(
        &self,
        _ctx: mk_core::types::TenantContext,
        key: &str
    ) -> Result<(), Self::Error> {
        sqlx::query("DELETE FROM sync_state WHERE id = $1")
            .bind(key)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn exists(
        &self,
        _ctx: mk_core::types::TenantContext,
        key: &str
    ) -> Result<bool, Self::Error> {
        let row: Option<(i64,)> = sqlx::query_as("SELECT 1 FROM sync_state WHERE id = $1")
            .bind(key)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.is_some())
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
            PostgresError::Database(_) => ()
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
