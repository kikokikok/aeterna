use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool as Pool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thread {
    pub thread_id: Uuid,
    pub tenant_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub context_json: serde_json::Value,
    pub state: serde_json::Value,
    pub ttl_seconds: i32,
}

pub struct ThreadRepository {
    _pool: Pool,
}

impl ThreadRepository {
    #[must_use]
    pub fn new(pool: Pool) -> Self {
        Self { _pool: pool }
    }

    pub async fn create_thread(
        &self,
        _tenant_id: &str,
        _context: serde_json::Value,
        _ttl_seconds: i32,
    ) -> Result<Thread, anyhow::Error> {
        anyhow::bail!("Thread persistence is not yet implemented")
    }

    pub async fn get_thread(&self, _thread_id: Uuid) -> Result<Option<Thread>, anyhow::Error> {
        anyhow::bail!("Thread persistence is not yet implemented")
    }

    pub async fn update_thread(
        &self,
        _thread_id: Uuid,
        _state: serde_json::Value,
    ) -> Result<(), anyhow::Error> {
        anyhow::bail!("Thread persistence is not yet implemented")
    }

    pub async fn list_threads(&self, _tenant_id: &str) -> Result<Vec<Thread>, anyhow::Error> {
        anyhow::bail!("Thread persistence is not yet implemented")
    }

    pub async fn delete_expired_threads(&self) -> Result<u64, anyhow::Error> {
        anyhow::bail!("Thread persistence is not yet implemented")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_thread_fails_when_persistence_is_unimplemented() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://localhost/aeterna")
            .expect("lazy pool");
        let repository = ThreadRepository::new(pool);

        let err = repository
            .create_thread("tenant-1", serde_json::json!({"scope": "test"}), 3600)
            .await
            .expect_err("create_thread must fail until persistence is wired");

        assert!(err.to_string().contains("not yet implemented"));
    }

    #[tokio::test]
    async fn test_get_thread_fails_when_persistence_is_unimplemented() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://localhost/aeterna")
            .expect("lazy pool");
        let repository = ThreadRepository::new(pool);

        let err = repository
            .get_thread(Uuid::new_v4())
            .await
            .expect_err("get_thread must fail until persistence is wired");

        assert!(err.to_string().contains("not yet implemented"));
    }

    #[tokio::test]
    async fn test_cleanup_fails_when_persistence_is_unimplemented() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://localhost/aeterna")
            .expect("lazy pool");
        let repository = ThreadRepository::new(pool);

        let err = repository
            .delete_expired_threads()
            .await
            .expect_err("cleanup must fail until persistence is wired");

        assert!(err.to_string().contains("not yet implemented"));
    }
}
