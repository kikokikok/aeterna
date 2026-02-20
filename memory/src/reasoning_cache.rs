use async_trait::async_trait;
use mk_core::types::{ReasoningTrace, TenantContext};
use sha2::{Digest, Sha256};
use std::sync::Arc;

#[async_trait]
pub trait ReasoningCacheBackend: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<CachedReasoning>, CacheError>;
    async fn set(
        &self,
        key: &str,
        value: &CachedReasoning,
        ttl_seconds: u64,
    ) -> Result<(), CacheError>;
    async fn delete(&self, key: &str) -> Result<(), CacheError>;
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CachedReasoning {
    pub trace: ReasoningTrace,
    pub cached_at: i64,
    pub access_count: u64,
    pub last_accessed_at: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("Cache connection error: {0}")]
    ConnectionError(String),
    #[error("Cache serialization error: {0}")]
    SerializationError(String),
    #[error("Cache operation error: {0}")]
    OperationError(String),
}

#[derive(Debug, Clone)]
pub struct ReasoningDecayConfig {
    pub recency_weight: f64,
    pub frequency_weight: f64,
    pub age_weight: f64,
    pub eviction_threshold: f64,
}

impl Default for ReasoningDecayConfig {
    fn default() -> Self {
        Self {
            recency_weight: 0.4,
            frequency_weight: 0.4,
            age_weight: 0.2,
            eviction_threshold: 0.1,
        }
    }
}

pub fn compute_reasoning_decay_score(
    now: i64,
    cached: &CachedReasoning,
    ttl_seconds: u64,
    config: &ReasoningDecayConfig,
) -> f64 {
    let seconds_since_access = (now - cached.last_accessed_at).max(0) as f64;
    let recency_norm = 1.0 / (1.0 + seconds_since_access / 3600.0);

    let freq_norm = (cached.access_count as f64 / 10.0).min(1.0);

    let age_seconds = (now - cached.cached_at).max(0) as f64;
    let ttl = ttl_seconds.max(1) as f64;
    let age_norm = 1.0 / (1.0 + age_seconds / ttl);

    config.recency_weight * recency_norm
        + config.frequency_weight * freq_norm
        + config.age_weight * age_norm
}

pub struct ReasoningCache<B: ReasoningCacheBackend> {
    backend: Arc<B>,
    ttl_seconds: u64,
    enabled: bool,
    telemetry: Arc<crate::telemetry::MemoryTelemetry>,
    decay_config: ReasoningDecayConfig,
}

impl<B: ReasoningCacheBackend> ReasoningCache<B> {
    pub fn new(
        backend: Arc<B>,
        ttl_seconds: u64,
        enabled: bool,
        telemetry: Arc<crate::telemetry::MemoryTelemetry>,
    ) -> Self {
        Self {
            backend,
            ttl_seconds,
            enabled,
            telemetry,
            decay_config: ReasoningDecayConfig::default(),
        }
    }

    pub fn with_decay_config(mut self, config: ReasoningDecayConfig) -> Self {
        self.decay_config = config;
        self
    }

    fn should_evict(&self, cached: &CachedReasoning) -> bool {
        let now = chrono::Utc::now().timestamp();
        let score =
            compute_reasoning_decay_score(now, cached, self.ttl_seconds, &self.decay_config);
        score < self.decay_config.eviction_threshold
    }

    pub fn generate_cache_key(ctx: &TenantContext, query: &str) -> String {
        let normalized_query = Self::normalize_query(query);
        let mut hasher = Sha256::new();
        hasher.update(ctx.tenant_id.as_str().as_bytes());
        hasher.update(b":");
        hasher.update(normalized_query.as_bytes());
        let hash = hex::encode(hasher.finalize());
        format!("reasoning:{}:{}", ctx.tenant_id.as_str(), &hash[..16])
    }

    fn normalize_query(query: &str) -> String {
        query
            .to_lowercase()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub async fn get(
        &self,
        ctx: &TenantContext,
        query: &str,
    ) -> Result<Option<ReasoningTrace>, CacheError> {
        if !self.enabled {
            return Ok(None);
        }

        let key = Self::generate_cache_key(ctx, query);
        match self.backend.get(&key).await {
            Ok(Some(cached)) => {
                if self.should_evict(&cached) {
                    // Entry has decayed below threshold — treat as miss and remove
                    let _ = self.backend.delete(&key).await;
                    self.telemetry.record_reasoning_cache_miss();
                    Ok(None)
                } else {
                    self.telemetry.record_reasoning_cache_hit();
                    Ok(Some(cached.trace))
                }
            }
            Ok(None) => {
                self.telemetry.record_reasoning_cache_miss();
                Ok(None)
            }
            Err(e) => {
                tracing::warn!("Reasoning cache get error: {}", e);
                self.telemetry.record_reasoning_cache_miss();
                Ok(None)
            }
        }
    }

    pub async fn set(
        &self,
        ctx: &TenantContext,
        query: &str,
        trace: &ReasoningTrace,
    ) -> Result<(), CacheError> {
        if !self.enabled {
            return Ok(());
        }

        let key = Self::generate_cache_key(ctx, query);
        let now = chrono::Utc::now().timestamp();
        let cached = CachedReasoning {
            trace: trace.clone(),
            cached_at: now,
            access_count: 1,
            last_accessed_at: now,
        };

        match self.backend.set(&key, &cached, self.ttl_seconds).await {
            Ok(()) => Ok(()),
            Err(e) => {
                tracing::warn!("Reasoning cache set error: {}", e);
                Ok(())
            }
        }
    }

    pub async fn invalidate(&self, ctx: &TenantContext, query: &str) -> Result<(), CacheError> {
        if !self.enabled {
            return Ok(());
        }

        let key = Self::generate_cache_key(ctx, query);
        self.backend.delete(&key).await
    }
}

pub struct RedisReasoningCacheBackend {
    connection_manager: redis::aio::ConnectionManager,
}

impl RedisReasoningCacheBackend {
    pub async fn new(connection_string: &str) -> Result<Self, CacheError> {
        let client = redis::Client::open(connection_string)
            .map_err(|e| CacheError::ConnectionError(e.to_string()))?;

        let connection_manager = client
            .get_connection_manager()
            .await
            .map_err(|e| CacheError::ConnectionError(e.to_string()))?;

        Ok(Self { connection_manager })
    }
}

#[async_trait]
impl ReasoningCacheBackend for RedisReasoningCacheBackend {
    async fn get(&self, key: &str) -> Result<Option<CachedReasoning>, CacheError> {
        use redis::AsyncCommands;
        let mut conn = self.connection_manager.clone();

        let value: Option<String> = conn
            .get(key)
            .await
            .map_err(|e| CacheError::OperationError(e.to_string()))?;

        match value {
            Some(json) => {
                let cached: CachedReasoning = serde_json::from_str(&json)
                    .map_err(|e| CacheError::SerializationError(e.to_string()))?;
                Ok(Some(cached))
            }
            None => Ok(None),
        }
    }

    async fn set(
        &self,
        key: &str,
        value: &CachedReasoning,
        ttl_seconds: u64,
    ) -> Result<(), CacheError> {
        use redis::AsyncCommands;
        let mut conn = self.connection_manager.clone();

        let json = serde_json::to_string(value)
            .map_err(|e| CacheError::SerializationError(e.to_string()))?;

        let _: () = conn
            .set_ex(key, json, ttl_seconds)
            .await
            .map_err(|e| CacheError::OperationError(e.to_string()))?;

        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<(), CacheError> {
        use redis::AsyncCommands;
        let mut conn = self.connection_manager.clone();

        let _: usize = conn
            .del(key)
            .await
            .map_err(|e| CacheError::OperationError(e.to_string()))?;

        Ok(())
    }
}

pub struct InMemoryReasoningCacheBackend {
    cache: tokio::sync::RwLock<std::collections::HashMap<String, (CachedReasoning, i64)>>,
    access_order: tokio::sync::RwLock<std::collections::VecDeque<String>>,
    max_entries: usize,
}

impl InMemoryReasoningCacheBackend {
    pub fn new() -> Self {
        Self::with_max_entries(10000)
    }

    pub fn with_max_entries(max_entries: usize) -> Self {
        Self {
            cache: tokio::sync::RwLock::new(std::collections::HashMap::new()),
            access_order: tokio::sync::RwLock::new(std::collections::VecDeque::new()),
            max_entries,
        }
    }

    async fn evict_lru_if_needed(&self) {
        let cache = self.cache.read().await;
        if cache.len() < self.max_entries {
            return;
        }
        drop(cache);

        let mut cache = self.cache.write().await;
        let mut access_order = self.access_order.write().await;

        let now = chrono::Utc::now().timestamp();
        let default_decay = ReasoningDecayConfig::default();

        while cache.len() >= self.max_entries {
            let mut worst_key: Option<String> = None;
            let mut worst_score: f64 = f64::MAX;

            // Iterate over access_order from oldest to newest to naturally break ties by LRU
            for key in access_order.iter() {
                if let Some((cached, _)) = cache.get(key) {
                    let score = compute_reasoning_decay_score(now, cached, 3600, &default_decay);
                    if score < worst_score {
                        worst_score = score;
                        worst_key = Some(key.clone());
                    }
                }
            }

            if let Some(key) = worst_key {
                cache.remove(&key);
                access_order.retain(|k| k != &key);
            } else {
                break;
            }
        }
    }

    async fn update_access_order(&self, key: &str) {
        let mut access_order = self.access_order.write().await;
        access_order.retain(|k| k != key);
        access_order.push_back(key.to_string());
    }
}

impl Default for InMemoryReasoningCacheBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ReasoningCacheBackend for InMemoryReasoningCacheBackend {
    async fn get(&self, key: &str) -> Result<Option<CachedReasoning>, CacheError> {
        let now = chrono::Utc::now().timestamp();
        let cache = self.cache.read().await;
        if let Some((_, expires_at)) = cache.get(key) {
            if now < *expires_at {
                drop(cache);
                self.update_access_order(key).await;
                let mut cache = self.cache.write().await;
                if let Some((cached, _)) = cache.get_mut(key) {
                    cached.access_count += 1;
                    cached.last_accessed_at = now;
                    return Ok(Some(cached.clone()));
                }
            }
        }
        Ok(None)
    }

    async fn set(
        &self,
        key: &str,
        value: &CachedReasoning,
        ttl_seconds: u64,
    ) -> Result<(), CacheError> {
        self.evict_lru_if_needed().await;

        let mut cache = self.cache.write().await;
        let expires_at = chrono::Utc::now().timestamp() + ttl_seconds as i64;
        cache.insert(key.to_string(), (value.clone(), expires_at));
        drop(cache);

        self.update_access_order(key).await;
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<(), CacheError> {
        let mut cache = self.cache.write().await;
        cache.remove(key);
        drop(cache);

        let mut access_order = self.access_order.write().await;
        access_order.retain(|k| k != key);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mk_core::types::{ReasoningStrategy, TenantId, UserId};

    fn test_ctx() -> TenantContext {
        TenantContext::new(
            TenantId::new("test-tenant".to_string()).unwrap(),
            UserId::new("test-user".to_string()).unwrap(),
        )
    }

    fn test_trace() -> ReasoningTrace {
        ReasoningTrace {
            strategy: ReasoningStrategy::Targeted,
            thought_process: "Test reasoning".to_string(),
            refined_query: Some("refined query".to_string()),
            start_time: chrono::Utc::now(),
            end_time: chrono::Utc::now(),
            timed_out: false,
            duration_ms: 100,
            metadata: std::collections::HashMap::new(),
        }
    }

    #[test]
    fn test_cache_key_generation() {
        let ctx = test_ctx();
        let key1 =
            ReasoningCache::<InMemoryReasoningCacheBackend>::generate_cache_key(&ctx, "test query");
        let key2 =
            ReasoningCache::<InMemoryReasoningCacheBackend>::generate_cache_key(&ctx, "TEST QUERY");
        let key3 = ReasoningCache::<InMemoryReasoningCacheBackend>::generate_cache_key(
            &ctx,
            "  test   query  ",
        );

        assert_eq!(key1, key2, "Keys should be case-insensitive");
        assert_eq!(key1, key3, "Keys should normalize whitespace");
        assert!(key1.starts_with("reasoning:test-tenant:"));
    }

    #[test]
    fn test_different_tenants_different_keys() {
        let ctx1 = TenantContext::new(
            TenantId::new("tenant-1".to_string()).unwrap(),
            UserId::new("user".to_string()).unwrap(),
        );
        let ctx2 = TenantContext::new(
            TenantId::new("tenant-2".to_string()).unwrap(),
            UserId::new("user".to_string()).unwrap(),
        );

        let key1 = ReasoningCache::<InMemoryReasoningCacheBackend>::generate_cache_key(
            &ctx1,
            "same query",
        );
        let key2 = ReasoningCache::<InMemoryReasoningCacheBackend>::generate_cache_key(
            &ctx2,
            "same query",
        );

        assert_ne!(key1, key2, "Different tenants should have different keys");
    }

    #[tokio::test]
    async fn test_in_memory_cache_roundtrip() {
        let backend = Arc::new(InMemoryReasoningCacheBackend::new());
        let telemetry = Arc::new(crate::telemetry::MemoryTelemetry::new());
        let cache = ReasoningCache::new(backend, 3600, true, telemetry);

        let ctx = test_ctx();
        let trace = test_trace();

        cache.set(&ctx, "test query", &trace).await.unwrap();

        let retrieved = cache.get(&ctx, "test query").await.unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.strategy, trace.strategy);
        assert_eq!(retrieved.refined_query, trace.refined_query);
    }

    #[tokio::test]
    async fn test_cache_disabled() {
        let backend = Arc::new(InMemoryReasoningCacheBackend::new());
        let telemetry = Arc::new(crate::telemetry::MemoryTelemetry::new());
        let cache = ReasoningCache::new(backend, 3600, false, telemetry);

        let ctx = test_ctx();
        let trace = test_trace();

        cache.set(&ctx, "test query", &trace).await.unwrap();
        let retrieved = cache.get(&ctx, "test query").await.unwrap();
        assert!(
            retrieved.is_none(),
            "Cache should return None when disabled"
        );
    }

    #[tokio::test]
    async fn test_cache_invalidation() {
        let backend = Arc::new(InMemoryReasoningCacheBackend::new());
        let telemetry = Arc::new(crate::telemetry::MemoryTelemetry::new());
        let cache = ReasoningCache::new(backend, 3600, true, telemetry);

        let ctx = test_ctx();
        let trace = test_trace();

        cache.set(&ctx, "test query", &trace).await.unwrap();
        assert!(cache.get(&ctx, "test query").await.unwrap().is_some());

        cache.invalidate(&ctx, "test query").await.unwrap();
        assert!(cache.get(&ctx, "test query").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_cache_expiration() {
        let backend = Arc::new(InMemoryReasoningCacheBackend::new());
        let telemetry = Arc::new(crate::telemetry::MemoryTelemetry::new());
        let cache = ReasoningCache::new(backend.clone(), 1, true, telemetry);

        let ctx = test_ctx();
        let trace = test_trace();

        cache.set(&ctx, "test query", &trace).await.unwrap();

        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        let retrieved = cache.get(&ctx, "test query").await.unwrap();
        assert!(retrieved.is_none(), "Cache entry should expire");
    }

    #[test]
    fn test_cache_error_display() {
        let conn_err = CacheError::ConnectionError("refused".to_string());
        let ser_err = CacheError::SerializationError("invalid json".to_string());
        let op_err = CacheError::OperationError("timeout".to_string());

        assert!(conn_err.to_string().contains("connection"));
        assert!(ser_err.to_string().contains("serialization"));
        assert!(op_err.to_string().contains("operation"));
    }

    #[test]
    fn test_cached_reasoning_serialization() {
        let cached = CachedReasoning {
            trace: test_trace(),
            cached_at: 1704067200,
            access_count: 1,
            last_accessed_at: 1704067200,
        };

        let json = serde_json::to_string(&cached).unwrap();
        let parsed: CachedReasoning = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.cached_at, cached.cached_at);
        assert_eq!(parsed.trace.strategy, cached.trace.strategy);
        assert_eq!(parsed.access_count, 1);
        assert_eq!(parsed.last_accessed_at, 1704067200);
    }

    #[tokio::test]
    async fn test_lru_eviction() {
        let backend = Arc::new(InMemoryReasoningCacheBackend::with_max_entries(3));
        let telemetry = Arc::new(crate::telemetry::MemoryTelemetry::new());
        let cache = ReasoningCache::new(backend.clone(), 3600, true, telemetry);

        let ctx = test_ctx();
        let trace = test_trace();

        cache.set(&ctx, "query1", &trace).await.unwrap();
        cache.set(&ctx, "query2", &trace).await.unwrap();
        cache.set(&ctx, "query3", &trace).await.unwrap();

        assert!(cache.get(&ctx, "query1").await.unwrap().is_some());
        assert!(cache.get(&ctx, "query2").await.unwrap().is_some());
        assert!(cache.get(&ctx, "query3").await.unwrap().is_some());

        cache.set(&ctx, "query4", &trace).await.unwrap();

        assert!(
            cache.get(&ctx, "query1").await.unwrap().is_none(),
            "Oldest entry (query1) should be evicted"
        );
        assert!(cache.get(&ctx, "query2").await.unwrap().is_some());
        assert!(cache.get(&ctx, "query3").await.unwrap().is_some());
        assert!(cache.get(&ctx, "query4").await.unwrap().is_some());
    }

    #[tokio::test]
    async fn test_lru_access_order_update() {
        let backend = Arc::new(InMemoryReasoningCacheBackend::with_max_entries(3));
        let telemetry = Arc::new(crate::telemetry::MemoryTelemetry::new());
        let cache = ReasoningCache::new(backend.clone(), 3600, true, telemetry);

        let ctx = test_ctx();
        let trace = test_trace();

        cache.set(&ctx, "query1", &trace).await.unwrap();
        cache.set(&ctx, "query2", &trace).await.unwrap();
        cache.set(&ctx, "query3", &trace).await.unwrap();

        cache.get(&ctx, "query1").await.unwrap();

        cache.set(&ctx, "query4", &trace).await.unwrap();

        assert!(
            cache.get(&ctx, "query1").await.unwrap().is_some(),
            "query1 should still exist (was accessed recently)"
        );
        assert!(
            cache.get(&ctx, "query2").await.unwrap().is_none(),
            "query2 should be evicted (oldest after query1 access)"
        );
        assert!(cache.get(&ctx, "query3").await.unwrap().is_some());
        assert!(cache.get(&ctx, "query4").await.unwrap().is_some());
    }

    #[test]
    fn test_with_max_entries_constructor() {
        let backend = InMemoryReasoningCacheBackend::with_max_entries(500);
        assert_eq!(backend.max_entries, 500);
    }
}
