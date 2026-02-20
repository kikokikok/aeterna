//! # Cold-Tier Storage (§4.2)
//!
//! Tiered storage implementation for memory archival to S3.
//!
//! ## Tier Policy
//! - **Hot** (Redis):     < 7 days,   frequently accessed  — managed by existing embedding cache
//! - **Warm** (Postgres): 7–90 days,  occasionally accessed — managed by existing backend
//! - **Cold** (S3/JSON):  > 90 days,  rarely accessed       — implemented here
//!
//! ## S3 Key Pattern
//! `{tenant_id}/memories/{year}/{month:02}/{batch_id}.json`
//!
//! Cold archives are stored as JSON arrays (Parquet is noted as a future migration path once
//! a native Rust Parquet library is added to the memory crate dependencies).

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Datelike, Utc};
use mk_core::types::{MemoryEntry, TenantContext};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum ColdTierError {
    #[error("S3 upload failed for key '{key}': {reason}")]
    UploadFailed { key: String, reason: String },

    #[error("S3 download failed for key '{key}': {reason}")]
    DownloadFailed { key: String, reason: String },

    #[error("S3 list failed for prefix '{prefix}': {reason}")]
    ListFailed { prefix: String, reason: String },

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("No cold-tier S3 client configured")]
    NotConfigured,
}

/// S3 abstraction for cold-tier archival.
#[async_trait]
pub trait ColdTierS3Client: Send + Sync {
    async fn upload_object(
        &self,
        bucket: &str,
        key: &str,
        data: Vec<u8>,
    ) -> Result<(), ColdTierError>;

    async fn download_object(&self, bucket: &str, key: &str) -> Result<Vec<u8>, ColdTierError>;

    async fn list_objects(&self, bucket: &str, prefix: &str) -> Result<Vec<String>, ColdTierError>;

    async fn delete_object(&self, bucket: &str, key: &str) -> Result<(), ColdTierError>;
}

/// Cold-tier configuration.
#[derive(Debug, Clone)]
pub struct ColdTierConfig {
    /// S3 bucket used for cold storage.
    pub bucket: String,

    /// Age threshold (days) above which memories are archived to cold tier.
    pub cold_threshold_days: u64,

    /// Maximum number of entries per archive batch (bundle).
    pub batch_size: usize,

    /// Whether archival is enabled at all.
    pub enabled: bool,
}

impl Default for ColdTierConfig {
    fn default() -> Self {
        Self {
            bucket: "aeterna-cold-storage".to_string(),
            cold_threshold_days: 90,
            batch_size: 500,
            enabled: true,
        }
    }
}

/// Metadata for a single cold-tier archive object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColdArchiveMetadata {
    /// S3 key of the archived file.
    pub s3_key: String,
    /// Tenant that owns these memories.
    pub tenant_id: String,
    /// Number of memory entries in the archive.
    pub entry_count: usize,
    /// Uncompressed JSON size in bytes.
    pub size_bytes: usize,
    /// Timestamp when this archive was created.
    pub archived_at: DateTime<Utc>,
    /// Earliest `created_at` value (Unix seconds) among archived entries.
    pub oldest_entry_ts: i64,
    /// Latest `created_at` value among archived entries.
    pub newest_entry_ts: i64,
}

/// In-memory index of all cold archives.  
/// Rebuilt on startup by scanning S3 (`list_objects`).
#[derive(Debug, Default, Clone)]
pub struct ColdArchiveIndex {
    /// Map from tenant_id → list of archive metadata.
    pub by_tenant: HashMap<String, Vec<ColdArchiveMetadata>>,
    pub total_archived_entries: u64,
    pub total_archived_bytes: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct ArchiveBatch {
    schema_version: u8,
    tenant_id: String,
    archived_at: DateTime<Utc>,
    entries: Vec<MemoryEntry>,
}

/// Manages cold-tier archival of memory entries to S3.
pub struct ColdTierManager {
    config: ColdTierConfig,
    s3: Option<Arc<dyn ColdTierS3Client>>,
    index: Arc<RwLock<ColdArchiveIndex>>,
}

impl ColdTierManager {
    /// Create a new manager.  
    /// `s3` may be `None` if cold-tier is disabled or not yet configured.
    pub fn new(config: ColdTierConfig, s3: Option<Arc<dyn ColdTierS3Client>>) -> Self {
        Self {
            config,
            s3,
            index: Arc::new(RwLock::new(ColdArchiveIndex::default())),
        }
    }

    fn make_s3_key(tenant_id: &str, now: &DateTime<Utc>, batch_id: &str) -> String {
        format!(
            "{}/memories/{}/{:02}/{}.json",
            tenant_id,
            now.year(),
            now.month(),
            batch_id
        )
    }

    fn tenant_prefix(tenant_id: &str) -> String {
        format!("{}/memories/", tenant_id)
    }

    pub async fn archive_entries(
        &self,
        ctx: &TenantContext,
        entries: Vec<MemoryEntry>,
    ) -> Result<Vec<String>, ColdTierError> {
        if !self.config.enabled {
            debug!("Cold-tier archival is disabled; skipping.");
            return Ok(vec![]);
        }

        let s3 = self.s3.as_ref().ok_or(ColdTierError::NotConfigured)?;

        if entries.is_empty() {
            return Ok(vec![]);
        }

        let now = Utc::now();
        let tenant_id = ctx.tenant_id.as_str();
        let mut written_keys = Vec::new();

        for chunk in entries.chunks(self.config.batch_size) {
            let batch_id = Uuid::new_v4().to_string();
            let key = Self::make_s3_key(tenant_id, &now, &batch_id);

            let oldest = chunk.iter().map(|e| e.created_at).min().unwrap_or(0);
            let newest = chunk.iter().map(|e| e.created_at).max().unwrap_or(0);

            let batch = ArchiveBatch {
                schema_version: 1,
                tenant_id: tenant_id.to_string(),
                archived_at: now,
                entries: chunk.to_vec(),
            };

            let json = serde_json::to_vec(&batch)?;
            let size_bytes = json.len();

            info!(
                tenant_id = %tenant_id,
                key = %key,
                entry_count = chunk.len(),
                size_bytes,
                "Archiving memory batch to cold tier"
            );

            s3.upload_object(&self.config.bucket, &key, json).await?;

            let meta = ColdArchiveMetadata {
                s3_key: key.clone(),
                tenant_id: tenant_id.to_string(),
                entry_count: chunk.len(),
                size_bytes,
                archived_at: now,
                oldest_entry_ts: oldest,
                newest_entry_ts: newest,
            };

            let mut idx = self.index.write().await;
            idx.by_tenant
                .entry(tenant_id.to_string())
                .or_default()
                .push(meta);
            idx.total_archived_entries += chunk.len() as u64;
            idx.total_archived_bytes += size_bytes as u64;
            drop(idx);

            written_keys.push(key);
        }

        Ok(written_keys)
    }

    /// Retrieve all archived memory entries for a tenant from S3.
    ///
    /// Lists all objects under the tenant prefix, downloads each, deserializes,
    /// and returns the flattened `Vec<MemoryEntry>`.
    pub async fn restore_entries(
        &self,
        ctx: &TenantContext,
    ) -> Result<Vec<MemoryEntry>, ColdTierError> {
        let s3 = self.s3.as_ref().ok_or(ColdTierError::NotConfigured)?;
        let tenant_id = ctx.tenant_id.as_str();
        let prefix = Self::tenant_prefix(tenant_id);

        let keys = s3
            .list_objects(&self.config.bucket, &prefix)
            .await
            .map_err(|e| ColdTierError::ListFailed {
                prefix: prefix.clone(),
                reason: e.to_string(),
            })?;

        let mut all_entries = Vec::new();

        for key in &keys {
            let data = s3
                .download_object(&self.config.bucket, key)
                .await
                .map_err(|e| ColdTierError::DownloadFailed {
                    key: key.clone(),
                    reason: e.to_string(),
                })?;

            let batch: ArchiveBatch = serde_json::from_slice(&data)?;
            all_entries.extend(batch.entries);
        }

        Ok(all_entries)
    }

    /// Delete all cold-tier archives for a given tenant (GDPR right-to-be-forgotten).
    ///
    /// Lists and deletes every S3 object under `{tenant_id}/memories/`.
    /// Returns the number of objects deleted.
    pub async fn delete_tenant_archives(&self, ctx: &TenantContext) -> Result<u64, ColdTierError> {
        let s3 = self.s3.as_ref().ok_or(ColdTierError::NotConfigured)?;
        let tenant_id = ctx.tenant_id.as_str();
        let prefix = Self::tenant_prefix(tenant_id);

        let keys = s3
            .list_objects(&self.config.bucket, &prefix)
            .await
            .map_err(|e| ColdTierError::ListFailed {
                prefix: prefix.clone(),
                reason: e.to_string(),
            })?;

        let count = keys.len() as u64;

        for key in &keys {
            s3.delete_object(&self.config.bucket, key)
                .await
                .map_err(|e| ColdTierError::UploadFailed {
                    key: key.clone(),
                    reason: e.to_string(),
                })?;
            debug!(key = %key, "Deleted cold-tier archive for GDPR");
        }

        let mut idx = self.index.write().await;
        if let Some(archives) = idx.by_tenant.remove(tenant_id) {
            let removed_entries: u64 = archives.iter().map(|a| a.entry_count as u64).sum();
            let removed_bytes: u64 = archives.iter().map(|a| a.size_bytes as u64).sum();
            idx.total_archived_entries = idx.total_archived_entries.saturating_sub(removed_entries);
            idx.total_archived_bytes = idx.total_archived_bytes.saturating_sub(removed_bytes);
        }

        if count > 0 {
            info!(tenant_id = %tenant_id, deleted = count, "Deleted cold-tier archives (GDPR)");
        }

        Ok(count)
    }

    /// Determine whether a `MemoryEntry` should be moved to cold tier.
    ///
    /// Returns `true` if `created_at` is older than `cold_threshold_days`.
    pub fn should_archive(&self, entry: &MemoryEntry) -> bool {
        let threshold_secs = self.config.cold_threshold_days as i64 * 86_400;
        let now = Utc::now().timestamp();
        now - entry.created_at > threshold_secs
    }

    /// Return a snapshot of the current archive index.
    pub async fn index_snapshot(&self) -> ColdArchiveIndex {
        self.index.read().await.clone()
    }

    /// Rebuild the in-memory index by scanning S3 for a specific tenant.
    ///
    /// Used on startup or when the index may be stale.
    pub async fn rebuild_index_for_tenant(&self, ctx: &TenantContext) -> Result<(), ColdTierError> {
        let s3 = self.s3.as_ref().ok_or(ColdTierError::NotConfigured)?;
        let tenant_id = ctx.tenant_id.as_str();
        let prefix = Self::tenant_prefix(tenant_id);

        let keys = s3
            .list_objects(&self.config.bucket, &prefix)
            .await
            .map_err(|e| ColdTierError::ListFailed {
                prefix: prefix.clone(),
                reason: e.to_string(),
            })?;

        let mut archives = Vec::new();
        let mut total_entries = 0u64;
        let mut total_bytes = 0u64;

        for key in keys {
            let data = s3
                .download_object(&self.config.bucket, &key)
                .await
                .map_err(|e| ColdTierError::DownloadFailed {
                    key: key.clone(),
                    reason: e.to_string(),
                })?;

            let size_bytes = data.len();
            let batch: ArchiveBatch = serde_json::from_slice(&data)?;
            let entry_count = batch.entries.len();
            let oldest = batch
                .entries
                .iter()
                .map(|e| e.created_at)
                .min()
                .unwrap_or(0);
            let newest = batch
                .entries
                .iter()
                .map(|e| e.created_at)
                .max()
                .unwrap_or(0);

            total_entries += entry_count as u64;
            total_bytes += size_bytes as u64;

            archives.push(ColdArchiveMetadata {
                s3_key: key,
                tenant_id: tenant_id.to_string(),
                entry_count,
                size_bytes,
                archived_at: batch.archived_at,
                oldest_entry_ts: oldest,
                newest_entry_ts: newest,
            });
        }

        let mut idx = self.index.write().await;
        idx.total_archived_entries = idx.total_archived_entries.saturating_sub(
            idx.by_tenant
                .get(tenant_id)
                .map(|a| a.iter().map(|m| m.entry_count as u64).sum())
                .unwrap_or(0),
        ) + total_entries;
        idx.total_archived_bytes = idx.total_archived_bytes.saturating_sub(
            idx.by_tenant
                .get(tenant_id)
                .map(|a| a.iter().map(|m| m.size_bytes as u64).sum())
                .unwrap_or(0),
        ) + total_bytes;
        idx.by_tenant.insert(tenant_id.to_string(), archives);

        warn!(
            tenant_id = %tenant_id,
            archives = idx.by_tenant.get(tenant_id).map(|a| a.len()).unwrap_or(0),
            "Rebuilt cold-tier index for tenant"
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    struct MockS3 {
        store: Mutex<HashMap<String, Vec<u8>>>,
    }

    #[async_trait]
    impl ColdTierS3Client for MockS3 {
        async fn upload_object(
            &self,
            _bucket: &str,
            key: &str,
            data: Vec<u8>,
        ) -> Result<(), ColdTierError> {
            self.store.lock().unwrap().insert(key.to_string(), data);
            Ok(())
        }

        async fn download_object(
            &self,
            _bucket: &str,
            key: &str,
        ) -> Result<Vec<u8>, ColdTierError> {
            self.store.lock().unwrap().get(key).cloned().ok_or_else(|| {
                ColdTierError::DownloadFailed {
                    key: key.to_string(),
                    reason: "not found".to_string(),
                }
            })
        }

        async fn list_objects(
            &self,
            _bucket: &str,
            prefix: &str,
        ) -> Result<Vec<String>, ColdTierError> {
            let keys = self
                .store
                .lock()
                .unwrap()
                .keys()
                .filter(|k| k.starts_with(prefix))
                .cloned()
                .collect();
            Ok(keys)
        }

        async fn delete_object(&self, _bucket: &str, key: &str) -> Result<(), ColdTierError> {
            self.store.lock().unwrap().remove(key);
            Ok(())
        }
    }

    fn make_ctx(tenant: &str) -> TenantContext {
        TenantContext {
            tenant_id: mk_core::types::TenantId::new(tenant.to_string()).unwrap(),
            ..Default::default()
        }
    }

    fn old_entry(id: &str) -> MemoryEntry {
        let ts = Utc::now().timestamp() - 100 * 86_400;
        MemoryEntry {
            id: id.to_string(),
            content: format!("content-{}", id),
            created_at: ts,
            updated_at: ts,
            ..Default::default()
        }
    }

    fn fresh_entry(id: &str) -> MemoryEntry {
        let ts = Utc::now().timestamp() - 86_400;
        MemoryEntry {
            id: id.to_string(),
            content: format!("content-{}", id),
            created_at: ts,
            updated_at: ts,
            ..Default::default()
        }
    }

    fn manager_with_mock() -> ColdTierManager {
        ColdTierManager::new(
            ColdTierConfig {
                batch_size: 2,
                ..Default::default()
            },
            Some(Arc::new(MockS3::default())),
        )
    }

    #[tokio::test]
    async fn test_archive_and_restore_round_trip() {
        let mgr = manager_with_mock();
        let ctx = make_ctx("tenant-1");
        let entries = vec![old_entry("m1"), old_entry("m2"), old_entry("m3")];

        let keys = mgr.archive_entries(&ctx, entries.clone()).await.unwrap();
        assert_eq!(keys.len(), 2);

        let restored = mgr.restore_entries(&ctx).await.unwrap();
        assert_eq!(restored.len(), 3);
        let mut ids: Vec<_> = restored.iter().map(|e| e.id.as_str()).collect();
        ids.sort_unstable();
        assert_eq!(ids, vec!["m1", "m2", "m3"]);
    }

    #[tokio::test]
    async fn test_index_updated_after_archive() {
        let mgr = manager_with_mock();
        let ctx = make_ctx("tenant-2");
        let entries = vec![old_entry("a"), old_entry("b")];

        mgr.archive_entries(&ctx, entries).await.unwrap();

        let idx = mgr.index_snapshot().await;
        assert_eq!(idx.total_archived_entries, 2);
        let tenant_archives = idx.by_tenant.get("tenant-2").unwrap();
        assert_eq!(tenant_archives.len(), 1);
    }

    #[tokio::test]
    async fn test_delete_tenant_archives() {
        let mgr = manager_with_mock();
        let ctx = make_ctx("tenant-gdpr");
        let entries = vec![old_entry("x"), old_entry("y")];

        mgr.archive_entries(&ctx, entries).await.unwrap();
        let deleted = mgr.delete_tenant_archives(&ctx).await.unwrap();
        assert_eq!(deleted, 1);

        let restored = mgr.restore_entries(&ctx).await.unwrap();
        assert!(restored.is_empty());

        let idx = mgr.index_snapshot().await;
        assert_eq!(idx.total_archived_entries, 0);
    }

    #[tokio::test]
    async fn test_should_archive_respects_threshold() {
        let mgr = ColdTierManager::new(ColdTierConfig::default(), None);

        let old = old_entry("old");
        let fresh = fresh_entry("fresh");

        assert!(mgr.should_archive(&old));
        assert!(!mgr.should_archive(&fresh));
    }

    #[tokio::test]
    async fn test_no_s3_returns_not_configured() {
        let mgr = ColdTierManager::new(ColdTierConfig::default(), None);
        let ctx = make_ctx("tenant-x");
        let err = mgr.archive_entries(&ctx, vec![old_entry("e1")]).await;
        assert!(matches!(err, Err(ColdTierError::NotConfigured)));
    }

    #[tokio::test]
    async fn test_disabled_config_skips_archival() {
        let mgr = ColdTierManager::new(
            ColdTierConfig {
                enabled: false,
                ..Default::default()
            },
            Some(Arc::new(MockS3::default())),
        );
        let ctx = make_ctx("tenant-disabled");
        let keys = mgr
            .archive_entries(&ctx, vec![old_entry("skip")])
            .await
            .unwrap();
        assert!(keys.is_empty());
    }

    #[tokio::test]
    async fn test_empty_entries_returns_empty_keys() {
        let mgr = manager_with_mock();
        let ctx = make_ctx("tenant-empty");
        let keys = mgr.archive_entries(&ctx, vec![]).await.unwrap();
        assert!(keys.is_empty());
    }

    #[tokio::test]
    async fn test_s3_key_format() {
        let now = Utc::now();
        let key = ColdTierManager::make_s3_key("my-tenant", &now, "batch-123");
        assert!(key.starts_with("my-tenant/memories/"));
        assert!(key.ends_with("batch-123.json"));
    }
}
