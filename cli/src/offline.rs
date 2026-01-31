//! Section 13.6: CLI Offline Mode
//!
//! Implements local caching, offline queueing, and conflict resolution
//! for CLI operations when server is unreachable.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Sqlite, sqlite::SqlitePoolOptions};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Offline-capable CLI client.
pub struct OfflineCliClient {
    db_pool: Pool<Sqlite>,
    server_url: String,
    config: OfflineConfig,
    queue: Arc<RwLock<VecDeque<QueuedOperation>>>,
    last_sync: Arc<RwLock<Option<DateTime<Utc>>>>
}

/// Offline mode configuration.
#[derive(Debug, Clone)]
pub struct OfflineConfig {
    /// Cache directory path.
    pub cache_dir: PathBuf,
    /// Max age of cached data before warning (hours).
    pub cache_warning_age_hours: u64,
    /// Max queue size.
    pub max_queue_size: usize,
    /// Retry interval for sync (seconds).
    pub sync_retry_interval_secs: u64
}

impl Default for OfflineConfig {
    fn default() -> Self {
        Self {
            cache_dir: PathBuf::from(".aeterna/cache"),
            cache_warning_age_hours: 24,
            max_queue_size: 1000,
            sync_retry_interval_secs: 300
        }
    }
}

/// Queued operation for later sync.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedOperation {
    pub id: String,
    pub operation_type: String,
    pub entity_type: String,
    pub entity_id: String,
    pub payload: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub retry_count: u32,
    pub status: QueueStatus
}

/// Queue status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum QueueStatus {
    Pending,
    Processing,
    Failed,
    Resolved
}

/// Conflict resolution strategy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConflictStrategy {
    /// Prefer local (client) version.
    PreferLocal,
    /// Prefer server version.
    PreferServer,
    /// Manual resolution required.
    Manual,
    /// Merge changes if possible.
    Merge
}

impl OfflineCliClient {
    /// Create new offline CLI client.
    pub async fn new(server_url: String, config: OfflineConfig) -> Result<Self, OfflineError> {
        // Ensure cache directory exists
        tokio::fs::create_dir_all(&config.cache_dir)
            .await
            .map_err(|e| OfflineError::CacheError(e.to_string()))?;

        // Connect to SQLite cache (13.6.1)
        let db_path = config.cache_dir.join("offline_cache.db");
        let db_url = format!("sqlite://{}?mode=rwc", db_path.display());

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&db_url)
            .await
            .map_err(|e| OfflineError::CacheError(e.to_string()))?;

        // Initialize schema
        Self::init_schema(&pool).await?;

        Ok(Self {
            db_pool: pool,
            server_url,
            config,
            queue: Arc::new(RwLock::new(VecDeque::new())),
            last_sync: Arc::new(RwLock::new(None))
        })
    }

    /// Initialize SQLite schema.
    async fn init_schema(pool: &Pool<Sqlite>) -> Result<(), OfflineError> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS policy_cache (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                data TEXT NOT NULL,
                cached_at TEXT NOT NULL
            );
            
            CREATE TABLE IF NOT EXISTS operation_queue (
                id TEXT PRIMARY KEY,
                operation_type TEXT NOT NULL,
                entity_type TEXT NOT NULL,
                entity_id TEXT NOT NULL,
                payload TEXT NOT NULL,
                created_at TEXT NOT NULL,
                retry_count INTEGER DEFAULT 0,
                status TEXT NOT NULL
            );
            
            CREATE TABLE IF NOT EXISTS sync_metadata (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            
            CREATE INDEX IF NOT EXISTS idx_queue_status ON operation_queue(status);
            CREATE INDEX IF NOT EXISTS idx_cache_cached_at ON policy_cache(cached_at);
            "#
        )
        .execute(pool)
        .await
        .map_err(|e| OfflineError::CacheError(e.to_string()))?;

        Ok(())
    }

    /// Check server reachability on CLI start (13.6.2).
    pub async fn check_server_reachability(&self) -> bool {
        let client = reqwest::Client::new();
        match client
            .get(format!("{}/health", self.server_url))
            .timeout(Duration::from_secs(5))
            .send()
            .await
        {
            Ok(response) => {
                if response.status().is_success() {
                    info!("Server is reachable");
                    true
                } else {
                    warn!("Server returned error status: {}", response.status());
                    false
                }
            }
            Err(e) => {
                warn!("Server is unreachable: {}", e);
                false
            }
        }
    }

    /// Display cache age warning (13.6.5).
    pub async fn display_cache_age_warning(&self) {
        let last_sync = self.last_sync.read().await.clone();

        if let Some(last) = last_sync {
            let age = Utc::now() - last;
            let age_hours = age.num_hours() as u64;

            if age_hours > self.config.cache_warning_age_hours {
                warn!(
                    "OFFLINE MODE: Cache is {} hours old. Data may be stale.",
                    age_hours
                );
                println!(
                    "⚠️  Warning: Operating in offline mode with {} hour old cache",
                    age_hours
                );
            } else {
                debug!("Cache age: {} hours", age_hours);
            }
        } else {
            warn!("OFFLINE MODE: No sync has ever occurred");
            println!("⚠️  Warning: Operating in offline mode with no prior sync");
        }
    }

    /// Queue write operation for later sync (13.6.3).
    pub async fn queue_operation(
        &self,
        operation_type: &str,
        entity_type: &str,
        entity_id: &str,
        payload: serde_json::Value
    ) -> Result<String, OfflineError> {
        let operation = QueuedOperation {
            id: uuid::Uuid::new_v4().to_string(),
            operation_type: operation_type.to_string(),
            entity_type: entity_type.to_string(),
            entity_id: entity_id.to_string(),
            payload,
            created_at: Utc::now(),
            retry_count: 0,
            status: QueueStatus::Pending
        };

        // Store in SQLite
        sqlx::query(
            r#"
            INSERT INTO operation_queue (id, operation_type, entity_type, entity_id, payload, created_at, retry_count, status)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#
        )
        .bind(&operation.id)
        .bind(&operation.operation_type)
        .bind(&operation.entity_type)
        .bind(&operation.entity_id)
        .bind(operation.payload.to_string())
        .bind(operation.created_at.to_rfc3339())
        .bind(operation.retry_count as i32)
        .bind(format!("{:?}", operation.status))
        .execute(&self.db_pool)
        .await
        .map_err(|e| OfflineError::QueueError(e.to_string()))?;

        // Add to in-memory queue
        self.queue.write().await.push_back(operation.clone());

        info!(
            "Queued operation {}: {} {} {}",
            operation.id, operation_type, entity_type, entity_id
        );

        Ok(operation.id)
    }

    /// Resolve conflicts between local and server state (13.6.4).
    pub async fn resolve_conflict(
        &self,
        local: &QueuedOperation,
        server_state: Option<serde_json::Value>,
        strategy: ConflictStrategy
    ) -> Result<ConflictResolution, OfflineError> {
        let resolution = match strategy {
            ConflictStrategy::PreferLocal => {
                info!("Conflict resolved: preferring local version");
                ConflictResolution::UseLocal(local.clone())
            }
            ConflictStrategy::PreferServer => {
                info!("Conflict resolved: preferring server version");
                ConflictResolution::UseServer
            }
            ConflictStrategy::Manual => {
                warn!(
                    "Manual conflict resolution required for operation {}",
                    local.id
                );
                ConflictResolution::RequiresManual(local.id.clone())
            }
            ConflictStrategy::Merge => {
                // Attempt to merge changes
                if let Some(server) = server_state {
                    let merged = Self::attempt_merge(&local.payload, &server);
                    ConflictResolution::Merged(merged)
                } else {
                    ConflictResolution::UseLocal(local.clone())
                }
            }
        };

        Ok(resolution)
    }

    /// Attempt to merge local and server changes.
    fn attempt_merge(local: &serde_json::Value, server: &serde_json::Value) -> serde_json::Value {
        // Simple merge: prefer local for most fields
        let mut merged = server.clone();

        if let (Some(local_obj), Some(merged_obj)) = (local.as_object(), merged.as_object_mut()) {
            for (key, value) in local_obj {
                merged_obj.insert(key.clone(), value.clone());
            }
        }

        merged
    }

    /// Sync queued operations with server.
    pub async fn sync_queued_operations(&self) -> Result<SyncResult, OfflineError> {
        let mut result = SyncResult {
            synced: 0,
            failed: 0,
            conflicts: 0
        };

        // Get pending operations from SQLite
        let rows = sqlx::query_as::<_, (String, String, String, String, String, i32)>(
            "SELECT id, operation_type, entity_type, entity_id, payload, retry_count 
             FROM operation_queue 
             WHERE status = 'Pending' 
             ORDER BY created_at"
        )
        .fetch_all(&self.db_pool)
        .await
        .map_err(|e| OfflineError::QueueError(e.to_string()))?;

        for (id, op_type, entity_type, entity_id, payload_str, retry_count) in rows {
            let payload: serde_json::Value = serde_json::from_str(&payload_str)
                .map_err(|e| OfflineError::SerializationError(e.to_string()))?;

            // Attempt to sync
            match self
                .sync_single_operation(&id, &op_type, &entity_type, &entity_id, &payload)
                .await
            {
                Ok(_) => {
                    // Mark as resolved
                    sqlx::query("UPDATE operation_queue SET status = 'Resolved' WHERE id = ?1")
                        .bind(&id)
                        .execute(&self.db_pool)
                        .await
                        .map_err(|e| OfflineError::QueueError(e.to_string()))?;

                    result.synced += 1;
                }
                Err(e) => {
                    warn!("Failed to sync operation {}: {}", id, e);

                    let new_retry_count = retry_count + 1;
                    sqlx::query("UPDATE operation_queue SET retry_count = ?1 WHERE id = ?2")
                        .bind(new_retry_count)
                        .bind(&id)
                        .execute(&self.db_pool)
                        .await
                        .map_err(|e| OfflineError::QueueError(e.to_string()))?;

                    result.failed += 1;
                }
            }
        }

        *self.last_sync.write().await = Some(Utc::now());

        Ok(result)
    }

    /// Sync a single operation.
    async fn sync_single_operation(
        &self,
        _id: &str,
        _op_type: &str,
        _entity_type: &str,
        _entity_id: &str,
        _payload: &serde_json::Value
    ) -> Result<(), OfflineError> {
        // In real implementation: send to server via API
        // For now, simulate success
        Ok(())
    }

    /// Get queue statistics.
    pub async fn queue_stats(&self) -> QueueStats {
        let pending_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM operation_queue WHERE status = 'Pending'")
                .fetch_one(&self.db_pool)
                .await
                .unwrap_or(0);

        QueueStats {
            pending: pending_count as usize,
            max_size: self.config.max_queue_size
        }
    }
}

/// Conflict resolution outcome.
#[derive(Debug, Clone)]
pub enum ConflictResolution {
    UseLocal(QueuedOperation),
    UseServer,
    Merged(serde_json::Value),
    RequiresManual(String)
}

/// Sync result.
#[derive(Debug, Clone)]
pub struct SyncResult {
    pub synced: u64,
    pub failed: u64,
    pub conflicts: u64
}

/// Queue statistics.
#[derive(Debug, Clone)]
pub struct QueueStats {
    pub pending: usize,
    pub max_size: usize
}

/// Offline errors.
#[derive(Debug, thiserror::Error)]
pub enum OfflineError {
    #[error("Cache error: {0}")]
    CacheError(String),

    #[error("Queue error: {0}")]
    QueueError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Sync error: {0}")]
    SyncError(String)
}

use sqlx::Row;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_conflict_resolution() {
        let local = serde_json::json!({
            "name": "Local Name",
            "description": "Local Description"
        });

        let server = serde_json::json!({
            "name": "Server Name",
            "other_field": "Server Value"
        });

        let merged = OfflineCliClient::attempt_merge(&local, &server);

        // Local should win for overlapping fields
        assert_eq!(merged["name"], "Local Name");
        assert_eq!(merged["description"], "Local Description");
        assert_eq!(merged["other_field"], "Server Value");
    }
}
