//! Dead-letter queue for permanently failed sync operations.
//!
//! Items that exceed their retry limit are moved here for manual inspection,
//! retry, or permanent discard.

use std::collections::HashMap;
use std::sync::LazyLock;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;

/// Status of a dead-letter item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeadLetterStatus {
    /// Item is waiting for manual intervention.
    Active,
    /// Item is currently being retried.
    Retrying,
    /// Item has been permanently discarded.
    Discarded,
}

/// A single item in the dead-letter queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeadLetterItem {
    /// Unique identifier for this dead-letter entry.
    pub id: String,
    /// Category of the failed item (e.g. `"sync_entry"`, `"promotion"`, `"federation"`).
    pub item_type: String,
    /// The original entity ID that failed processing.
    pub item_id: String,
    /// Tenant that owns this item.
    pub tenant_id: String,
    /// Human-readable error description.
    pub error: String,
    /// Number of retries attempted before moving to the dead-letter queue.
    pub retry_count: u32,
    /// Maximum retries that were configured.
    pub max_retries: u32,
    /// Unix timestamp (seconds) of the first failure.
    pub first_failed_at: i64,
    /// Unix timestamp (seconds) of the most recent failure.
    pub last_failed_at: i64,
    /// Current status of this dead-letter item.
    pub status: DeadLetterStatus,
    /// Arbitrary metadata attached to the failed item.
    pub metadata: serde_json::Value,
}

/// In-memory dead-letter queue for failed sync operations.
///
/// Provides thread-safe access to dead-letter items via an async `RwLock`.
/// A single global instance is available through [`DeadLetterQueue::global`].
pub struct DeadLetterQueue {
    items: RwLock<HashMap<String, DeadLetterItem>>,
}

static DEAD_LETTER: LazyLock<DeadLetterQueue> = LazyLock::new(DeadLetterQueue::new);

impl DeadLetterQueue {
    /// Create a new empty dead-letter queue.
    pub fn new() -> Self {
        Self {
            items: RwLock::new(HashMap::new()),
        }
    }

    /// Return the process-wide singleton dead-letter queue.
    pub fn global() -> &'static Self {
        &DEAD_LETTER
    }

    /// Add a failed item to the dead-letter queue.
    ///
    /// Returns the generated unique ID for the new entry.
    pub async fn add(
        &self,
        item_type: &str,
        item_id: &str,
        tenant_id: &str,
        error: &str,
        retry_count: u32,
        max_retries: u32,
    ) -> String {
        let now = Utc::now().timestamp();
        let id = Uuid::new_v4().to_string();
        let item = DeadLetterItem {
            id: id.clone(),
            item_type: item_type.to_string(),
            item_id: item_id.to_string(),
            tenant_id: tenant_id.to_string(),
            error: error.to_string(),
            retry_count,
            max_retries,
            first_failed_at: now,
            last_failed_at: now,
            status: DeadLetterStatus::Active,
            metadata: serde_json::Value::Null,
        };
        self.items.write().await.insert(id.clone(), item);
        id
    }

    /// Get a single item by ID.
    pub async fn get(&self, id: &str) -> Option<DeadLetterItem> {
        self.items.read().await.get(id).cloned()
    }

    /// List all active items, optionally filtered by tenant.
    pub async fn list_active(&self, tenant_id: Option<&str>) -> Vec<DeadLetterItem> {
        let items = self.items.read().await;
        items
            .values()
            .filter(|item| item.status == DeadLetterStatus::Active)
            .filter(|item| {
                tenant_id
                    .map(|tid| item.tenant_id == tid)
                    .unwrap_or(true)
            })
            .cloned()
            .collect()
    }

    /// List all items regardless of status.
    pub async fn list_all(&self) -> Vec<DeadLetterItem> {
        self.items.read().await.values().cloned().collect()
    }

    /// Mark an item for retry (sets status to `Retrying`).
    pub async fn mark_retrying(&self, id: &str) -> anyhow::Result<()> {
        let mut items = self.items.write().await;
        let item = items
            .get_mut(id)
            .ok_or_else(|| anyhow::anyhow!("Dead-letter item not found: {id}"))?;
        item.status = DeadLetterStatus::Retrying;
        Ok(())
    }

    /// Report the result of a retry attempt.
    ///
    /// If `success` is true the item is removed from the queue. Otherwise
    /// the error is updated and the item is set back to `Active`.
    pub async fn retry_result(
        &self,
        id: &str,
        success: bool,
        error: Option<String>,
    ) -> anyhow::Result<()> {
        let mut items = self.items.write().await;
        if success {
            items
                .remove(id)
                .ok_or_else(|| anyhow::anyhow!("Dead-letter item not found: {id}"))?;
            return Ok(());
        }
        let item = items
            .get_mut(id)
            .ok_or_else(|| anyhow::anyhow!("Dead-letter item not found: {id}"))?;
        item.status = DeadLetterStatus::Active;
        item.retry_count += 1;
        item.last_failed_at = Utc::now().timestamp();
        if let Some(err) = error {
            item.error = err;
        }
        Ok(())
    }

    /// Discard an item permanently (sets status to `Discarded`).
    pub async fn discard(&self, id: &str) -> anyhow::Result<()> {
        let mut items = self.items.write().await;
        let item = items
            .get_mut(id)
            .ok_or_else(|| anyhow::anyhow!("Dead-letter item not found: {id}"))?;
        item.status = DeadLetterStatus::Discarded;
        Ok(())
    }

    /// Count active items.
    pub async fn active_count(&self) -> usize {
        self.items
            .read()
            .await
            .values()
            .filter(|item| item.status == DeadLetterStatus::Active)
            .count()
    }

    /// Remove discarded items older than `max_age_secs` seconds.
    ///
    /// Returns the number of items removed.
    pub async fn cleanup_discarded(&self, max_age_secs: i64) -> usize {
        let now = Utc::now().timestamp();
        let mut items = self.items.write().await;
        let before = items.len();
        items.retain(|_, item| {
            !(item.status == DeadLetterStatus::Discarded
                && (now - item.last_failed_at) > max_age_secs)
        });
        before - items.len()
    }
}

impl Default for DeadLetterQueue {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Redis-backed dead-letter queue for multi-instance deployments
// ---------------------------------------------------------------------------

/// Redis-backed dead-letter queue for HA / multi-instance deployments.
///
/// Uses [`crate::redis_store::RedisStore`] with prefix `aeterna:dead_letter`.
pub struct RedisDeadLetterQueue {
    store: crate::redis_store::RedisStore,
}

impl RedisDeadLetterQueue {
    /// Create a new Redis-backed dead-letter queue.
    pub fn new(conn: std::sync::Arc<redis::aio::ConnectionManager>) -> Self {
        Self {
            store: crate::redis_store::RedisStore::new(conn, "aeterna:dead_letter"),
        }
    }

    /// Add a failed item to the dead-letter queue.
    ///
    /// Returns the generated unique ID for the new entry.
    pub async fn add(
        &self,
        item_type: &str,
        item_id: &str,
        tenant_id: &str,
        error: &str,
        retry_count: u32,
        max_retries: u32,
    ) -> anyhow::Result<String> {
        let now = Utc::now().timestamp();
        let id = Uuid::new_v4().to_string();
        let item = DeadLetterItem {
            id: id.clone(),
            item_type: item_type.to_string(),
            item_id: item_id.to_string(),
            tenant_id: tenant_id.to_string(),
            error: error.to_string(),
            retry_count,
            max_retries,
            first_failed_at: now,
            last_failed_at: now,
            status: DeadLetterStatus::Active,
            metadata: serde_json::Value::Null,
        };
        self.store.set(&id, &item, None).await?;
        Ok(id)
    }

    /// Get a single item by ID.
    pub async fn get(&self, id: &str) -> anyhow::Result<Option<DeadLetterItem>> {
        self.store.get(id).await
    }

    /// List all active items, optionally filtered by tenant.
    pub async fn list_active(
        &self,
        tenant_id: Option<&str>,
    ) -> anyhow::Result<Vec<DeadLetterItem>> {
        let all: Vec<DeadLetterItem> = self.store.list_all().await?;
        Ok(all
            .into_iter()
            .filter(|item| item.status == DeadLetterStatus::Active)
            .filter(|item| {
                tenant_id
                    .map(|tid| item.tenant_id == tid)
                    .unwrap_or(true)
            })
            .collect())
    }

    /// List all items regardless of status.
    pub async fn list_all(&self) -> anyhow::Result<Vec<DeadLetterItem>> {
        self.store.list_all().await
    }

    /// Mark an item for retry (sets status to `Retrying`).
    pub async fn mark_retrying(&self, id: &str) -> anyhow::Result<()> {
        let mut item = self
            .store
            .get::<DeadLetterItem>(id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Dead-letter item not found: {id}"))?;
        item.status = DeadLetterStatus::Retrying;
        self.store.set(id, &item, None).await
    }

    /// Report the result of a retry attempt.
    ///
    /// If `success` is true the item is removed from the queue. Otherwise
    /// the error is updated and the item is set back to `Active`.
    pub async fn retry_result(
        &self,
        id: &str,
        success: bool,
        error: Option<String>,
    ) -> anyhow::Result<()> {
        if success {
            let existed = self.store.delete(id).await?;
            if !existed {
                return Err(anyhow::anyhow!("Dead-letter item not found: {id}"));
            }
            return Ok(());
        }
        let mut item = self
            .store
            .get::<DeadLetterItem>(id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Dead-letter item not found: {id}"))?;
        item.status = DeadLetterStatus::Active;
        item.retry_count += 1;
        item.last_failed_at = Utc::now().timestamp();
        if let Some(err) = error {
            item.error = err;
        }
        self.store.set(id, &item, None).await
    }

    /// Discard an item permanently (sets status to `Discarded`).
    pub async fn discard(&self, id: &str) -> anyhow::Result<()> {
        let mut item = self
            .store
            .get::<DeadLetterItem>(id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Dead-letter item not found: {id}"))?;
        item.status = DeadLetterStatus::Discarded;
        self.store.set(id, &item, None).await
    }

    /// Count active items.
    pub async fn active_count(&self) -> anyhow::Result<usize> {
        let all: Vec<DeadLetterItem> = self.store.list_all().await?;
        Ok(all
            .iter()
            .filter(|item| item.status == DeadLetterStatus::Active)
            .count())
    }

    /// Remove discarded items older than `max_age_secs` seconds.
    ///
    /// Returns the number of items removed.
    pub async fn cleanup_discarded(&self, max_age_secs: i64) -> anyhow::Result<usize> {
        let now = Utc::now().timestamp();
        let all: Vec<DeadLetterItem> = self.store.list_all().await?;
        let mut count = 0;
        for item in all {
            if item.status == DeadLetterStatus::Discarded
                && (now - item.last_failed_at) > max_age_secs
            {
                self.store.delete(&item.id).await?;
                count += 1;
            }
        }
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_queue() -> DeadLetterQueue {
        DeadLetterQueue::new()
    }

    #[tokio::test]
    async fn add_and_get_item() {
        let q = new_queue();
        let id = q
            .add("sync_entry", "e1", "tenant1", "timeout", 3, 3)
            .await;
        let item = q.get(&id).await.expect("item should exist");
        assert_eq!(item.item_type, "sync_entry");
        assert_eq!(item.item_id, "e1");
        assert_eq!(item.tenant_id, "tenant1");
        assert_eq!(item.error, "timeout");
        assert_eq!(item.retry_count, 3);
        assert_eq!(item.max_retries, 3);
        assert_eq!(item.status, DeadLetterStatus::Active);
    }

    #[tokio::test]
    async fn get_missing_returns_none() {
        let q = new_queue();
        assert!(q.get("nonexistent").await.is_none());
    }

    #[tokio::test]
    async fn list_active_filters_correctly() {
        let q = new_queue();
        let id1 = q.add("sync_entry", "e1", "t1", "err", 3, 3).await;
        let _id2 = q.add("promotion", "e2", "t2", "err", 3, 3).await;
        q.discard(&id1).await.unwrap();

        let active = q.list_active(None).await;
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].item_type, "promotion");
    }

    #[tokio::test]
    async fn list_active_filters_by_tenant() {
        let q = new_queue();
        q.add("sync_entry", "e1", "t1", "err", 3, 3).await;
        q.add("sync_entry", "e2", "t2", "err", 3, 3).await;

        let t1_items = q.list_active(Some("t1")).await;
        assert_eq!(t1_items.len(), 1);
        assert_eq!(t1_items[0].tenant_id, "t1");
    }

    #[tokio::test]
    async fn mark_retrying_and_retry_success_removes_item() {
        let q = new_queue();
        let id = q.add("sync_entry", "e1", "t1", "err", 3, 3).await;

        q.mark_retrying(&id).await.unwrap();
        let item = q.get(&id).await.unwrap();
        assert_eq!(item.status, DeadLetterStatus::Retrying);

        q.retry_result(&id, true, None).await.unwrap();
        assert!(q.get(&id).await.is_none());
    }

    #[tokio::test]
    async fn retry_failure_updates_error_and_count() {
        let q = new_queue();
        let id = q.add("sync_entry", "e1", "t1", "old error", 3, 3).await;

        q.mark_retrying(&id).await.unwrap();
        q.retry_result(&id, false, Some("new error".to_string()))
            .await
            .unwrap();

        let item = q.get(&id).await.unwrap();
        assert_eq!(item.status, DeadLetterStatus::Active);
        assert_eq!(item.retry_count, 4);
        assert_eq!(item.error, "new error");
    }

    #[tokio::test]
    async fn discard_sets_status() {
        let q = new_queue();
        let id = q.add("sync_entry", "e1", "t1", "err", 3, 3).await;
        q.discard(&id).await.unwrap();

        let item = q.get(&id).await.unwrap();
        assert_eq!(item.status, DeadLetterStatus::Discarded);
    }

    #[tokio::test]
    async fn active_count_excludes_non_active() {
        let q = new_queue();
        let id1 = q.add("a", "1", "t", "e", 0, 3).await;
        q.add("b", "2", "t", "e", 0, 3).await;
        q.discard(&id1).await.unwrap();

        assert_eq!(q.active_count().await, 1);
    }

    #[tokio::test]
    async fn cleanup_discarded_removes_old_items() {
        let q = new_queue();
        let id = q.add("a", "1", "t", "e", 0, 3).await;
        q.discard(&id).await.unwrap();

        // Force the last_failed_at to be old
        {
            let mut items = q.items.write().await;
            if let Some(item) = items.get_mut(&id) {
                item.last_failed_at = Utc::now().timestamp() - 10000;
            }
        }

        let removed = q.cleanup_discarded(100).await;
        assert_eq!(removed, 1);
        assert!(q.get(&id).await.is_none());
    }

    #[tokio::test]
    async fn cleanup_discarded_keeps_recent_items() {
        let q = new_queue();
        let id = q.add("a", "1", "t", "e", 0, 3).await;
        q.discard(&id).await.unwrap();

        let removed = q.cleanup_discarded(999_999).await;
        assert_eq!(removed, 0);
        assert!(q.get(&id).await.is_some());
    }

    #[tokio::test]
    async fn list_all_returns_every_status() {
        let q = new_queue();
        let id1 = q.add("a", "1", "t", "e", 0, 3).await;
        q.add("b", "2", "t", "e", 0, 3).await;
        q.discard(&id1).await.unwrap();

        assert_eq!(q.list_all().await.len(), 2);
    }

    #[tokio::test]
    async fn mark_retrying_missing_item_errors() {
        let q = new_queue();
        assert!(q.mark_retrying("nope").await.is_err());
    }

    #[tokio::test]
    async fn discard_missing_item_errors() {
        let q = new_queue();
        assert!(q.discard("nope").await.is_err());
    }

    #[tokio::test]
    async fn retry_result_missing_item_errors() {
        let q = new_queue();
        assert!(q.retry_result("nope", true, None).await.is_err());
        assert!(q.retry_result("nope", false, None).await.is_err());
    }

    #[tokio::test]
    async fn serialization_roundtrip() {
        let q = new_queue();
        let id = q.add("federation", "e1", "t1", "err", 2, 5).await;
        let item = q.get(&id).await.unwrap();
        let json = serde_json::to_string(&item).unwrap();
        let deserialized: DeadLetterItem = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, item.id);
        assert_eq!(deserialized.item_type, "federation");
        assert_eq!(deserialized.status, DeadLetterStatus::Active);

        // Verify camelCase serialization
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(value.get("itemType").is_some());
        assert!(value.get("tenantId").is_some());
        assert!(value.get("retryCount").is_some());
        assert!(value.get("maxRetries").is_some());
        assert!(value.get("firstFailedAt").is_some());
        assert!(value.get("lastFailedAt").is_some());
    }
}
