use chrono::{DateTime, Utc};
/// Shard Router for Scalable Code Search Repository Management
///
/// This module provides consistent hashing and affinity routing to ensure
/// repository operations are always directed to the pod/container where
/// the repository clone resides.
///
/// Architecture:
/// - Each indexer pod registers itself with a unique `shard_id`
/// - Repositories are assigned to shards using consistent hashing
/// - On scale events, a rebalancing job reassigns and migrates repos
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use uuid::Uuid;

/// Shard information for an indexer pod
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct IndexerShard {
    pub shard_id: String,
    pub pod_name: String,
    pub pod_ip: String,
    pub capacity: i32,     // Max repos this shard can handle
    pub current_load: i32, // Current number of repos
    pub status: ShardStatus,
    pub last_heartbeat: DateTime<Utc>,
    pub registered_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(rename_all = "lowercase")]
pub enum ShardStatus {
    Active,
    Draining, // No new repos, migrating existing
    Offline,
    Maintenance,
}

/// Consistent hash ring for shard routing
pub struct ShardRouter {
    pool: PgPool,
    local_shard_id: Option<String>,
    virtual_nodes: i32, // Virtual nodes per shard for better distribution
}

impl ShardRouter {
    pub fn new(pool: PgPool, local_shard_id: Option<String>) -> Self {
        Self {
            pool,
            local_shard_id,
            virtual_nodes: 150, // Good balance for most deployments
        }
    }

    /// Register this pod as an indexer shard
    pub async fn register_shard(
        &self,
        shard_id: &str,
        pod_name: &str,
        pod_ip: &str,
        capacity: i32,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(r#"
            INSERT INTO codesearch_indexer_shards (shard_id, pod_name, pod_ip, capacity, current_load, status, last_heartbeat, registered_at)
            VALUES ($1, $2, $3, $4, 0, 'active', NOW(), NOW())
            ON CONFLICT (shard_id) DO UPDATE SET
                pod_name = EXCLUDED.pod_name,
                pod_ip = EXCLUDED.pod_ip,
                capacity = EXCLUDED.capacity,
                status = 'active',
                last_heartbeat = NOW()
        "#)
        .bind(shard_id)
        .bind(pod_name)
        .bind(pod_ip)
        .bind(capacity)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Send heartbeat to mark shard as alive
    pub async fn heartbeat(&self, shard_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE codesearch_indexer_shards SET last_heartbeat = NOW() WHERE shard_id = $1",
        )
        .bind(shard_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get all active shards
    pub async fn get_active_shards(&self) -> Result<Vec<IndexerShard>, sqlx::Error> {
        sqlx::query_as::<_, IndexerShard>(
            "SELECT * FROM codesearch_indexer_shards WHERE status = 'active' AND last_heartbeat > NOW() - INTERVAL '30 seconds'"
        )
        .fetch_all(&self.pool)
        .await
    }

    /// Assign a repository to an optimal shard using consistent hashing
    pub async fn assign_shard(&self, repo_id: Uuid) -> Result<String, ShardRoutingError> {
        let shards = self
            .get_active_shards()
            .await
            .map_err(|e| ShardRoutingError::DatabaseError(e.to_string()))?;

        if shards.is_empty() {
            return Err(ShardRoutingError::NoShardsAvailable);
        }

        // Build virtual node ring
        let mut ring: Vec<(u64, String)> = Vec::new();
        for shard in &shards {
            if shard.current_load >= shard.capacity {
                continue; // Skip full shards
            }
            for i in 0..self.virtual_nodes {
                let key = format!("{}:{}", shard.shard_id, i);
                let hash = self.hash_key(&key);
                ring.push((hash, shard.shard_id.clone()));
            }
        }

        if ring.is_empty() {
            return Err(ShardRoutingError::AllShardsAtCapacity);
        }

        ring.sort_by_key(|(h, _)| *h);

        // Find the shard for this repo
        let repo_hash = self.hash_key(&repo_id.to_string());
        let target_shard = ring
            .iter()
            .find(|(h, _)| *h >= repo_hash)
            .unwrap_or(&ring[0])
            .1
            .clone();

        // Update the repo's shard assignment and increment load
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| ShardRoutingError::DatabaseError(e.to_string()))?;

        sqlx::query("UPDATE codesearch_repositories SET shard_id = $1 WHERE id = $2")
            .bind(&target_shard)
            .bind(repo_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| ShardRoutingError::DatabaseError(e.to_string()))?;

        sqlx::query("UPDATE codesearch_indexer_shards SET current_load = current_load + 1 WHERE shard_id = $1")
            .bind(&target_shard)
            .execute(&mut *tx)
            .await
            .map_err(|e| ShardRoutingError::DatabaseError(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| ShardRoutingError::DatabaseError(e.to_string()))?;

        Ok(target_shard)
    }

    /// Get the shard for a repository (already assigned)
    pub async fn get_shard_for_repo(&self, repo_id: Uuid) -> Result<Option<String>, sqlx::Error> {
        let row: Option<(Option<String>,)> =
            sqlx::query_as("SELECT shard_id FROM codesearch_repositories WHERE id = $1")
                .bind(repo_id)
                .fetch_optional(&self.pool)
                .await?;

        Ok(row.and_then(|(s,)| s))
    }

    /// Check if this pod should handle the given repository
    pub async fn is_local(&self, repo_id: Uuid) -> Result<bool, sqlx::Error> {
        let assigned_shard = self.get_shard_for_repo(repo_id).await?;
        Ok(assigned_shard == self.local_shard_id)
    }

    /// Get routing info for a repository
    pub async fn get_routing_info(
        &self,
        repo_id: Uuid,
    ) -> Result<Option<IndexerShard>, sqlx::Error> {
        let shard_id = self.get_shard_for_repo(repo_id).await?;
        match shard_id {
            Some(id) => {
                sqlx::query_as::<_, IndexerShard>(
                    "SELECT * FROM codesearch_indexer_shards WHERE shard_id = $1",
                )
                .bind(id)
                .fetch_optional(&self.pool)
                .await
            }
            None => Ok(None),
        }
    }

    /// Mark a shard as draining (for graceful shutdown or scaling down)
    pub async fn drain_shard(&self, shard_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE codesearch_indexer_shards SET status = 'draining' WHERE shard_id = $1")
            .bind(shard_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Rebalance repos from a draining shard to active shards
    pub async fn rebalance_from_shard(
        &self,
        draining_shard_id: &str,
    ) -> Result<i32, ShardRoutingError> {
        let repos: Vec<(Uuid,)> =
            sqlx::query_as("SELECT id FROM codesearch_repositories WHERE shard_id = $1")
                .bind(draining_shard_id)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| ShardRoutingError::DatabaseError(e.to_string()))?;

        let mut migrated = 0;
        for (repo_id,) in repos {
            // Clear the current shard assignment
            sqlx::query("UPDATE codesearch_repositories SET shard_id = NULL WHERE id = $1")
                .bind(repo_id)
                .execute(&self.pool)
                .await
                .map_err(|e| ShardRoutingError::DatabaseError(e.to_string()))?;

            // Reassign to a new shard
            self.assign_shard(repo_id).await?;
            migrated += 1;
        }

        // Mark the old shard as offline
        sqlx::query("UPDATE codesearch_indexer_shards SET status = 'offline', current_load = 0 WHERE shard_id = $1")
            .bind(draining_shard_id)
            .execute(&self.pool)
            .await
            .map_err(|e| ShardRoutingError::DatabaseError(e.to_string()))?;

        Ok(migrated)
    }

    fn hash_key(&self, key: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        hasher.finish()
    }
}

/// Cold storage manager for S3/GCS backup
pub struct ColdStorageManager {
    s3_client: aws_sdk_s3::Client,
    bucket: String,
}

impl ColdStorageManager {
    pub async fn new(bucket: String) -> Self {
        let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
        let s3_client = aws_sdk_s3::Client::new(&config);
        Self { s3_client, bucket }
    }

    /// Backup a repository to S3 as a git bundle
    pub async fn backup_repo(
        &self,
        tenant_id: &str,
        repo_id: Uuid,
        local_path: &std::path::Path,
    ) -> Result<String, ColdStorageError> {
        let bundle_path = local_path.with_extension("bundle");
        let s3_key = format!("{}/{}.bundle", tenant_id, repo_id);

        // Create git bundle
        let output = std::process::Command::new("git")
            .args(["bundle", "create", bundle_path.to_str().unwrap(), "--all"])
            .current_dir(local_path)
            .output()
            .map_err(|e| ColdStorageError::BundleFailed(e.to_string()))?;

        if !output.status.success() {
            return Err(ColdStorageError::BundleFailed(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }

        // Upload to S3
        let body = aws_sdk_s3::primitives::ByteStream::from_path(&bundle_path)
            .await
            .map_err(|e| ColdStorageError::UploadFailed(e.to_string()))?;

        self.s3_client
            .put_object()
            .bucket(&self.bucket)
            .key(&s3_key)
            .body(body)
            .send()
            .await
            .map_err(|e| ColdStorageError::UploadFailed(e.to_string()))?;

        // Cleanup local bundle
        std::fs::remove_file(&bundle_path).ok();

        Ok(format!("s3://{}/{}", self.bucket, s3_key))
    }

    /// Restore a repository from S3 bundle
    pub async fn restore_repo(
        &self,
        s3_uri: &str,
        local_path: &std::path::Path,
    ) -> Result<(), ColdStorageError> {
        // Parse S3 URI
        let key = s3_uri
            .strip_prefix(&format!("s3://{}/", self.bucket))
            .ok_or_else(|| ColdStorageError::InvalidUri(s3_uri.to_string()))?;

        // Download bundle
        let bundle_path = local_path.with_extension("bundle");
        let resp = self
            .s3_client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| ColdStorageError::DownloadFailed(e.to_string()))?;

        let data = resp
            .body
            .collect()
            .await
            .map_err(|e| ColdStorageError::DownloadFailed(e.to_string()))?;

        std::fs::write(&bundle_path, data.into_bytes())
            .map_err(|e| ColdStorageError::DownloadFailed(e.to_string()))?;

        // Clone from bundle
        std::fs::create_dir_all(local_path)
            .map_err(|e| ColdStorageError::RestoreFailed(e.to_string()))?;

        let output = std::process::Command::new("git")
            .args(["clone", bundle_path.to_str().unwrap(), "."])
            .current_dir(local_path)
            .output()
            .map_err(|e| ColdStorageError::RestoreFailed(e.to_string()))?;

        if !output.status.success() {
            return Err(ColdStorageError::RestoreFailed(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }

        // Cleanup bundle
        std::fs::remove_file(&bundle_path).ok();

        Ok(())
    }

    /// Delete a backup from S3
    pub async fn delete_backup(&self, s3_uri: &str) -> Result<(), ColdStorageError> {
        let key = s3_uri
            .strip_prefix(&format!("s3://{}/", self.bucket))
            .ok_or_else(|| ColdStorageError::InvalidUri(s3_uri.to_string()))?;

        self.s3_client
            .delete_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| ColdStorageError::DeleteFailed(e.to_string()))?;

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ShardRoutingError {
    #[error("No shards available")]
    NoShardsAvailable,

    #[error("All shards at capacity")]
    AllShardsAtCapacity,

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Shard not found: {0}")]
    ShardNotFound(String),
}

#[derive(Debug, thiserror::Error)]
pub enum ColdStorageError {
    #[error("Failed to create bundle: {0}")]
    BundleFailed(String),

    #[error("Failed to upload to S3: {0}")]
    UploadFailed(String),

    #[error("Failed to download from S3: {0}")]
    DownloadFailed(String),

    #[error("Failed to restore repository: {0}")]
    RestoreFailed(String),

    #[error("Failed to delete backup: {0}")]
    DeleteFailed(String),

    #[error("Invalid S3 URI: {0}")]
    InvalidUri(String),
}
