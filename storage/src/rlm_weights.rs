//! RLM Policy State Persistence.
//!
//! This module provides PostgreSQL storage for RLM decomposition trainer
//! weights, allowing policy states to be persisted and restored across service
//! restarts.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres, Row};
use std::collections::HashMap;
use thiserror::Error;

/// Errors from RLM weight storage operations.
#[derive(Error, Debug)]
pub enum RlmWeightStorageError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Policy state not found for tenant: {0}")]
    NotFound(String)
}

/// Stored policy state matching the trainer's PolicyState structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredPolicyState {
    /// Tenant ID for multi-tenant isolation.
    pub tenant_id: String,
    /// Weights for each action type.
    pub action_weights: HashMap<String, f32>,
    /// Exploration rate (epsilon).
    pub epsilon: f32,
    /// Number of training steps.
    pub step_count: usize,
    /// Last update timestamp.
    pub updated_at: i64
}

/// Trait for RLM weight storage operations.
#[async_trait]
pub trait RlmWeightStorage {
    /// Error type for storage operations.
    type Error: std::error::Error + Send + Sync;

    /// Save policy state for a tenant.
    async fn save_policy_state(&self, state: &StoredPolicyState) -> Result<(), Self::Error>;

    /// Load policy state for a tenant.
    async fn load_policy_state(
        &self,
        tenant_id: &str
    ) -> Result<Option<StoredPolicyState>, Self::Error>;

    /// Delete policy state for a tenant.
    async fn delete_policy_state(&self, tenant_id: &str) -> Result<(), Self::Error>;

    /// List all policy states (for admin/diagnostics).
    async fn list_policy_states(&self, limit: usize)
    -> Result<Vec<StoredPolicyState>, Self::Error>;
}

/// PostgreSQL implementation of RLM weight storage.
pub struct PostgresRlmWeightStorage {
    pool: Pool<Postgres>
}

impl PostgresRlmWeightStorage {
    /// Creates a new PostgreSQL RLM weight storage.
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }

    /// Initializes the RLM policy state table schema.
    pub async fn initialize_schema(&self) -> Result<(), RlmWeightStorageError> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS rlm_policy_state (
                tenant_id TEXT PRIMARY KEY,
                action_weights JSONB NOT NULL DEFAULT '{}',
                epsilon REAL NOT NULL DEFAULT 0.1,
                step_count BIGINT NOT NULL DEFAULT 0,
                updated_at BIGINT NOT NULL,
                created_at BIGINT NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::BIGINT
            )"
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_rlm_policy_state_updated_at ON \
             rlm_policy_state(updated_at)"
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Gets the underlying connection pool.
    pub fn pool(&self) -> &Pool<Postgres> {
        &self.pool
    }
}

#[async_trait]
impl RlmWeightStorage for PostgresRlmWeightStorage {
    type Error = RlmWeightStorageError;

    async fn save_policy_state(&self, state: &StoredPolicyState) -> Result<(), Self::Error> {
        let weights_json = serde_json::to_value(&state.action_weights)?;

        sqlx::query(
            "INSERT INTO rlm_policy_state (tenant_id, action_weights, epsilon, step_count, \
             updated_at)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (tenant_id) DO UPDATE SET
                 action_weights = $2,
                 epsilon = $3,
                 step_count = $4,
                 updated_at = $5"
        )
        .bind(&state.tenant_id)
        .bind(&weights_json)
        .bind(state.epsilon)
        .bind(state.step_count as i64)
        .bind(state.updated_at)
        .execute(&self.pool)
        .await?;

        tracing::debug!(
            "Saved RLM policy state for tenant {} with {} action weights",
            state.tenant_id,
            state.action_weights.len()
        );

        Ok(())
    }

    async fn load_policy_state(
        &self,
        tenant_id: &str
    ) -> Result<Option<StoredPolicyState>, Self::Error> {
        let row = sqlx::query(
            "SELECT tenant_id, action_weights, epsilon, step_count, updated_at
             FROM rlm_policy_state
             WHERE tenant_id = $1"
        )
        .bind(tenant_id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            let weights_json: serde_json::Value = row.get("action_weights");
            let action_weights: HashMap<String, f32> = serde_json::from_value(weights_json)?;

            Ok(Some(StoredPolicyState {
                tenant_id: row.get("tenant_id"),
                action_weights,
                epsilon: row.get("epsilon"),
                step_count: row.get::<i64, _>("step_count") as usize,
                updated_at: row.get("updated_at")
            }))
        } else {
            Ok(None)
        }
    }

    async fn delete_policy_state(&self, tenant_id: &str) -> Result<(), Self::Error> {
        sqlx::query("DELETE FROM rlm_policy_state WHERE tenant_id = $1")
            .bind(tenant_id)
            .execute(&self.pool)
            .await?;

        tracing::debug!("Deleted RLM policy state for tenant {}", tenant_id);
        Ok(())
    }

    async fn list_policy_states(
        &self,
        limit: usize
    ) -> Result<Vec<StoredPolicyState>, Self::Error> {
        let rows = sqlx::query(
            "SELECT tenant_id, action_weights, epsilon, step_count, updated_at
             FROM rlm_policy_state
             ORDER BY updated_at DESC
             LIMIT $1"
        )
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        let mut states = Vec::with_capacity(rows.len());
        for row in rows {
            let weights_json: serde_json::Value = row.get("action_weights");
            let action_weights: HashMap<String, f32> = serde_json::from_value(weights_json)?;

            states.push(StoredPolicyState {
                tenant_id: row.get("tenant_id"),
                action_weights,
                epsilon: row.get("epsilon"),
                step_count: row.get::<i64, _>("step_count") as usize,
                updated_at: row.get("updated_at")
            });
        }

        Ok(states)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stored_policy_state_serialization() {
        let mut action_weights = HashMap::new();
        action_weights.insert("SearchLayer".to_string(), 0.8);
        action_weights.insert("DrillDown".to_string(), 0.6);

        let state = StoredPolicyState {
            tenant_id: "test-tenant".to_string(),
            action_weights,
            epsilon: 0.05,
            step_count: 1000,
            updated_at: chrono::Utc::now().timestamp()
        };

        let json = serde_json::to_string(&state).unwrap();
        let deserialized: StoredPolicyState = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.tenant_id, "test-tenant");
        assert_eq!(deserialized.epsilon, 0.05);
        assert_eq!(deserialized.step_count, 1000);
        assert_eq!(deserialized.action_weights.len(), 2);
        assert_eq!(deserialized.action_weights.get("SearchLayer"), Some(&0.8));
    }

    #[test]
    fn test_stored_policy_state_default_values() {
        let state = StoredPolicyState {
            tenant_id: "tenant".to_string(),
            action_weights: HashMap::new(),
            epsilon: 0.1,
            step_count: 0,
            updated_at: 0
        };

        assert!(state.action_weights.is_empty());
        assert_eq!(state.epsilon, 0.1);
        assert_eq!(state.step_count, 0);
    }
}
