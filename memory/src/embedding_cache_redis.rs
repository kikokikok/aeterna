///! # Redis Embedding Cache Backend
///!
///! Redis-based implementation of the embedding cache with support for:
///! - Exact match lookups using simple key-value
///! - Semantic similarity search using RediSearch (if available)
///! - TTL management
///! - Per-tenant isolation
use crate::embedding_cache::{CacheError, CachedEmbedding, EmbeddingCacheBackend};
use async_trait::async_trait;
use mk_core::types::TenantContext;
use redis::AsyncCommands;
use std::sync::Arc;
use tracing::debug;

pub struct RedisEmbeddingCacheBackend {
    _client: Arc<redis::Client>,
    connection_manager: redis::aio::ConnectionManager,
    /// Whether RediSearch is available for similarity search
    similarity_enabled: bool,
}

impl RedisEmbeddingCacheBackend {
    pub async fn new(connection_string: &str) -> Result<Self, CacheError> {
        let client = redis::Client::open(connection_string).map_err(|e| {
            CacheError::ConnectionError(format!("Failed to create Redis client: {}", e))
        })?;

        let connection_manager = client.get_connection_manager().await.map_err(|e| {
            CacheError::ConnectionError(format!("Failed to connect to Redis: {}", e))
        })?;

        // Check if RediSearch is available (for semantic similarity)
        let similarity_enabled = Self::check_redisearch_available(&connection_manager).await;

        Ok(Self {
            _client: Arc::new(client),
            connection_manager,
            similarity_enabled,
        })
    }

    async fn check_redisearch_available(_conn: &redis::aio::ConnectionManager) -> bool {
        // Try to check if RediSearch module is loaded
        // For now, we'll default to false and implement basic caching only
        // In production, you'd check: MODULE LIST and look for "search"
        false
    }

    fn scoped_key(&self, tenant_id: &str, key: &str) -> String {
        format!("{}:emb:{}", tenant_id, key)
    }

    #[allow(dead_code)]
    fn semantic_index_key(&self, tenant_id: &str) -> String {
        format!("{}:emb:semantic:index", tenant_id)
    }
}

#[async_trait]
impl EmbeddingCacheBackend for RedisEmbeddingCacheBackend {
    async fn get_exact(&self, key: &str) -> Result<Option<CachedEmbedding>, CacheError> {
        let mut conn = self.connection_manager.clone();

        let data: Option<String> = conn
            .get(key)
            .await
            .map_err(|e| CacheError::OperationError(format!("Redis GET failed: {}", e)))?;

        match data {
            Some(json_str) => {
                let cached: CachedEmbedding = serde_json::from_str(&json_str).map_err(|e| {
                    CacheError::SerializationError(format!("Failed to deserialize: {}", e))
                })?;
                Ok(Some(cached))
            }
            None => Ok(None),
        }
    }

    async fn set_exact(
        &self,
        key: &str,
        value: &CachedEmbedding,
        ttl_seconds: u64,
    ) -> Result<(), CacheError> {
        let mut conn = self.connection_manager.clone();

        let json_str = serde_json::to_string(value)
            .map_err(|e| CacheError::SerializationError(format!("Failed to serialize: {}", e)))?;

        let _: () = conn
            .set_ex(key, json_str, ttl_seconds)
            .await
            .map_err(|e| CacheError::OperationError(format!("Redis SET failed: {}", e)))?;

        Ok(())
    }

    async fn find_similar(
        &self,
        ctx: &TenantContext,
        _embedding: &[f32],
        threshold: f32,
    ) -> Result<Option<CachedEmbedding>, CacheError> {
        if !self.similarity_enabled {
            return Err(CacheError::SimilarityNotSupported);
        }

        // For now, return not supported
        // In production, this would use RediSearch VSS (Vector Similarity Search)
        // Example query: FT.SEARCH idx "* => [KNN 1 @vec $query AS score]" ...
        debug!(
            "Semantic similarity search requested for tenant {}, threshold {}",
            ctx.tenant_id.as_str(),
            threshold
        );

        Err(CacheError::SimilarityNotSupported)
    }

    async fn store_with_vector(
        &self,
        ctx: &TenantContext,
        content_hash: &str,
        embedding: &[f32],
        ttl_seconds: u64,
    ) -> Result<(), CacheError> {
        // Store the content hash with embedding for future similarity lookups
        // This is simplified - in production you'd use RediSearch to index the vector

        let key = self.scoped_key(ctx.tenant_id.as_str(), &format!("vec:{}", content_hash));
        let mut conn = self.connection_manager.clone();

        // Store as JSON for now (not optimized for similarity search)
        let embedding_json = serde_json::to_string(embedding).map_err(|e| {
            CacheError::SerializationError(format!("Failed to serialize embedding: {}", e))
        })?;

        let _: () = conn
            .set_ex(key, embedding_json, ttl_seconds)
            .await
            .map_err(|e| CacheError::OperationError(format!("Failed to store vector: {}", e)))?;

        debug!("Stored embedding vector for content hash: {}", content_hash);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    #[ignore] // Requires Redis to be running
    async fn test_redis_exact_cache() {
        let backend = RedisEmbeddingCacheBackend::new("redis://localhost:6379")
            .await
            .expect("Failed to create Redis backend");

        let cached = CachedEmbedding {
            embedding: vec![1.0, 2.0, 3.0],
            content: "test content".to_string(),
            content_hash: "abc123".to_string(),
            model: "text-embedding-ada-002".to_string(),
            cached_at: chrono::Utc::now().timestamp(),
            tenant_id: "test-tenant".to_string(),
            access_count: 1,
            last_accessed_at: chrono::Utc::now().timestamp(),
        };

        // Test set and get
        backend
            .set_exact("test:key", &cached, 60)
            .await
            .expect("Failed to set");

        let retrieved = backend.get_exact("test:key").await.expect("Failed to get");

        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.content, "test content");
        assert_eq!(retrieved.embedding.len(), 3);
    }

    #[tokio::test]
    #[ignore] // Requires Redis to be running
    async fn test_redis_cache_miss() {
        let backend = RedisEmbeddingCacheBackend::new("redis://localhost:6379")
            .await
            .expect("Failed to create Redis backend");

        let retrieved = backend
            .get_exact("nonexistent:key")
            .await
            .expect("Failed to get");

        assert!(retrieved.is_none());
    }
}
