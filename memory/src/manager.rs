use crate::governance::GovernanceService;
use crate::telemetry::MemoryTelemetry;
use mk_core::traits::MemoryProviderAdapter;
use mk_core::traits::{AuthorizationService, EmbeddingService};
use mk_core::types::{MemoryEntry, MemoryLayer};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub type ProviderMap = HashMap<
    MemoryLayer,
    Box<dyn MemoryProviderAdapter<Error = Box<dyn std::error::Error + Send + Sync>> + Send + Sync>
>;

pub struct MemoryManager {
    providers: Arc<RwLock<ProviderMap>>,
    embedding_service: Option<
        Arc<dyn EmbeddingService<Error = Box<dyn std::error::Error + Send + Sync>> + Send + Sync>
    >,
    governance_service: Arc<GovernanceService>,
    auth_service: Option<
        Arc<
            dyn AuthorizationService<Error = Box<dyn std::error::Error + Send + Sync>>
                + Send
                + Sync
        >
    >,
    telemetry: Arc<MemoryTelemetry>,
    config: config::MemoryConfig
}

impl MemoryManager {
    pub fn new() -> Self {
        Self {
            providers: Arc::new(RwLock::new(HashMap::new())),
            embedding_service: None,
            governance_service: Arc::new(GovernanceService::new()),
            auth_service: None,
            telemetry: Arc::new(MemoryTelemetry::new()),
            config: config::MemoryConfig::default()
        }
    }

    pub fn with_config(mut self, config: config::MemoryConfig) -> Self {
        self.config = config;
        self
    }

    pub fn with_embedding_service(
        mut self,
        embedding_service: Arc<
            dyn EmbeddingService<Error = Box<dyn std::error::Error + Send + Sync>> + Send + Sync
        >
    ) -> Self {
        self.embedding_service = Some(embedding_service);
        self
    }

    pub fn with_auth_service(
        mut self,
        auth_service: Arc<
            dyn AuthorizationService<Error = Box<dyn std::error::Error + Send + Sync>>
                + Send
                + Sync
        >
    ) -> Self {
        self.auth_service = Some(auth_service);
        self
    }

    pub fn with_telemetry(mut self, telemetry: Arc<MemoryTelemetry>) -> Self {
        self.telemetry = telemetry;
        self
    }
}

impl Default for MemoryManager {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryManager {
    pub async fn register_provider(
        &self,
        layer: MemoryLayer,
        provider: Box<
            dyn MemoryProviderAdapter<Error = Box<dyn std::error::Error + Send + Sync>>
                + Send
                + Sync
        >
    ) {
        let mut providers = self.providers.write().await;
        providers.insert(layer, provider);
    }

    pub async fn search_hierarchical(
        &self,
        ctx: mk_core::types::TenantContext,
        query_vector: Vec<f32>,
        limit: usize,
        filters: HashMap<String, serde_json::Value>
    ) -> Result<Vec<MemoryEntry>, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(auth) = &self.auth_service {
            if !auth
                .check_permission(&ctx, "memory:read", "hierarchical")
                .await?
            {
                return Err("Unauthorized to search hierarchical memory".into());
            }
        }

        let start = std::time::Instant::now();
        let providers = self.providers.read().await;
        let mut all_results = Vec::new();

        for (layer, provider) in providers.iter() {
            let layer_str = format!("{:?}", layer);
            let _span = self.telemetry.record_operation_start("search", &layer_str);
            match provider
                .search(ctx.clone(), query_vector.clone(), limit, filters.clone())
                .await
            {
                Ok(results) => {
                    self.telemetry.record_operation_success(
                        "search",
                        &layer_str,
                        start.elapsed().as_millis() as f64
                    );
                    for mut entry in results {
                        entry.layer = *layer;
                        all_results.push(entry);
                    }
                }
                Err(e) => {
                    self.telemetry
                        .record_operation_failure("search", &layer_str, &e.to_string());
                    tracing::error!("Error searching layer {:?}: {}", layer, e)
                }
            }
        }

        all_results.sort_by(|a, b| a.layer.precedence().cmp(&b.layer.precedence()));

        let final_results: Vec<MemoryEntry> = all_results.into_iter().take(limit).collect();
        self.telemetry
            .record_search_operation(final_results.len(), query_vector.len());
        Ok(final_results)
    }

    pub async fn search_with_threshold(
        &self,
        ctx: mk_core::types::TenantContext,
        query_vector: Vec<f32>,
        limit: usize,
        threshold: f32,
        filters: HashMap<String, serde_json::Value>
    ) -> Result<Vec<MemoryEntry>, Box<dyn std::error::Error + Send + Sync>> {
        let providers = self.providers.read().await;
        let mut all_results = Vec::new();

        for (layer, provider) in providers.iter() {
            match provider
                .search(ctx.clone(), query_vector.clone(), limit, filters.clone())
                .await
            {
                Ok(results) => {
                    for mut entry in results {
                        let score = entry
                            .metadata
                            .get("score")
                            .and_then(|s| s.as_f64())
                            .map(|s| s as f32)
                            .unwrap_or(1.0);

                        if score >= threshold {
                            entry.layer = *layer;
                            all_results.push(entry);
                        }
                    }
                }
                Err(e) => tracing::error!("Error searching layer {:?}: {}", layer, e)
            }
        }

        all_results.sort_by(|a, b| a.layer.precedence().cmp(&b.layer.precedence()));

        Ok(all_results.into_iter().take(limit).collect())
    }

    pub async fn search_text_with_threshold(
        &self,
        ctx: mk_core::types::TenantContext,
        query_text: &str,
        limit: usize,
        threshold: f32,
        filters: HashMap<String, serde_json::Value>
    ) -> Result<Vec<MemoryEntry>, Box<dyn std::error::Error + Send + Sync>> {
        let embedding_service = self
            .embedding_service
            .as_ref()
            .ok_or("Embedding service not configured")?;

        let query_vector = embedding_service.embed(query_text).await?;

        self.search_with_threshold(ctx, query_vector, limit, threshold, filters)
            .await
    }

    pub async fn add_to_layer(
        &self,
        ctx: mk_core::types::TenantContext,
        layer: MemoryLayer,
        mut entry: MemoryEntry
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(auth) = &self.auth_service {
            if !auth
                .check_permission(&ctx, "memory:write", &format!("layer:{:?}", layer))
                .await?
            {
                return Err("Unauthorized to write to this memory layer".into());
            }
        }

        let start = std::time::Instant::now();
        let layer_str = format!("{:?}", layer);
        let _span = self.telemetry.record_operation_start("add", &layer_str);

        let original_content = entry.content.clone();
        entry.content = self.governance_service.redact_pii(&entry.content);
        if entry.content != original_content {
            self.telemetry.record_governance_redaction(&layer_str);
        }

        let providers = self.providers.read().await;
        let provider = providers
            .get(&layer)
            .ok_or("No provider registered for layer")?;

        match provider.add(ctx, entry).await {
            Ok(id) => {
                self.telemetry.record_operation_success(
                    "add",
                    &layer_str,
                    start.elapsed().as_millis() as f64
                );
                Ok(id)
            }
            Err(e) => {
                self.telemetry
                    .record_operation_failure("add", &layer_str, &e.to_string());
                Err(e)
            }
        }
    }

    pub async fn delete_from_layer(
        &self,
        ctx: mk_core::types::TenantContext,
        layer: MemoryLayer,
        id: &str
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let providers = self.providers.read().await;
        let provider = providers
            .get(&layer)
            .ok_or("No provider registered for layer")?;
        provider.delete(ctx, id).await
    }

    pub async fn get_from_layer(
        &self,
        ctx: mk_core::types::TenantContext,
        layer: MemoryLayer,
        id: &str
    ) -> Result<Option<MemoryEntry>, Box<dyn std::error::Error + Send + Sync>> {
        let providers = self.providers.read().await;
        let provider = providers
            .get(&layer)
            .ok_or("No provider registered for layer")?;

        let entry = provider.get(ctx.clone(), id).await?;

        if let Some(mut entry) = entry {
            let now = chrono::Utc::now().timestamp();
            let count = entry
                .metadata
                .get("access_count")
                .and_then(|v| v.as_u64())
                .unwrap_or(0)
                + 1;

            entry
                .metadata
                .insert("access_count".to_string(), serde_json::json!(count));
            entry
                .metadata
                .insert("last_accessed_at".to_string(), serde_json::json!(now));
            entry.updated_at = now;

            provider.update(ctx, entry.clone()).await?;
            Ok(Some(entry))
        } else {
            Ok(None)
        }
    }

    pub async fn list_all_from_layer(
        &self,
        ctx: mk_core::types::TenantContext,
        layer: MemoryLayer
    ) -> Result<Vec<MemoryEntry>, Box<dyn std::error::Error + Send + Sync>> {
        let providers = self.providers.read().await;
        let provider = providers
            .get(&layer)
            .ok_or("No provider registered for layer")?;

        let (result, _) = provider.list(ctx, layer, 1000, None).await?;
        Ok(result)
    }

    pub async fn promote_memory(
        &self,
        ctx: mk_core::types::TenantContext,
        id: &str,
        source_layer: MemoryLayer,
        target_layer: MemoryLayer
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let entry = self
            .get_from_layer(ctx.clone(), source_layer, id)
            .await?
            .ok_or_else(|| format!("Memory {} not found in layer {:?}", id, source_layer))?;

        let mut promoted_entry = entry.clone();
        promoted_entry.id = format!("{}_promoted", entry.id);
        promoted_entry.layer = target_layer;

        let now = chrono::Utc::now().timestamp();
        promoted_entry.metadata.insert(
            "original_memory_id".to_string(),
            serde_json::json!(entry.id)
        );
        promoted_entry
            .metadata
            .insert("promoted_at".to_string(), serde_json::json!(now));

        self.add_to_layer(ctx, target_layer, promoted_entry).await
    }

    pub async fn promote_important_memories(
        &self,
        ctx: mk_core::types::TenantContext,
        layer: MemoryLayer
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        use crate::promotion::PromotionService;
        let promotion_service = PromotionService::new(Arc::new(MemoryManager {
            providers: self.providers.clone(),
            embedding_service: self.embedding_service.clone(),
            governance_service: self.governance_service.clone(),
            auth_service: self.auth_service.clone(),
            telemetry: self.telemetry.clone(),
            config: self.config.clone()
        }))
        .with_config(self.config.clone())
        .with_telemetry(self.telemetry.clone());

        promotion_service
            .promote_layer_memories(ctx, layer, &mk_core::types::LayerIdentifiers::default())
            .await
            .map_err(|e| {
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string()
                )) as Box<dyn std::error::Error + Send + Sync>
            })
    }

    pub async fn close_session(
        &self,
        ctx: mk_core::types::TenantContext,
        session_id: &str
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        tracing::info!("Closing session: {}", session_id);

        self.promote_important_memories(ctx.clone(), MemoryLayer::Session)
            .await?;

        self.delete_from_layer(ctx, MemoryLayer::Session, session_id)
            .await?;

        Ok(())
    }

    pub async fn close_agent(
        &self,
        ctx: mk_core::types::TenantContext,
        agent_id: &str
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        tracing::info!("Closing agent: {}", agent_id);

        self.promote_important_memories(ctx.clone(), MemoryLayer::Agent)
            .await?;

        self.delete_from_layer(ctx, MemoryLayer::Agent, agent_id)
            .await?;

        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::providers::MockProvider;
    use mk_core::types::TenantContext;

    pub(crate) fn test_ctx() -> TenantContext {
        use std::str::FromStr;
        TenantContext {
            tenant_id: mk_core::types::TenantId::from_str("test-tenant").unwrap(),
            user_id: mk_core::types::UserId::from_str("test-user").unwrap(),
            agent_id: None
        }
    }

    #[tokio::test]
    async fn test_hierarchical_search() {
        let manager = MemoryManager::new();
        let ctx = test_ctx();
        let agent_provider = Box::new(MockProvider::new());
        let session_provider = Box::new(MockProvider::new());

        manager
            .register_provider(MemoryLayer::Agent, agent_provider)
            .await;
        manager
            .register_provider(MemoryLayer::Session, session_provider)
            .await;

        let agent_entry = MemoryEntry {
            id: "agent_1".to_string(),
            content: "agent content".to_string(),
            embedding: None,
            layer: MemoryLayer::Agent,
            metadata: HashMap::new(),
            created_at: 0,
            updated_at: 0
        };

        let session_entry = MemoryEntry {
            id: "session_1".to_string(),
            content: "session content".to_string(),
            embedding: None,
            layer: MemoryLayer::Session,
            metadata: HashMap::new(),
            created_at: 0,
            updated_at: 0
        };

        manager
            .add_to_layer(ctx.clone(), MemoryLayer::Agent, agent_entry)
            .await
            .unwrap();
        manager
            .add_to_layer(ctx.clone(), MemoryLayer::Session, session_entry)
            .await
            .unwrap();

        let results = manager
            .search_hierarchical(ctx, vec![], 10, HashMap::new())
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].id, "agent_1");
        assert_eq!(results[1].id, "session_1");
    }

    #[tokio::test]
    async fn test_search_with_threshold() {
        let manager = MemoryManager::new();
        let ctx = test_ctx();
        let provider = Box::new(MockProvider::new());

        manager.register_provider(MemoryLayer::User, provider).await;

        let entry_high_score = MemoryEntry {
            id: "high_score".to_string(),
            content: "high score content".to_string(),
            embedding: None,
            layer: MemoryLayer::User,
            metadata: {
                let mut map = HashMap::new();
                map.insert("score".to_string(), serde_json::json!(0.9));
                map
            },
            created_at: 0,
            updated_at: 0
        };

        let entry_low_score = MemoryEntry {
            id: "low_score".to_string(),
            content: "low score content".to_string(),
            embedding: None,
            layer: MemoryLayer::User,
            metadata: {
                let mut map = HashMap::new();
                map.insert("score".to_string(), serde_json::json!(0.5));
                map
            },
            created_at: 0,
            updated_at: 0
        };

        manager
            .add_to_layer(ctx.clone(), MemoryLayer::User, entry_high_score)
            .await
            .unwrap();
        manager
            .add_to_layer(ctx.clone(), MemoryLayer::User, entry_low_score)
            .await
            .unwrap();

        let results = manager
            .search_with_threshold(ctx.clone(), vec![], 10, 0.7, HashMap::new())
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "high_score");

        let results = manager
            .search_with_threshold(ctx, vec![], 10, 0.3, HashMap::new())
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_search_with_threshold_no_score_in_metadata() {
        let manager = MemoryManager::new();
        let ctx = test_ctx();
        let provider = Box::new(MockProvider::new());

        manager.register_provider(MemoryLayer::User, provider).await;

        let entry = MemoryEntry {
            id: "no_score".to_string(),
            content: "no score content".to_string(),
            embedding: None,
            layer: MemoryLayer::User,
            metadata: HashMap::new(),
            created_at: 0,
            updated_at: 0
        };

        manager
            .add_to_layer(ctx.clone(), MemoryLayer::User, entry)
            .await
            .unwrap();

        let results = manager
            .search_with_threshold(ctx, vec![], 10, 0.8, HashMap::new())
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "no_score");
    }

    #[tokio::test]
    async fn test_add_to_layer_with_governance() {
        let manager = MemoryManager::new();
        let ctx = test_ctx();
        let provider = Box::new(MockProvider::new());
        manager.register_provider(MemoryLayer::User, provider).await;

        let entry = MemoryEntry {
            id: "mem_1".to_string(),
            content: "Contact user@example.com".to_string(),
            embedding: None,
            layer: MemoryLayer::User,
            metadata: HashMap::new(),
            created_at: 0,
            updated_at: 0
        };

        manager
            .add_to_layer(ctx.clone(), MemoryLayer::User, entry)
            .await
            .unwrap();

        let retrieved = manager
            .get_from_layer(ctx, MemoryLayer::User, "mem_1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(retrieved.content, "Contact [REDACTED_EMAIL]");
    }

    #[tokio::test]
    async fn test_delete_from_layer() {
        let manager = MemoryManager::new();
        let ctx = test_ctx();
        let provider = Box::new(MockProvider::new());
        manager.register_provider(MemoryLayer::User, provider).await;

        let entry = MemoryEntry {
            id: "mem_1".to_string(),
            content: "test".to_string(),
            embedding: None,
            layer: MemoryLayer::User,
            metadata: HashMap::new(),
            created_at: 0,
            updated_at: 0
        };

        manager
            .add_to_layer(ctx.clone(), MemoryLayer::User, entry)
            .await
            .unwrap();
        manager
            .delete_from_layer(ctx.clone(), MemoryLayer::User, "mem_1")
            .await
            .unwrap();

        let retrieved = manager
            .get_from_layer(ctx, MemoryLayer::User, "mem_1")
            .await
            .unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_promote_memory_manual() {
        let manager = MemoryManager::new();
        let ctx = test_ctx();
        let mock_session = Box::new(MockProvider::new());
        let mock_project = Box::new(MockProvider::new());
        manager
            .register_provider(MemoryLayer::Session, mock_session)
            .await;
        manager
            .register_provider(MemoryLayer::Project, mock_project)
            .await;

        let entry = MemoryEntry {
            id: "session_mem".to_string(),
            content: "to be promoted".to_string(),
            embedding: None,
            layer: MemoryLayer::Session,
            metadata: HashMap::new(),
            created_at: 0,
            updated_at: 0
        };

        manager
            .add_to_layer(ctx.clone(), MemoryLayer::Session, entry)
            .await
            .unwrap();
        manager
            .promote_memory(
                ctx.clone(),
                "session_mem",
                MemoryLayer::Session,
                MemoryLayer::Project
            )
            .await
            .unwrap();

        let promoted = manager
            .get_from_layer(ctx, MemoryLayer::Project, "session_mem_promoted")
            .await
            .unwrap();
        assert!(promoted.is_some());
    }

    #[tokio::test]
    async fn test_search_precedence_ordering() {
        let manager = MemoryManager::new();
        let ctx = test_ctx();
        let agent_provider = Box::new(MockProvider::new());
        let user_provider = Box::new(MockProvider::new());

        manager
            .register_provider(MemoryLayer::Agent, agent_provider)
            .await;
        manager
            .register_provider(MemoryLayer::User, user_provider)
            .await;

        let agent_entry = MemoryEntry {
            id: "agent_high_priority".to_string(),
            content: "agent content".to_string(),
            embedding: None,
            layer: MemoryLayer::Agent,
            metadata: {
                let mut map = HashMap::new();
                map.insert("score".to_string(), serde_json::json!(0.5));
                map
            },
            created_at: 0,
            updated_at: 0
        };

        let user_entry = MemoryEntry {
            id: "user_high_similarity".to_string(),
            content: "user content".to_string(),
            embedding: None,
            layer: MemoryLayer::User,
            metadata: {
                let mut map = HashMap::new();
                map.insert("score".to_string(), serde_json::json!(0.9));
                map
            },
            created_at: 0,
            updated_at: 0
        };

        manager
            .add_to_layer(ctx.clone(), MemoryLayer::Agent, agent_entry)
            .await
            .unwrap();
        manager
            .add_to_layer(ctx.clone(), MemoryLayer::User, user_entry)
            .await
            .unwrap();

        let results = manager
            .search_hierarchical(ctx, vec![], 10, HashMap::new())
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].id, "agent_high_priority");
        assert_eq!(results[1].id, "user_high_similarity");
    }

    #[tokio::test]
    async fn test_close_session_triggers_promotion() {
        let manager = MemoryManager::new().with_config(config::MemoryConfig {
            promotion_threshold: 0.5
        });
        let ctx = test_ctx();
        let mock_session = Box::new(MockProvider::new());
        let mock_project = Box::new(MockProvider::new());
        manager
            .register_provider(MemoryLayer::Session, mock_session)
            .await;
        manager
            .register_provider(MemoryLayer::Project, mock_project)
            .await;

        let entry = MemoryEntry {
            id: "important".to_string(),
            content: "highly important".to_string(),
            embedding: None,
            layer: MemoryLayer::Session,
            metadata: {
                let mut m = HashMap::new();
                m.insert("score".to_string(), serde_json::json!(1.0));
                m
            },
            created_at: 0,
            updated_at: 0
        };

        manager
            .add_to_layer(ctx.clone(), MemoryLayer::Session, entry)
            .await
            .unwrap();
        manager.close_session(ctx.clone(), "some_id").await.unwrap();

        let promoted = manager
            .list_all_from_layer(ctx, MemoryLayer::Project)
            .await
            .unwrap();
        assert!(!promoted.is_empty());
    }

    #[tokio::test]
    async fn test_search_text_with_threshold_requires_embedding_service() {
        let manager = MemoryManager::new();
        let ctx = test_ctx();
        let provider = Box::new(MockProvider::new());

        manager.register_provider(MemoryLayer::User, provider).await;

        let result = manager
            .search_text_with_threshold(ctx, "test query", 10, 0.7, HashMap::new())
            .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Embedding service not configured")
        );
    }

    #[tokio::test]
    async fn test_hierarchical_search_provider_error() {
        struct FailingProvider;
        #[async_trait::async_trait]
        impl mk_core::traits::MemoryProviderAdapter for FailingProvider {
            type Error = Box<dyn std::error::Error + Send + Sync>;
            async fn add(
                &self,
                _ctx: mk_core::types::TenantContext,
                _e: MemoryEntry
            ) -> Result<String, Self::Error> {
                Ok("id".to_string())
            }
            async fn get(
                &self,
                _ctx: mk_core::types::TenantContext,
                _id: &str
            ) -> Result<Option<MemoryEntry>, Self::Error> {
                Ok(None)
            }
            async fn search(
                &self,
                _ctx: mk_core::types::TenantContext,
                _v: Vec<f32>,
                _l: usize,
                _f: HashMap<String, serde_json::Value>
            ) -> Result<Vec<MemoryEntry>, Self::Error> {
                Err("search failed".into())
            }
            async fn update(
                &self,
                _ctx: mk_core::types::TenantContext,
                _e: MemoryEntry
            ) -> Result<(), Self::Error> {
                Ok(())
            }
            async fn delete(
                &self,
                _ctx: mk_core::types::TenantContext,
                _id: &str
            ) -> Result<(), Self::Error> {
                Ok(())
            }
            async fn list(
                &self,
                _ctx: mk_core::types::TenantContext,
                _l: MemoryLayer,
                _lim: usize,
                _c: Option<String>
            ) -> Result<(Vec<MemoryEntry>, Option<String>), Self::Error> {
                Ok((vec![], None))
            }
        }

        let manager = MemoryManager::new();
        let ctx = test_ctx();
        manager
            .register_provider(MemoryLayer::Agent, Box::new(FailingProvider))
            .await;

        let results = manager
            .search_hierarchical(ctx, vec![0.0], 10, HashMap::new())
            .await
            .unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_close_agent_triggers_promotion() {
        let manager = MemoryManager::new().with_config(config::MemoryConfig {
            promotion_threshold: 0.5,
            ..Default::default()
        });
        let ctx = test_ctx();
        let mock_agent = Box::new(MockProvider::new());
        let mock_user = Box::new(MockProvider::new());
        manager
            .register_provider(MemoryLayer::Agent, mock_agent)
            .await;
        manager
            .register_provider(MemoryLayer::User, mock_user)
            .await;

        let entry = MemoryEntry {
            id: "agent_mem".to_string(),
            content: "agent memory content".to_string(),
            embedding: None,
            layer: MemoryLayer::Agent,
            metadata: {
                let mut m = HashMap::new();
                m.insert("score".to_string(), serde_json::json!(1.0));
                m
            },
            created_at: 0,
            updated_at: 0
        };

        manager
            .add_to_layer(ctx.clone(), MemoryLayer::Agent, entry)
            .await
            .unwrap();
        manager.close_agent(ctx.clone(), "agent_id").await.unwrap();

        let promoted = manager
            .list_all_from_layer(ctx, MemoryLayer::User)
            .await
            .unwrap();
        assert!(!promoted.is_empty());
        assert_eq!(
            promoted[0].metadata.get("original_memory_id").unwrap(),
            "agent_mem"
        );
    }

    #[tokio::test]
    async fn test_search_text_with_threshold_success() {
        use crate::embedding::mock::MockEmbeddingService;
        let manager =
            MemoryManager::new().with_embedding_service(Arc::new(MockEmbeddingService::new(1536)));
        let ctx = test_ctx();

        let provider = Box::new(MockProvider::new());
        manager.register_provider(MemoryLayer::User, provider).await;

        let entry = MemoryEntry {
            id: "text_mem".to_string(),
            content: "some text content".to_string(),
            embedding: None,
            layer: MemoryLayer::User,
            metadata: {
                let mut m = HashMap::new();
                m.insert("score".to_string(), serde_json::json!(0.9));
                m
            },
            created_at: 0,
            updated_at: 0
        };

        manager
            .add_to_layer(ctx.clone(), MemoryLayer::User, entry)
            .await
            .unwrap();

        let results = manager
            .search_text_with_threshold(ctx, "query", 10, 0.5, HashMap::new())
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "text_mem");
    }

    #[tokio::test]
    async fn test_with_telemetry_and_default() {
        let telemetry = Arc::new(MemoryTelemetry::new());
        let manager = MemoryManager::default().with_telemetry(telemetry);
        assert!(manager.embedding_service.is_none());
    }

    #[tokio::test]
    async fn test_add_to_layer_no_provider() {
        let manager = MemoryManager::new();
        let ctx = test_ctx();
        let entry = MemoryEntry {
            id: "test".to_string(),
            content: "test".to_string(),
            embedding: None,
            layer: MemoryLayer::User,
            metadata: HashMap::new(),
            created_at: 0,
            updated_at: 0
        };

        let result = manager.add_to_layer(ctx, MemoryLayer::User, entry).await;
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "No provider registered for layer"
        );
    }

    #[tokio::test]
    async fn test_search_with_threshold_provider_error() {
        struct FailingProvider;
        #[async_trait::async_trait]
        impl mk_core::traits::MemoryProviderAdapter for FailingProvider {
            type Error = Box<dyn std::error::Error + Send + Sync>;
            async fn add(
                &self,
                _ctx: mk_core::types::TenantContext,
                _e: MemoryEntry
            ) -> Result<String, Self::Error> {
                Ok("id".into())
            }
            async fn get(
                &self,
                _ctx: mk_core::types::TenantContext,
                _id: &str
            ) -> Result<Option<MemoryEntry>, Self::Error> {
                Ok(None)
            }
            async fn search(
                &self,
                _ctx: mk_core::types::TenantContext,
                _v: Vec<f32>,
                _l: usize,
                _f: HashMap<String, serde_json::Value>
            ) -> Result<Vec<MemoryEntry>, Self::Error> {
                Err("search failed".into())
            }
            async fn update(
                &self,
                _ctx: mk_core::types::TenantContext,
                _e: MemoryEntry
            ) -> Result<(), Self::Error> {
                Ok(())
            }
            async fn delete(
                &self,
                _ctx: mk_core::types::TenantContext,
                _id: &str
            ) -> Result<(), Self::Error> {
                Ok(())
            }
            async fn list(
                &self,
                _ctx: mk_core::types::TenantContext,
                _l: MemoryLayer,
                _lim: usize,
                _c: Option<String>
            ) -> Result<(Vec<MemoryEntry>, Option<String>), Self::Error> {
                Ok((vec![], None))
            }
        }

        let manager = MemoryManager::new();
        let ctx = test_ctx();
        manager
            .register_provider(MemoryLayer::User, Box::new(FailingProvider))
            .await;

        let results = manager
            .search_with_threshold(ctx, vec![0.0], 10, 0.5, HashMap::new())
            .await
            .unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_promote_important_memories_error_mapping() {
        struct ErrorProvider;
        #[async_trait::async_trait]
        impl mk_core::traits::MemoryProviderAdapter for ErrorProvider {
            type Error = Box<dyn std::error::Error + Send + Sync>;
            async fn add(
                &self,
                _ctx: mk_core::types::TenantContext,
                _e: MemoryEntry
            ) -> Result<String, Self::Error> {
                Ok("id".into())
            }
            async fn get(
                &self,
                _ctx: mk_core::types::TenantContext,
                _id: &str
            ) -> Result<Option<MemoryEntry>, Self::Error> {
                Ok(None)
            }
            async fn search(
                &self,
                _ctx: mk_core::types::TenantContext,
                _v: Vec<f32>,
                _l: usize,
                _f: HashMap<String, serde_json::Value>
            ) -> Result<Vec<MemoryEntry>, Self::Error> {
                Err("list failed".into())
            }
            async fn update(
                &self,
                _ctx: mk_core::types::TenantContext,
                _e: MemoryEntry
            ) -> Result<(), Self::Error> {
                Ok(())
            }
            async fn delete(
                &self,
                _ctx: mk_core::types::TenantContext,
                _id: &str
            ) -> Result<(), Self::Error> {
                Ok(())
            }
            async fn list(
                &self,
                _ctx: mk_core::types::TenantContext,
                _l: MemoryLayer,
                _lim: usize,
                _c: Option<String>
            ) -> Result<(Vec<MemoryEntry>, Option<String>), Self::Error> {
                Err("list failed".into())
            }
        }

        let manager = MemoryManager::new();
        let ctx = test_ctx();
        manager
            .register_provider(MemoryLayer::Session, Box::new(ErrorProvider))
            .await;

        let result = manager
            .promote_important_memories(ctx, MemoryLayer::Session)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_add_to_layer_provider_error() {
        struct FailingAddProvider;
        #[async_trait::async_trait]
        impl mk_core::traits::MemoryProviderAdapter for FailingAddProvider {
            type Error = Box<dyn std::error::Error + Send + Sync>;
            async fn add(
                &self,
                _ctx: mk_core::types::TenantContext,
                _e: MemoryEntry
            ) -> Result<String, Self::Error> {
                Err("add failed".into())
            }
            async fn get(
                &self,
                _ctx: mk_core::types::TenantContext,
                _id: &str
            ) -> Result<Option<MemoryEntry>, Self::Error> {
                Ok(None)
            }
            async fn search(
                &self,
                _ctx: mk_core::types::TenantContext,
                _v: Vec<f32>,
                _l: usize,
                _f: HashMap<String, serde_json::Value>
            ) -> Result<Vec<MemoryEntry>, Self::Error> {
                Ok(vec![])
            }
            async fn update(
                &self,
                _ctx: mk_core::types::TenantContext,
                _e: MemoryEntry
            ) -> Result<(), Self::Error> {
                Ok(())
            }
            async fn delete(
                &self,
                _ctx: mk_core::types::TenantContext,
                _id: &str
            ) -> Result<(), Self::Error> {
                Ok(())
            }
            async fn list(
                &self,
                _ctx: mk_core::types::TenantContext,
                _l: MemoryLayer,
                _lim: usize,
                _c: Option<String>
            ) -> Result<(Vec<MemoryEntry>, Option<String>), Self::Error> {
                Ok((vec![], None))
            }
        }

        let manager = MemoryManager::new();
        let ctx = test_ctx();
        manager
            .register_provider(MemoryLayer::User, Box::new(FailingAddProvider))
            .await;

        let entry = MemoryEntry {
            id: "test".to_string(),
            content: "test".to_string(),
            embedding: None,
            layer: MemoryLayer::User,
            metadata: HashMap::new(),
            created_at: 0,
            updated_at: 0
        };

        let result = manager.add_to_layer(ctx, MemoryLayer::User, entry).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "add failed");
    }
}
