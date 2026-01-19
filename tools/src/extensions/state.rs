use redis::AsyncCommands;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::Arc;

use super::{ExtensionContext, ExtensionContextState};
use async_trait::async_trait;
use knowledge::context_architect::LlmClient;
use serde_json::Value;

#[derive(Debug, thiserror::Error)]
pub enum ExtensionStateError {
    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),

    #[error("Serialization error: {0}")]
    Serialization(String),
}

pub type StateMigrator = Arc<
    dyn Fn(ExtensionContextState) -> Result<ExtensionContextState, ExtensionStateError>
        + Send
        + Sync,
>;

pub type StateCompactor = Arc<
    dyn Fn(ExtensionContextState) -> Result<ExtensionContextState, ExtensionStateError>
        + Send
        + Sync,
>;

#[async_trait]
pub trait AsyncStateCompactor: Send + Sync {
    async fn compact(
        &self,
        payload: ExtensionContextState,
    ) -> Result<ExtensionContextState, ExtensionStateError>;
}

pub type AsyncStateCompactorHandle = Arc<dyn AsyncStateCompactor>;

pub struct ExtensionStateStore {
    redis_url: String,
    ttl_secs: u64,
    version: u32,
    migrator: Option<StateMigrator>,
    compactor: Option<StateCompactor>,
    async_compactor: Option<AsyncStateCompactorHandle>,
}

pub struct LlmStateCompactor<C: LlmClient> {
    client: Arc<C>,
    min_state_bytes: usize,
    max_tokens: u32,
}

impl<C: LlmClient> LlmStateCompactor<C> {
    pub fn new(client: Arc<C>, min_state_bytes: usize, max_tokens: u32) -> Self {
        Self {
            client,
            min_state_bytes,
            max_tokens,
        }
    }

    fn should_compact(&self, payload: &ExtensionContextState) -> bool {
        serde_json::to_vec(&payload.state)
            .map(|bytes| bytes.len() >= self.min_state_bytes)
            .unwrap_or(false)
    }

    fn build_prompt(&self, payload: &ExtensionContextState) -> Result<String, ExtensionStateError> {
        let serialized = serde_json::to_string(&payload.state)
            .map_err(|err| ExtensionStateError::Serialization(err.to_string()))?;
        Ok(format!(
            "Compress the following JSON state for an extension. Preserve key facts, recent \
             \ndecisions, and references needed for future tool calls. Keep JSON structure. \
             \nReturn ONLY valid JSON without markdown. Limit to {max_tokens} \
             tokens.\n\n{serialized}",
            max_tokens = self.max_tokens
        ))
    }

    async fn compress_payload(
        &self,
        payload: ExtensionContextState,
    ) -> Result<ExtensionContextState, ExtensionStateError> {
        if !self.should_compact(&payload) {
            return Ok(payload);
        }

        let prompt = self.build_prompt(&payload)?;
        let response = self
            .client
            .complete(&prompt)
            .await
            .map_err(|err| ExtensionStateError::Serialization(err.to_string()))?;
        let trimmed = response.trim();
        if trimmed.is_empty() {
            return Ok(payload);
        }

        let parsed: HashMap<String, Value> = serde_json::from_str(trimmed)
            .map_err(|err| ExtensionStateError::Serialization(err.to_string()))?;
        Ok(ExtensionContextState {
            state: parsed,
            version: payload.version,
        })
    }
}

#[async_trait]
impl<C: LlmClient> AsyncStateCompactor for LlmStateCompactor<C> {
    async fn compact(
        &self,
        payload: ExtensionContextState,
    ) -> Result<ExtensionContextState, ExtensionStateError> {
        self.compress_payload(payload).await
    }
}

impl ExtensionStateStore {
    pub fn new(redis_url: String, ttl_secs: u64) -> Self {
        Self {
            redis_url,
            ttl_secs,
            version: 1,
            migrator: None,
            compactor: None,
            async_compactor: None,
        }
    }

    pub fn with_version(mut self, version: u32) -> Self {
        self.version = version;
        self
    }

    pub fn migrate_with(mut self, migrator: StateMigrator) -> Self {
        self.migrator = Some(migrator);
        self
    }

    pub fn compact_with(mut self, compactor: StateCompactor) -> Self {
        self.compactor = Some(compactor);
        self
    }

    pub fn compact_with_async(mut self, compactor: AsyncStateCompactorHandle) -> Self {
        self.async_compactor = Some(compactor);
        self
    }

    pub async fn save(&self, ctx: &ExtensionContext) -> Result<(), ExtensionStateError> {
        let mut con = self.connection().await?;
        let mut payload = ctx
            .to_state_payload()
            .map_err(|err| ExtensionStateError::Serialization(err.to_string()))?;
        if let Some(compactor) = &self.compactor {
            payload = compactor(payload)?;
        }
        if let Some(compactor) = &self.async_compactor {
            payload = compactor.compact(payload).await?;
        }
        payload.version = self.version;
        let data = serde_json::to_vec(&payload)
            .map_err(|err| ExtensionStateError::Serialization(err.to_string()))?;
        let data = compress_payload(&data)?;
        let key = self.key(ctx);
        let _: () = con.set_ex(key, data, self.ttl_secs).await?;
        Ok(())
    }

    pub async fn load(&self, ctx: &mut ExtensionContext) -> Result<(), ExtensionStateError> {
        let mut con = self.connection().await?;
        let key = self.key(ctx);
        let data: Option<Vec<u8>> = con.get(key).await?;
        if let Some(bytes) = data {
            let bytes = decompress_payload(&bytes)?;
            let payload: ExtensionContextState = serde_json::from_slice(&bytes)
                .map_err(|err| ExtensionStateError::Serialization(err.to_string()))?;
            let payload = self.migrate(payload)?;
            ctx.replace_state(payload.state);
        }
        Ok(())
    }

    pub async fn delete(&self, ctx: &ExtensionContext) -> Result<(), ExtensionStateError> {
        let mut con = self.connection().await?;
        let key = self.key(ctx);
        let _: () = con.del(key).await?;
        Ok(())
    }

    async fn connection(&self) -> Result<redis::aio::ConnectionManager, ExtensionStateError> {
        let client = redis::Client::open(self.redis_url.clone())?;
        let con = redis::aio::ConnectionManager::new(client).await?;
        Ok(con)
    }

    fn key(&self, ctx: &ExtensionContext) -> String {
        format!(
            "extension:{}:{}:{}",
            ctx.tenant_ctx.tenant_id, ctx.session_id, ctx.extension_id
        )
    }

    fn migrate(
        &self,
        mut payload: ExtensionContextState,
    ) -> Result<ExtensionContextState, ExtensionStateError> {
        if payload.version > self.version {
            return Err(ExtensionStateError::Serialization(
                "Unsupported state version".to_string(),
            ));
        }
        if let Some(migrator) = &self.migrator {
            payload = migrator(payload)?;
        }
        payload.version = self.version;
        Ok(payload)
    }
}

fn compress_payload(data: &[u8]) -> Result<Vec<u8>, ExtensionStateError> {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder
        .write_all(data)
        .map_err(|err| ExtensionStateError::Serialization(err.to_string()))?;
    encoder
        .finish()
        .map_err(|err| ExtensionStateError::Serialization(err.to_string()))
}

fn decompress_payload(data: &[u8]) -> Result<Vec<u8>, ExtensionStateError> {
    let mut decoder = flate2::read::GzDecoder::new(data);
    let mut out = Vec::new();
    decoder
        .read_to_end(&mut out)
        .map_err(|err| ExtensionStateError::Serialization(err.to_string()))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mk_core::types::TenantContext;
    use std::sync::Arc;
    use testcontainers::runners::AsyncRunner;
    use testcontainers_modules::redis::Redis;

    async fn create_store() -> Option<ExtensionStateStore> {
        let container = Redis::default().start().await.ok()?;
        let host = container.get_host().await.ok()?;
        let port = container.get_host_port_ipv4(6379).await.ok()?;
        let url = format!("redis://{}:{}/", host, port);
        Some(ExtensionStateStore::new(url, 60))
    }

    #[tokio::test]
    async fn test_state_store_roundtrip() {
        let Some(store) = create_store().await else {
            return;
        };
        let registry = Arc::new(crate::tools::ToolRegistry::new());
        let mut ctx = ExtensionContext::new(
            TenantContext::default(),
            "session".to_string(),
            "ext".to_string(),
            registry,
            1024,
        );
        ctx.set_state("key", "value").unwrap();
        store.save(&ctx).await.unwrap();

        ctx.clear_state();
        store.load(&mut ctx).await.unwrap();
        let value: Option<String> = ctx.get_state("key").unwrap();
        assert_eq!(value, Some("value".to_string()));
    }

    #[test]
    fn test_compression_roundtrip() {
        let payload = serde_json::to_vec(&ExtensionContextState {
            state: std::collections::HashMap::from([(
                "key".to_string(),
                serde_json::json!("value"),
            )]),
            version: 1,
        })
        .unwrap();
        let compressed = compress_payload(&payload).unwrap();
        let decompressed = decompress_payload(&compressed).unwrap();
        assert_eq!(payload, decompressed);
    }

    #[test]
    fn test_compactor() {
        let compactor: StateCompactor = Arc::new(|mut payload| {
            payload.state.clear();
            Ok(payload)
        });
        let store =
            ExtensionStateStore::new("redis://localhost".to_string(), 10).compact_with(compactor);
        let payload = ExtensionContextState {
            state: std::collections::HashMap::from([(
                "key".to_string(),
                serde_json::json!("value"),
            )]),
            version: 1,
        };
        let compacted = store.compactor.unwrap()(payload).unwrap();
        assert!(compacted.state.is_empty());
    }

    #[test]
    fn test_migrator() {
        let migrator: StateMigrator = Arc::new(|mut payload| {
            payload.state.clear();
            Ok(payload)
        });
        let store =
            ExtensionStateStore::new("redis://localhost".to_string(), 10).migrate_with(migrator);
        let payload = ExtensionContextState {
            state: std::collections::HashMap::from([(
                "key".to_string(),
                serde_json::json!("value"),
            )]),
            version: 1,
        };
        let migrated = store.migrate(payload).unwrap();
        assert!(migrated.state.is_empty());
    }

    #[tokio::test]
    async fn test_llm_state_compactor_respects_threshold() {
        struct MockClient;

        #[async_trait]
        impl LlmClient for MockClient {
            async fn complete(
                &self,
                _prompt: &str,
            ) -> Result<String, knowledge::context_architect::LlmError> {
                Ok("{}".to_string())
            }

            async fn complete_with_system(
                &self,
                _system: &str,
                _user: &str,
            ) -> Result<String, knowledge::context_architect::LlmError> {
                Ok("{}".to_string())
            }
        }

        let compactor = LlmStateCompactor::new(Arc::new(MockClient), 1024, 100);
        let payload = ExtensionContextState {
            state: std::collections::HashMap::from([(
                "key".to_string(),
                serde_json::json!("value"),
            )]),
            version: 1,
        };
        let compacted = compactor.compact(payload.clone()).await.unwrap();
        assert_eq!(compacted.state, payload.state);
    }

    #[tokio::test]
    async fn test_llm_state_compactor_applies_json() {
        struct MockClient;

        #[async_trait]
        impl LlmClient for MockClient {
            async fn complete(
                &self,
                _prompt: &str,
            ) -> Result<String, knowledge::context_architect::LlmError> {
                Ok("{\"summary\":\"ok\"}".to_string())
            }

            async fn complete_with_system(
                &self,
                _system: &str,
                _user: &str,
            ) -> Result<String, knowledge::context_architect::LlmError> {
                Ok("{\"summary\":\"ok\"}".to_string())
            }
        }

        let compactor = LlmStateCompactor::new(Arc::new(MockClient), 1, 100);
        let payload = ExtensionContextState {
            state: std::collections::HashMap::from([(
                "key".to_string(),
                serde_json::json!("value"),
            )]),
            version: 1,
        };
        let compacted = compactor.compact(payload).await.unwrap();
        assert_eq!(compacted.state.len(), 1);
        assert_eq!(
            compacted.state.get("summary").unwrap(),
            &serde_json::json!("ok")
        );
    }
}
