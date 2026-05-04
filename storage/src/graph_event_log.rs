use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres, Row};
use std::hash::{DefaultHasher, Hash, Hasher};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GraphEventLogError {
    #[error("Database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("Tenant ID is required")]
    MissingTenant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEvent {
    pub id: i64,
    pub tenant_id: String,
    pub seq: i64,
    pub kind: String,
    pub payload: serde_json::Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub struct GraphEventLog {
    pool: Pool<Postgres>,
}

impl GraphEventLog {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }

    fn advisory_lock_key(tenant_id: &str) -> i64 {
        let mut hasher = DefaultHasher::new();
        "graph_event_seq:".hash(&mut hasher);
        tenant_id.hash(&mut hasher);
        hasher.finish() as i64
    }

    /// Append a new event for the given tenant. Allocates a per-tenant
    /// monotonic seq via advisory lock within a single transaction.
    /// Returns the assigned seq.
    pub async fn append(
        &self,
        tenant_id: &str,
        kind: &str,
        payload: serde_json::Value,
    ) -> Result<i64, GraphEventLogError> {
        if tenant_id.is_empty() {
            return Err(GraphEventLogError::MissingTenant);
        }

        let lock_key = Self::advisory_lock_key(tenant_id);

        let mut tx = self.pool.begin().await?;

        sqlx::query("SELECT set_config('app.tenant_id', $1, true)")
            .bind(tenant_id)
            .execute(&mut *tx)
            .await?;

        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(lock_key)
            .execute(&mut *tx)
            .await?;

        let next_seq: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(seq), 0) + 1 FROM graph_events WHERE tenant_id = $1",
        )
        .bind(tenant_id)
        .fetch_one(&mut *tx)
        .await?;

        sqlx::query(
            "INSERT INTO graph_events (tenant_id, seq, kind, payload) VALUES ($1, $2, $3, $4)",
        )
        .bind(tenant_id)
        .bind(next_seq)
        .bind(kind)
        .bind(&payload)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(next_seq)
    }

    /// Tail events for a tenant starting after `from_seq`, returning up
    /// to `batch_size` events ordered by seq.
    pub async fn tail(
        &self,
        tenant_id: &str,
        from_seq: i64,
        batch_size: i64,
    ) -> Result<Vec<GraphEvent>, GraphEventLogError> {
        if tenant_id.is_empty() {
            return Err(GraphEventLogError::MissingTenant);
        }

        let rows = sqlx::query(
            r#"
            SELECT id, tenant_id, seq, kind, payload, created_at
            FROM graph_events
            WHERE tenant_id = $1 AND seq > $2
            ORDER BY seq ASC
            LIMIT $3
            "#,
        )
        .bind(tenant_id)
        .bind(from_seq)
        .bind(batch_size)
        .fetch_all(&self.pool)
        .await?;

        let events = rows
            .into_iter()
            .map(|row| GraphEvent {
                id: row.get("id"),
                tenant_id: row.get("tenant_id"),
                seq: row.get("seq"),
                kind: row.get("kind"),
                payload: row.get("payload"),
                created_at: row.get("created_at"),
            })
            .collect();

        Ok(events)
    }

    /// Get the current head seq for a tenant (max seq value).
    pub async fn head_seq(&self, tenant_id: &str) -> Result<i64, GraphEventLogError> {
        let seq: Option<i64> =
            sqlx::query_scalar("SELECT MAX(seq) FROM graph_events WHERE tenant_id = $1")
                .bind(tenant_id)
                .fetch_one(&self.pool)
                .await?;

        Ok(seq.unwrap_or(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_advisory_lock_key_deterministic() {
        let k1 = GraphEventLog::advisory_lock_key("tenant-a");
        let k2 = GraphEventLog::advisory_lock_key("tenant-a");
        let k3 = GraphEventLog::advisory_lock_key("tenant-b");
        assert_eq!(k1, k2);
        assert_ne!(k1, k3);
    }
}
