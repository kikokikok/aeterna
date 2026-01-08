use async_trait::async_trait;
use mk_core::traits::MemoryProviderAdapter;
use mk_core::types::{MemoryEntry, MemoryLayer};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct MockProvider {
    entries: Arc<RwLock<HashMap<String, MemoryEntry>>>,
}

impl MockProvider {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for MockProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MemoryProviderAdapter for MockProvider {
    type Error = Box<dyn std::error::Error + Send + Sync>;

    async fn add(&self, entry: MemoryEntry) -> Result<String, Self::Error> {
        let mut entries = self.entries.write().await;
        let id = entry.id.clone();
        entries.insert(id.clone(), entry);
        Ok(id)
    }

    async fn search(
        &self,
        _query_vector: Vec<f32>,
        limit: usize,
        filters: HashMap<String, serde_json::Value>,
    ) -> Result<Vec<MemoryEntry>, Self::Error> {
        let entries = self.entries.read().await;
        let results: Vec<MemoryEntry> = entries
            .values()
            .filter(|entry| {
                for (key, val) in &filters {
                    if entry.metadata.get(key) != Some(val) {
                        return false;
                    }
                }
                true
            })
            .take(limit)
            .cloned()
            .collect();
        Ok(results)
    }

    async fn get(&self, id: &str) -> Result<Option<MemoryEntry>, Self::Error> {
        let entries = self.entries.read().await;
        Ok(entries.get(id).cloned())
    }

    async fn update(&self, entry: MemoryEntry) -> Result<(), Self::Error> {
        let mut entries = self.entries.write().await;
        if entries.contains_key(&entry.id) {
            entries.insert(entry.id.clone(), entry);
            Ok(())
        } else {
            Err("Entry not found".into())
        }
    }

    async fn delete(&self, id: &str) -> Result<(), Self::Error> {
        let mut entries = self.entries.write().await;
        entries.remove(id);
        Ok(())
    }

    async fn list(
        &self,
        layer: MemoryLayer,
        limit: usize,
        cursor: Option<String>,
    ) -> Result<(Vec<MemoryEntry>, Option<String>), Self::Error> {
        let entries = self.entries.read().await;
        let mut results: Vec<MemoryEntry> = entries
            .values()
            .filter(|e| e.layer == layer)
            .collect::<Vec<_>>()
            .into_iter()
            .cloned()
            .collect();

        results.sort_by(|a, b| a.id.cmp(&b.id));

        let start_index = if let Some(c) = cursor {
            results
                .iter()
                .position(|e| e.id == c)
                .map(|p| p + 1)
                .unwrap_or(0)
        } else {
            0
        };

        let page = results
            .iter()
            .skip(start_index)
            .take(limit)
            .cloned()
            .collect::<Vec<_>>();
        let next_cursor = if page.len() == limit && results.len() > start_index + limit {
            page.last().map(|e| e.id.clone())
        } else {
            None
        };

        Ok((page, next_cursor))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mk_core::types::MemoryLayer;

    #[tokio::test]
    async fn test_mock_provider_basic_ops() {
        let provider = MockProvider::new();
        let entry = MemoryEntry {
            id: "test1".to_string(),
            content: "hello world".to_string(),
            embedding: None,
            layer: MemoryLayer::Agent,
            metadata: HashMap::new(),
            created_at: 0,
            updated_at: 0,
        };

        provider.add(entry.clone()).await.unwrap();

        let retrieved = provider.get("test1").await.unwrap().unwrap();
        assert_eq!(retrieved.content, "hello world");

        let (list, _) = provider.list(MemoryLayer::Agent, 10, None).await.unwrap();
        assert_eq!(list.len(), 1);

        provider.delete("test1").await.unwrap();
        assert!(provider.get("test1").await.unwrap().is_none());
    }
}
