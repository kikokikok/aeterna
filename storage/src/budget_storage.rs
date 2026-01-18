use mk_core::types::MemoryLayer;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres, Row};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum BudgetStorageError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Budget not found for tenant: {0}")]
    NotFound(String)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredBudget {
    pub tenant_id: String,
    pub daily_token_limit: i64,
    pub hourly_token_limit: i64,
    pub per_layer_limits: serde_json::Value,
    pub warning_threshold_percent: i32,
    pub critical_threshold_percent: i32,
    pub exhausted_action: String,
    pub created_at: i64,
    pub updated_at: i64
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredUsage {
    pub tenant_id: String,
    pub layer: String,
    pub window_type: String,
    pub tokens_used: i64,
    pub window_start: i64
}

pub struct BudgetStorage {
    pool: Pool<Postgres>
}

impl BudgetStorage {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }

    pub async fn initialize_schema(&self) -> Result<(), BudgetStorageError> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS summarization_budgets (
                tenant_id TEXT PRIMARY KEY,
                daily_token_limit BIGINT NOT NULL DEFAULT 1000000,
                hourly_token_limit BIGINT NOT NULL DEFAULT 100000,
                per_layer_limits JSONB NOT NULL DEFAULT '{}',
                warning_threshold_percent INTEGER NOT NULL DEFAULT 80,
                critical_threshold_percent INTEGER NOT NULL DEFAULT 90,
                exhausted_action TEXT NOT NULL DEFAULT 'reject',
                created_at BIGINT NOT NULL,
                updated_at BIGINT NOT NULL
            )"
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS summarization_usage (
                tenant_id TEXT NOT NULL,
                layer TEXT NOT NULL,
                window_type TEXT NOT NULL,
                tokens_used BIGINT NOT NULL DEFAULT 0,
                window_start BIGINT NOT NULL,
                PRIMARY KEY (tenant_id, layer, window_type)
            )"
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_summarization_usage_tenant 
             ON summarization_usage(tenant_id)"
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_budget(
        &self,
        tenant_id: &str
    ) -> Result<Option<StoredBudget>, BudgetStorageError> {
        let row = sqlx::query(
            "SELECT tenant_id, daily_token_limit, hourly_token_limit, per_layer_limits,
                    warning_threshold_percent, critical_threshold_percent, exhausted_action,
                    created_at, updated_at
             FROM summarization_budgets WHERE tenant_id = $1"
        )
        .bind(tenant_id)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => Ok(Some(StoredBudget {
                tenant_id: row.get("tenant_id"),
                daily_token_limit: row.get("daily_token_limit"),
                hourly_token_limit: row.get("hourly_token_limit"),
                per_layer_limits: row.get("per_layer_limits"),
                warning_threshold_percent: row.get("warning_threshold_percent"),
                critical_threshold_percent: row.get("critical_threshold_percent"),
                exhausted_action: row.get("exhausted_action"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at")
            })),
            None => Ok(None)
        }
    }

    pub async fn upsert_budget(&self, budget: &StoredBudget) -> Result<(), BudgetStorageError> {
        sqlx::query(
            "INSERT INTO summarization_budgets 
             (tenant_id, daily_token_limit, hourly_token_limit, per_layer_limits,
              warning_threshold_percent, critical_threshold_percent, exhausted_action,
              created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
             ON CONFLICT (tenant_id) DO UPDATE SET
                daily_token_limit = EXCLUDED.daily_token_limit,
                hourly_token_limit = EXCLUDED.hourly_token_limit,
                per_layer_limits = EXCLUDED.per_layer_limits,
                warning_threshold_percent = EXCLUDED.warning_threshold_percent,
                critical_threshold_percent = EXCLUDED.critical_threshold_percent,
                exhausted_action = EXCLUDED.exhausted_action,
                updated_at = EXCLUDED.updated_at"
        )
        .bind(&budget.tenant_id)
        .bind(budget.daily_token_limit)
        .bind(budget.hourly_token_limit)
        .bind(&budget.per_layer_limits)
        .bind(budget.warning_threshold_percent)
        .bind(budget.critical_threshold_percent)
        .bind(&budget.exhausted_action)
        .bind(budget.created_at)
        .bind(budget.updated_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn delete_budget(&self, tenant_id: &str) -> Result<bool, BudgetStorageError> {
        let result = sqlx::query("DELETE FROM summarization_budgets WHERE tenant_id = $1")
            .bind(tenant_id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn record_usage(
        &self,
        tenant_id: &str,
        layer: MemoryLayer,
        window_type: &str,
        tokens: i64,
        window_start: i64
    ) -> Result<(), BudgetStorageError> {
        let layer_str = layer.display_name();

        sqlx::query(
            "INSERT INTO summarization_usage 
             (tenant_id, layer, window_type, tokens_used, window_start)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (tenant_id, layer, window_type) DO UPDATE SET
                tokens_used = CASE 
                    WHEN summarization_usage.window_start < $5 THEN $4
                    ELSE summarization_usage.tokens_used + $4
                END,
                window_start = CASE 
                    WHEN summarization_usage.window_start < $5 THEN $5
                    ELSE summarization_usage.window_start
                END"
        )
        .bind(tenant_id)
        .bind(layer_str)
        .bind(window_type)
        .bind(tokens)
        .bind(window_start)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_usage(
        &self,
        tenant_id: &str,
        layer: Option<MemoryLayer>,
        window_type: &str,
        current_window_start: i64
    ) -> Result<i64, BudgetStorageError> {
        let query = if let Some(l) = layer {
            let layer_str = l.display_name();
            sqlx::query(
                "SELECT COALESCE(SUM(
                    CASE WHEN window_start >= $3 THEN tokens_used ELSE 0 END
                ), 0) as total
                 FROM summarization_usage 
                 WHERE tenant_id = $1 AND layer = $2 AND window_type = $4"
            )
            .bind(tenant_id)
            .bind(layer_str)
            .bind(current_window_start)
            .bind(window_type)
        } else {
            sqlx::query(
                "SELECT COALESCE(SUM(
                    CASE WHEN window_start >= $2 THEN tokens_used ELSE 0 END
                ), 0) as total
                 FROM summarization_usage 
                 WHERE tenant_id = $1 AND window_type = $3"
            )
            .bind(tenant_id)
            .bind(current_window_start)
            .bind(window_type)
        };

        let row = query.fetch_one(&self.pool).await?;
        let total: i64 = row.get("total");
        Ok(total)
    }

    pub async fn get_all_layer_usage(
        &self,
        tenant_id: &str,
        window_type: &str,
        current_window_start: i64
    ) -> Result<Vec<(String, i64)>, BudgetStorageError> {
        let rows = sqlx::query(
            "SELECT layer, 
                    CASE WHEN window_start >= $2 THEN tokens_used ELSE 0 END as tokens
             FROM summarization_usage 
             WHERE tenant_id = $1 AND window_type = $3"
        )
        .bind(tenant_id)
        .bind(current_window_start)
        .bind(window_type)
        .fetch_all(&self.pool)
        .await?;

        let results: Vec<(String, i64)> = rows
            .iter()
            .map(|row| (row.get("layer"), row.get("tokens")))
            .collect();

        Ok(results)
    }

    pub async fn reset_usage(
        &self,
        tenant_id: &str,
        window_type: &str
    ) -> Result<(), BudgetStorageError> {
        sqlx::query(
            "DELETE FROM summarization_usage 
             WHERE tenant_id = $1 AND window_type = $2"
        )
        .bind(tenant_id)
        .bind(window_type)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn cleanup_old_usage(
        &self,
        before_timestamp: i64
    ) -> Result<u64, BudgetStorageError> {
        let result = sqlx::query("DELETE FROM summarization_usage WHERE window_start < $1")
            .bind(before_timestamp)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stored_budget_serialization() {
        let budget = StoredBudget {
            tenant_id: "test-tenant".to_string(),
            daily_token_limit: 1_000_000,
            hourly_token_limit: 100_000,
            per_layer_limits: serde_json::json!({
                "session": 50000,
                "project": 100000
            }),
            warning_threshold_percent: 80,
            critical_threshold_percent: 90,
            exhausted_action: "reject".to_string(),
            created_at: 1704067200,
            updated_at: 1704067200
        };

        let json = serde_json::to_string(&budget).unwrap();
        let deserialized: StoredBudget = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.tenant_id, "test-tenant");
        assert_eq!(deserialized.daily_token_limit, 1_000_000);
        assert_eq!(deserialized.exhausted_action, "reject");
    }

    #[test]
    fn test_stored_usage_serialization() {
        let usage = StoredUsage {
            tenant_id: "test-tenant".to_string(),
            layer: "session".to_string(),
            window_type: "daily".to_string(),
            tokens_used: 5000,
            window_start: 1704067200
        };

        let json = serde_json::to_string(&usage).unwrap();
        let deserialized: StoredUsage = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.tenant_id, "test-tenant");
        assert_eq!(deserialized.tokens_used, 5000);
    }
}
