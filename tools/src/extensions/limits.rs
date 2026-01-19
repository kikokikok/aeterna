use std::collections::HashMap;
use std::sync::Arc;

use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{info, warn};

use super::ExtensionStateError;

const ONE_MB: usize = 1024 * 1024;
const ONE_HOUR_SECS: u64 = 3600;
const FIFTY_MB: usize = 50 * 1024 * 1024;

const DEFAULT_MAX_STATE_SIZE_BYTES: usize = ONE_MB;
const DEFAULT_STATE_TTL_SECS: u64 = ONE_HOUR_SECS;
const DEFAULT_TENANT_TOTAL_LIMIT_BYTES: usize = FIFTY_MB;
const ALERT_THRESHOLD_PERCENT: f32 = 0.8;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionStateConfig {
    pub max_state_size_bytes: usize,
    pub state_ttl_secs: u64,
}

impl Default for ExtensionStateConfig {
    fn default() -> Self {
        Self {
            max_state_size_bytes: DEFAULT_MAX_STATE_SIZE_BYTES,
            state_ttl_secs: DEFAULT_STATE_TTL_SECS,
        }
    }
}

impl ExtensionStateConfig {
    pub fn new(max_state_size_bytes: usize, state_ttl_secs: u64) -> Self {
        Self {
            max_state_size_bytes,
            state_ttl_secs,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtensionStateMetrics {
    pub size_bytes: u64,
    pub keys_count: u64,
    pub evictions: u64,
    pub alerts_triggered: u64,
}

#[derive(Debug, Clone)]
pub struct LruEntry {
    pub key: String,
    pub size_bytes: usize,
    pub last_access_at: i64,
}

pub struct ExtensionStateLimiter {
    redis_url: String,
    tenant_total_limit_bytes: usize,
    extension_configs: Arc<RwLock<HashMap<String, ExtensionStateConfig>>>,
    metrics: Arc<RwLock<HashMap<String, ExtensionStateMetrics>>>,
}

impl ExtensionStateLimiter {
    pub fn new(redis_url: String) -> Self {
        Self {
            redis_url,
            tenant_total_limit_bytes: DEFAULT_TENANT_TOTAL_LIMIT_BYTES,
            extension_configs: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn with_tenant_limit(mut self, limit_bytes: usize) -> Self {
        self.tenant_total_limit_bytes = limit_bytes;
        self
    }

    pub async fn register_extension(&self, extension_id: &str, config: ExtensionStateConfig) {
        let mut configs = self.extension_configs.write().await;
        configs.insert(extension_id.to_string(), config);
    }

    pub async fn get_config(&self, extension_id: &str) -> ExtensionStateConfig {
        let configs = self.extension_configs.read().await;
        configs.get(extension_id).cloned().unwrap_or_default()
    }

    pub async fn check_size_limit(
        &self,
        extension_id: &str,
        state_bytes: usize,
    ) -> Result<(), ExtensionStateError> {
        let config = self.get_config(extension_id).await;

        if state_bytes > config.max_state_size_bytes {
            return Err(ExtensionStateError::Serialization(format!(
                "State size {} exceeds limit {} for extension {}",
                state_bytes, config.max_state_size_bytes, extension_id
            )));
        }

        let threshold_bytes =
            (config.max_state_size_bytes as f32 * ALERT_THRESHOLD_PERCENT) as usize;
        if state_bytes >= threshold_bytes {
            warn!(
                extension_id = %extension_id,
                size_bytes = state_bytes,
                limit_bytes = config.max_state_size_bytes,
                "Extension state approaching size limit ({}%)",
                (state_bytes as f32 / config.max_state_size_bytes as f32 * 100.0) as u32
            );
            self.increment_alerts(extension_id).await;
        }

        Ok(())
    }

    pub async fn enforce_tenant_limit(
        &self,
        tenant_id: &str,
        incoming_bytes: usize,
    ) -> Result<Vec<String>, ExtensionStateError> {
        let mut con = self.connection().await?;
        let pattern = format!("extension:{}:*", tenant_id);

        let keys: Vec<String> = redis::cmd("KEYS")
            .arg(&pattern)
            .query_async(&mut con)
            .await
            .map_err(ExtensionStateError::Redis)?;

        if keys.is_empty() {
            return Ok(vec![]);
        }

        let mut entries: Vec<LruEntry> = Vec::new();
        let mut total_size: usize = 0;

        for key in &keys {
            let size: usize = con.strlen(key).await.map_err(ExtensionStateError::Redis)?;
            let idle: Option<i64> = redis::cmd("OBJECT")
                .arg("IDLETIME")
                .arg(key)
                .query_async(&mut con)
                .await
                .ok();

            total_size += size;

            entries.push(LruEntry {
                key: key.clone(),
                size_bytes: size,
                last_access_at: idle.unwrap_or(0),
            });
        }

        let mut evicted: Vec<String> = Vec::new();

        if total_size + incoming_bytes > self.tenant_total_limit_bytes {
            entries.sort_by(|a, b| b.last_access_at.cmp(&a.last_access_at));

            let mut current_total = total_size;
            for entry in entries {
                if current_total + incoming_bytes <= self.tenant_total_limit_bytes {
                    break;
                }
                let _: () = con
                    .del(&entry.key)
                    .await
                    .map_err(ExtensionStateError::Redis)?;
                current_total -= entry.size_bytes;
                evicted.push(entry.key.clone());

                info!(
                    tenant_id = %tenant_id,
                    evicted_key = %entry.key,
                    freed_bytes = entry.size_bytes,
                    "Evicted LRU extension state"
                );
            }

            self.increment_evictions(tenant_id, evicted.len() as u64)
                .await;
        }

        Ok(evicted)
    }

    pub async fn save_with_ttl(
        &self,
        key: &str,
        data: &[u8],
        extension_id: &str,
    ) -> Result<(), ExtensionStateError> {
        let config = self.get_config(extension_id).await;
        let mut con = self.connection().await?;
        let _: () = con
            .set_ex(key, data, config.state_ttl_secs)
            .await
            .map_err(ExtensionStateError::Redis)?;

        self.update_metrics(extension_id, data.len() as u64).await;
        Ok(())
    }

    pub async fn get_metrics(&self, extension_id: &str) -> ExtensionStateMetrics {
        let metrics = self.metrics.read().await;
        metrics.get(extension_id).cloned().unwrap_or_default()
    }

    pub async fn get_all_metrics(&self) -> HashMap<String, ExtensionStateMetrics> {
        self.metrics.read().await.clone()
    }

    async fn update_metrics(&self, extension_id: &str, size_bytes: u64) {
        let mut metrics = self.metrics.write().await;
        let entry = metrics
            .entry(extension_id.to_string())
            .or_insert_with(ExtensionStateMetrics::default);
        entry.size_bytes = size_bytes;
        entry.keys_count += 1;
    }

    async fn increment_evictions(&self, tenant_id: &str, count: u64) {
        let mut metrics = self.metrics.write().await;
        let entry = metrics
            .entry(tenant_id.to_string())
            .or_insert_with(ExtensionStateMetrics::default);
        entry.evictions += count;
    }

    async fn increment_alerts(&self, extension_id: &str) {
        let mut metrics = self.metrics.write().await;
        let entry = metrics
            .entry(extension_id.to_string())
            .or_insert_with(ExtensionStateMetrics::default);
        entry.alerts_triggered += 1;
    }

    async fn connection(&self) -> Result<redis::aio::ConnectionManager, ExtensionStateError> {
        let client = redis::Client::open(self.redis_url.clone())?;
        let con = redis::aio::ConnectionManager::new(client).await?;
        Ok(con)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extension_state_config_default() {
        let config = ExtensionStateConfig::default();
        assert_eq!(config.max_state_size_bytes, 1024 * 1024);
        assert_eq!(config.state_ttl_secs, 3600);
    }

    #[test]
    fn test_extension_state_config_custom() {
        let config = ExtensionStateConfig::new(512 * 1024, 7200);
        assert_eq!(config.max_state_size_bytes, 512 * 1024);
        assert_eq!(config.state_ttl_secs, 7200);
    }

    #[tokio::test]
    async fn test_limiter_register_and_get_config() {
        let limiter = ExtensionStateLimiter::new("redis://localhost".to_string());
        let config = ExtensionStateConfig::new(2048, 1800);
        limiter.register_extension("test-ext", config.clone()).await;

        let retrieved = limiter.get_config("test-ext").await;
        assert_eq!(retrieved.max_state_size_bytes, 2048);
        assert_eq!(retrieved.state_ttl_secs, 1800);
    }

    #[tokio::test]
    async fn test_check_size_limit_within_bounds() {
        let limiter = ExtensionStateLimiter::new("redis://localhost".to_string());
        let config = ExtensionStateConfig::new(1024, 3600);
        limiter.register_extension("test-ext", config).await;

        let result = limiter.check_size_limit("test-ext", 512).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_check_size_limit_exceeds() {
        let limiter = ExtensionStateLimiter::new("redis://localhost".to_string());
        let config = ExtensionStateConfig::new(1024, 3600);
        limiter.register_extension("test-ext", config).await;

        let result = limiter.check_size_limit("test-ext", 2048).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_metrics_tracking() {
        let limiter = ExtensionStateLimiter::new("redis://localhost".to_string());
        limiter.update_metrics("test-ext", 500).await;

        let metrics = limiter.get_metrics("test-ext").await;
        assert_eq!(metrics.size_bytes, 500);
        assert_eq!(metrics.keys_count, 1);
    }

    #[tokio::test]
    async fn test_alert_threshold_tracking() {
        let limiter = ExtensionStateLimiter::new("redis://localhost".to_string());
        let config = ExtensionStateConfig::new(1000, 3600);
        limiter.register_extension("test-ext", config).await;

        let eighty_five_percent_of_limit = 850;
        let _ = limiter
            .check_size_limit("test-ext", eighty_five_percent_of_limit)
            .await;

        let metrics = limiter.get_metrics("test-ext").await;
        assert_eq!(metrics.alerts_triggered, 1);
    }

    #[tokio::test]
    async fn test_get_all_metrics() {
        let limiter = ExtensionStateLimiter::new("redis://localhost".to_string());
        limiter.update_metrics("ext1", 100).await;
        limiter.update_metrics("ext2", 200).await;

        let all_metrics = limiter.get_all_metrics().await;
        assert_eq!(all_metrics.len(), 2);
        assert!(all_metrics.contains_key("ext1"));
        assert!(all_metrics.contains_key("ext2"));
    }
}
