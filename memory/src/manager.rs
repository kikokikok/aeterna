use mk_core::traits::MemoryProviderAdapter;
use mk_core::types::{MemoryEntry, MemoryLayer};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub type ProviderMap = HashMap<
    MemoryLayer,
    Box<dyn MemoryProviderAdapter<Error = Box<dyn std::error::Error + Send + Sync>> + Send + Sync>,
>;

pub struct MemoryManager {
    providers: Arc<RwLock<ProviderMap>>,
}

impl MemoryManager {
    pub fn new() -> Self {
        Self {
            providers: Arc::new(RwLock::new(HashMap::new())),
        }
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
                + Sync,
        >,
    ) {
        let mut providers = self.providers.write().await;
        providers.insert(layer, provider);
    }

    pub async fn search_hierarchical(
        &self,
        query_vector: Vec<f32>,
        limit: usize,
        filters: HashMap<String, serde_json::Value>,
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
                Err(e) => tracing::error!("Error searching layer {:?}: {}", layer, e),
            }
        }

        all_results.sort_by(|a, b| a.layer.precedence().cmp(&b.layer.precedence()));

        Ok(all_results.into_iter().take(limit).collect())
    }

    pub async fn add_to_layer(
        &self,
        layer: MemoryLayer,
        entry: MemoryEntry,
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
        id: &str,
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
        id: &str,
    ) -> Result<Option<MemoryEntry>, Box<dyn std::error::Error + Send + Sync>> {
        let providers = self.providers.read().await;
        let provider = providers
            .get(&layer)
            .ok_or("No provider registered for layer")?;
        provider.get(id).await
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
            updated_at: 0,
        };

        let session_entry = MemoryEntry {
            id: "session_1".to_string(),
            content: "session content".to_string(),
            embedding: None,
            layer: MemoryLayer::Session,
            metadata: HashMap::new(),
            created_at: 0,
            updated_at: 0,
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
}
