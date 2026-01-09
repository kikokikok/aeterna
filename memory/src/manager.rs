use mk_core::traits::EmbeddingService;
use mk_core::traits::MemoryProviderAdapter;
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
    >
}

impl MemoryManager {
    pub fn new() -> Self {
        Self {
            providers: Arc::new(RwLock::new(HashMap::new())),
            embedding_service: None
        }
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
        query_vector: Vec<f32>,
        limit: usize,
        filters: HashMap<String, serde_json::Value>
    ) -> Result<Vec<MemoryEntry>, Box<dyn std::error::Error + Send + Sync>> {
        let providers = self.providers.read().await;
        let mut all_results = Vec::new();

        for (layer, provider) in providers.iter() {
            match provider
                .search(query_vector.clone(), limit, filters.clone())
                .await
            {
                Ok(results) => {
                    for mut entry in results {
                        entry.layer = *layer;
                        all_results.push(entry);
                    }
                }
                Err(e) => tracing::error!("Error searching layer {:?}: {}", layer, e)
            }
        }

        all_results.sort_by(|a, b| a.layer.precedence().cmp(&b.layer.precedence()));

        Ok(all_results.into_iter().take(limit).collect())
    }

    pub async fn search_with_threshold(
        &self,
        query_vector: Vec<f32>,
        limit: usize,
        threshold: f32,
        filters: HashMap<String, serde_json::Value>
    ) -> Result<Vec<MemoryEntry>, Box<dyn std::error::Error + Send + Sync>> {
        let providers = self.providers.read().await;
        let mut all_results = Vec::new();

        for (layer, provider) in providers.iter() {
            match provider
                .search(query_vector.clone(), limit, filters.clone())
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

        self.search_with_threshold(query_vector, limit, threshold, filters)
            .await
    }

    pub async fn add_to_layer(
        &self,
        layer: MemoryLayer,
        entry: MemoryEntry
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let providers = self.providers.read().await;
        let provider = providers
            .get(&layer)
            .ok_or("No provider registered for layer")?;
        provider.add(entry).await
    }

    pub async fn delete_from_layer(
        &self,
        layer: MemoryLayer,
        id: &str
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let providers = self.providers.read().await;
        let provider = providers
            .get(&layer)
            .ok_or("No provider registered for layer")?;
        provider.delete(id).await
    }

    pub async fn get_from_layer(
        &self,
        layer: MemoryLayer,
        id: &str
    ) -> Result<Option<MemoryEntry>, Box<dyn std::error::Error + Send + Sync>> {
        let providers = self.providers.read().await;
        let provider = providers
            .get(&layer)
            .ok_or("No provider registered for layer")?;
        provider.get(id).await
    }

    pub async fn list_all_from_layer(
        &self,
        layer: MemoryLayer
    ) -> Result<Vec<MemoryEntry>, Box<dyn std::error::Error + Send + Sync>> {
        let providers = self.providers.read().await;
        let provider = providers
            .get(&layer)
            .ok_or("No provider registered for layer")?;

        let result = provider.search(vec![0.0; 0], 1000, HashMap::new()).await?;
        Ok(result)
    }

    pub async fn promote_important_memories(
        &self,
        layer: MemoryLayer
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        use crate::promotion::PromotionService;
        let promotion_service = PromotionService::new(Arc::new(MemoryManager {
            providers: self.providers.clone(),
            embedding_service: self.embedding_service.clone()
        }));

        promotion_service
            .promote_layer_memories(layer, &mk_core::types::LayerIdentifiers::default())
            .await
            .map_err(|e| e.into())
    }

    pub async fn close_session(
        &self,
        session_id: &str
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        tracing::info!("Closing session: {}", session_id);

        self.promote_important_memories(MemoryLayer::Session)
            .await?;

        self.delete_from_layer(MemoryLayer::Session, session_id)
            .await?;

        Ok(())
    }

    pub async fn close_agent(
        &self,
        agent_id: &str
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        tracing::info!("Closing agent: {}", agent_id);

        self.promote_important_memories(MemoryLayer::Agent).await?;

        self.delete_from_layer(MemoryLayer::Agent, agent_id).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::MockProvider;

    #[tokio::test]
    async fn test_hierarchical_search() {
        let manager = MemoryManager::new();
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
            .add_to_layer(MemoryLayer::Agent, agent_entry)
            .await
            .unwrap();
        manager
            .add_to_layer(MemoryLayer::Session, session_entry)
            .await
            .unwrap();

        let results = manager
            .search_hierarchical(vec![], 10, HashMap::new())
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].id, "agent_1");
        assert_eq!(results[1].id, "session_1");
    }

    #[tokio::test]
    async fn test_search_with_threshold() {
        let manager = MemoryManager::new();
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
            .add_to_layer(MemoryLayer::User, entry_high_score)
            .await
            .unwrap();
        manager
            .add_to_layer(MemoryLayer::User, entry_low_score)
            .await
            .unwrap();

        let results = manager
            .search_with_threshold(vec![], 10, 0.7, HashMap::new())
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "high_score");

        let results = manager
            .search_with_threshold(vec![], 10, 0.3, HashMap::new())
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_search_with_threshold_no_score_in_metadata() {
        let manager = MemoryManager::new();
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
            .add_to_layer(MemoryLayer::User, entry)
            .await
            .unwrap();

        let results = manager
            .search_with_threshold(vec![], 10, 0.8, HashMap::new())
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "no_score");
    }

    #[tokio::test]
    async fn test_search_text_with_threshold_requires_embedding_service() {
        let manager = MemoryManager::new();
        let provider = Box::new(MockProvider::new());

        manager.register_provider(MemoryLayer::User, provider).await;

        let result = manager
            .search_text_with_threshold("test query", 10, 0.7, HashMap::new())
            .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Embedding service not configured")
        );
    }
}
