use async_trait::async_trait;
use mk_core::traits::StorageBackend;
use sqlx::{Pool, Postgres};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PostgresError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}

pub struct PostgresBackend {
    pool: Pool<Postgres>,
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
            )",
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

#[async_trait]
impl StorageBackend for PostgresBackend {
    type Error = PostgresError;

    async fn store(&self, key: &str, value: &[u8]) -> Result<(), Self::Error> {
        sqlx::query(
            "INSERT INTO sync_state (id, data, updated_at)
             VALUES ($1, $2, $3)
             ON CONFLICT (id) DO UPDATE SET data = $2, updated_at = $3",
        )
        .bind(key)
        .bind(serde_json::from_slice::<serde_json::Value>(value).unwrap_or_default())
        .bind(chrono::Utc::now().timestamp())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn retrieve(&self, key: &str) -> Result<Option<Vec<u8>>, Self::Error> {
        let row: Option<(serde_json::Value,)> =
            sqlx::query_as("SELECT data FROM sync_state WHERE id = $1")
                .bind(key)
                .fetch_optional(&self.pool)
                .await?;

        Ok(row.and_then(|(v,)| serde_json::to_vec(&v).ok()))
    }

    async fn delete(&self, key: &str) -> Result<(), Self::Error> {
        sqlx::query("DELETE FROM sync_state WHERE id = $1")
            .bind(key)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn exists(&self, key: &str) -> Result<bool, Self::Error> {
        let row: Option<(i64,)> = sqlx::query_as("SELECT 1 FROM sync_state WHERE id = $1")
            .bind(key)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.is_some())
    }
}
