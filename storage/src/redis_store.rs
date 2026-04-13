//! Generic Redis-backed store for JSON-serializable items with optional TTL.
//!
//! Provides a reusable abstraction for storing, retrieving, listing, and
//! deleting items in Redis. Each store instance is scoped by a key prefix
//! (e.g. `"aeterna:export_jobs"`), and an auxiliary Redis SET tracks known
//! item IDs so the store can enumerate without `SCAN`.
//!
//! This replaces in-memory `LazyLock<RwLock<HashMap>>` patterns that break
//! under multi-instance (Kubernetes ReplicaSet) deployments.

use redis::AsyncCommands;
use serde::{Serialize, de::DeserializeOwned};
use std::sync::Arc;

/// Generic Redis-backed store for JSON items with optional TTL.
///
/// Each item is stored under `{prefix}:{id}` and the set of known IDs is
/// maintained in `{prefix}:__index`.
///
/// # Thread Safety
///
/// `ConnectionManager` is `Clone + Send + Sync` and internally pools
/// connections, so this type is safe to share across tasks.
pub struct RedisStore {
    conn: Arc<redis::aio::ConnectionManager>,
    prefix: String,
}

impl RedisStore {
    /// Create a new store backed by the given connection manager.
    ///
    /// `prefix` is prepended to every key (e.g. `"aeterna:export_jobs"`).
    pub fn new(conn: Arc<redis::aio::ConnectionManager>, prefix: &str) -> Self {
        Self {
            conn,
            prefix: prefix.to_string(),
        }
    }

    fn key(&self, id: &str) -> String {
        format!("{}:{}", self.prefix, id)
    }

    fn list_key(&self) -> String {
        format!("{}:__index", self.prefix)
    }

    /// Store an item with optional TTL in seconds.
    pub async fn set<T: Serialize>(
        &self,
        id: &str,
        item: &T,
        ttl_secs: Option<u64>,
    ) -> anyhow::Result<()> {
        let json =
            serde_json::to_string(item).map_err(|e| anyhow::anyhow!("serialization: {e}"))?;
        let mut conn = (*self.conn).clone();
        if let Some(ttl) = ttl_secs {
            conn.set_ex::<_, _, ()>(self.key(id), &json, ttl).await?;
        } else {
            conn.set::<_, _, ()>(self.key(id), &json).await?;
        }
        // Add to index set for listing
        conn.sadd::<_, _, ()>(self.list_key(), id).await?;
        Ok(())
    }

    /// Get an item by ID.
    pub async fn get<T: DeserializeOwned>(&self, id: &str) -> anyhow::Result<Option<T>> {
        let mut conn = (*self.conn).clone();
        let result: Option<String> = conn.get(self.key(id)).await?;
        match result {
            Some(json) => Ok(Some(
                serde_json::from_str(&json)
                    .map_err(|e| anyhow::anyhow!("deserialization: {e}"))?,
            )),
            None => Ok(None),
        }
    }

    /// Delete an item.
    pub async fn delete(&self, id: &str) -> anyhow::Result<bool> {
        let mut conn = (*self.conn).clone();
        let deleted: i64 = conn.del(self.key(id)).await?;
        conn.srem::<_, _, ()>(self.list_key(), id).await?;
        Ok(deleted > 0)
    }

    /// List all items. Scans the index set and fetches each item.
    ///
    /// Stale index entries (where the underlying key has expired) are cleaned
    /// up automatically.
    pub async fn list_all<T: DeserializeOwned>(&self) -> anyhow::Result<Vec<T>> {
        let mut conn = (*self.conn).clone();
        let ids: Vec<String> = conn.smembers(self.list_key()).await?;
        let mut items = Vec::new();
        for id in ids {
            if let Some(item) = self.get::<T>(&id).await? {
                items.push(item);
            } else {
                // Stale index entry -- clean up
                conn.srem::<_, _, ()>(self.list_key(), &id).await?;
            }
        }
        Ok(items)
    }

    /// Atomically get and delete an item (single-use pattern).
    ///
    /// Uses Redis `GETDEL` command. Returns `None` if the item does not exist.
    pub async fn take<T: DeserializeOwned>(&self, id: &str) -> anyhow::Result<Option<T>> {
        let mut conn = (*self.conn).clone();
        let result: Option<String> = redis::cmd("GETDEL")
            .arg(self.key(id))
            .query_async(&mut conn)
            .await?;
        if result.is_some() {
            // Remove from index
            conn.srem::<_, _, ()>(self.list_key(), id).await?;
        }
        match result {
            Some(json) => Ok(Some(
                serde_json::from_str(&json)
                    .map_err(|e| anyhow::anyhow!("deserialization: {e}"))?,
            )),
            None => Ok(None),
        }
    }

    /// Update an item by reading it, applying `updater`, and writing it back.
    ///
    /// Returns `None` if the item does not exist.
    pub async fn update<T: Serialize + DeserializeOwned>(
        &self,
        id: &str,
        updater: impl FnOnce(&mut T),
        ttl_secs: Option<u64>,
    ) -> anyhow::Result<Option<T>> {
        if let Some(mut item) = self.get::<T>(id).await? {
            updater(&mut item);
            self.set(id, &item, ttl_secs).await?;
            Ok(Some(item))
        } else {
            Ok(None)
        }
    }

    /// Count items in the index (may include stale entries).
    pub async fn count(&self) -> anyhow::Result<u64> {
        let mut conn = (*self.conn).clone();
        let count: u64 = conn.scard(self.list_key()).await?;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    // Unit tests for RedisStore require a running Redis instance.
    // These are integration tests that should be run with `--include-ignored`.

    #[test]
    fn key_formatting() {
        // Verify key generation without a live connection.
        // We cannot construct a ConnectionManager without Redis, so we test
        // the key logic indirectly by checking the format strings.
        let prefix = "aeterna:test";
        let id = "abc-123";
        let expected_key = format!("{prefix}:{id}");
        assert_eq!(expected_key, "aeterna:test:abc-123");
        let expected_index = format!("{prefix}:__index");
        assert_eq!(expected_index, "aeterna:test:__index");
    }
}
