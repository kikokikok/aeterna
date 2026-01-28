pub mod qdrant;

use async_trait::async_trait;
use mk_core::traits::MemoryProviderAdapter;
use mk_core::types::{MemoryEntry, MemoryLayer};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct MockProvider {
    entries: Arc<RwLock<HashMap<String, MemoryEntry>>>
}

impl MockProvider {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new()))
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

    async fn add(
        &self,
        ctx: mk_core::types::TenantContext,
        entry: MemoryEntry
    ) -> Result<String, Self::Error> {
        let mut entries = self.entries.write().await;
        let id = entry.id.clone();
        let mut entry = entry;
        entry
            .metadata
            .insert("tenant_id".to_string(), serde_json::json!(ctx.tenant_id));
        entries.insert(id.clone(), entry);
        Ok(id)
    }

    async fn search(
        &self,
        ctx: mk_core::types::TenantContext,
        _query_vector: Vec<f32>,
        limit: usize,
        filters: HashMap<String, serde_json::Value>
    ) -> Result<Vec<MemoryEntry>, Self::Error> {
        let entries = self.entries.read().await;
        let results: Vec<MemoryEntry> = entries
            .values()
            .filter(|entry| {
                // Ensure tenant isolation in mock search
                if entry.metadata.get("tenant_id") != Some(&serde_json::json!(ctx.tenant_id)) {
                    return false;
                }
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

    async fn get(
        &self,
        ctx: mk_core::types::TenantContext,
        id: &str
    ) -> Result<Option<MemoryEntry>, Self::Error> {
        let entries = self.entries.read().await;
        if let Some(entry) = entries.get(id)
            && entry.metadata.get("tenant_id") == Some(&serde_json::json!(ctx.tenant_id))
        {
            return Ok(Some(entry.clone()));
        }
        Ok(None)
    }

    async fn update(
        &self,
        ctx: mk_core::types::TenantContext,
        entry: MemoryEntry
    ) -> Result<(), Self::Error> {
        let mut entries = self.entries.write().await;
        if let Some(existing) = entries.get(&entry.id)
            && existing.metadata.get("tenant_id") == Some(&serde_json::json!(ctx.tenant_id))
        {
            let mut entry = entry;
            entry
                .metadata
                .insert("tenant_id".to_string(), serde_json::json!(ctx.tenant_id));
            entries.insert(entry.id.clone(), entry);
            return Ok(());
        }
        Err("Entry not found or access denied".into())
    }

    async fn delete(
        &self,
        ctx: mk_core::types::TenantContext,
        id: &str
    ) -> Result<(), Self::Error> {
        let mut entries = self.entries.write().await;
        if let Some(existing) = entries.get(id)
            && existing.metadata.get("tenant_id") == Some(&serde_json::json!(ctx.tenant_id))
        {
            entries.remove(id);
        }
        Ok(())
    }

    async fn list(
        &self,
        ctx: mk_core::types::TenantContext,
        layer: MemoryLayer,
        limit: usize,
        cursor: Option<String>
    ) -> Result<(Vec<MemoryEntry>, Option<String>), Self::Error> {
        let entries = self.entries.read().await;
        let mut results: Vec<MemoryEntry> = entries
            .values()
            .filter(|e| {
                e.layer == layer
                    && e.metadata.get("tenant_id") == Some(&serde_json::json!(ctx.tenant_id))
            })
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
    use mk_core::types::{MemoryLayer, TenantContext};

    fn test_ctx() -> TenantContext {
        use std::str::FromStr;
        TenantContext {
            tenant_id: mk_core::types::TenantId::from_str("test-tenant").unwrap(),
            user_id: mk_core::types::UserId::from_str("test-user").unwrap(),
            agent_id: None
        }
    }

    #[tokio::test]
    async fn test_mock_provider_basic_ops() {
        let provider = MockProvider::new();
        let ctx = test_ctx();
        let entry = MemoryEntry {
            summaries: std::collections::HashMap::new(),
            context_vector: None,
            importance_score: None,
            id: "test1".to_string(),
            content: "hello world".to_string(),
            embedding: None,
            layer: MemoryLayer::Agent,
            metadata: HashMap::new(),
            created_at: 0,
            updated_at: 0
        };

        provider.add(ctx.clone(), entry.clone()).await.unwrap();

        let retrieved = provider.get(ctx.clone(), "test1").await.unwrap().unwrap();
        assert_eq!(retrieved.content, "hello world");

        let mut updated = entry.clone();
        updated.content = "updated".to_string();
        provider.update(ctx.clone(), updated).await.unwrap();
        assert_eq!(
            provider
                .get(ctx.clone(), "test1")
                .await
                .unwrap()
                .unwrap()
                .content,
            "updated"
        );

        let (list, _) = provider
            .list(ctx.clone(), MemoryLayer::Agent, 10, None)
            .await
            .unwrap();
        assert_eq!(list.len(), 1);

        provider.delete(ctx.clone(), "test1").await.unwrap();
        assert!(provider.get(ctx.clone(), "test1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_mock_provider_update_nonexistent() {
        let provider = MockProvider::new();
        let ctx = test_ctx();
        let entry = MemoryEntry {
            summaries: std::collections::HashMap::new(),
            context_vector: None,
            importance_score: None,
            id: "ghost".to_string(),
            content: "ghost".to_string(),
            embedding: None,
            layer: MemoryLayer::Agent,
            metadata: HashMap::new(),
            created_at: 0,
            updated_at: 0
        };
        assert!(provider.update(ctx, entry).await.is_err());
    }

    #[tokio::test]
    async fn test_mock_provider_search() {
        let provider = MockProvider::new();
        let ctx = test_ctx();
        let entry1 = MemoryEntry {
            summaries: std::collections::HashMap::new(),
            context_vector: None,
            importance_score: None,
            id: "1".to_string(),
            content: "one".to_string(),
            embedding: None,
            layer: MemoryLayer::Agent,
            metadata: {
                let mut m = HashMap::new();
                m.insert("type".to_string(), serde_json::json!("a"));
                m
            },
            created_at: 0,
            updated_at: 0
        };
        let entry2 = MemoryEntry {
            summaries: std::collections::HashMap::new(),
            context_vector: None,
            importance_score: None,
            id: "2".to_string(),
            content: "two".to_string(),
            embedding: None,
            layer: MemoryLayer::Agent,
            metadata: {
                let mut m = HashMap::new();
                m.insert("type".to_string(), serde_json::json!("b"));
                m
            },
            created_at: 0,
            updated_at: 0
        };

        provider.add(ctx.clone(), entry1).await.unwrap();
        provider.add(ctx.clone(), entry2).await.unwrap();

        let mut filters = HashMap::new();
        filters.insert("type".to_string(), serde_json::json!("a"));

        let results = provider.search(ctx, vec![], 10, filters).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "1");
    }

    #[tokio::test]
    async fn test_mock_provider_list_pagination() {
        let provider = MockProvider::new();
        let ctx = test_ctx();
        for i in 0..5 {
            let entry = MemoryEntry {
                summaries: std::collections::HashMap::new(),
                context_vector: None,
                importance_score: None,
                id: format!("{}", i),
                content: format!("content {}", i),
                embedding: None,
                layer: MemoryLayer::Agent,
                metadata: HashMap::new(),
                created_at: 0,
                updated_at: 0
            };
            provider.add(ctx.clone(), entry).await.unwrap();
        }

        let (page1, cursor) = provider
            .list(ctx.clone(), MemoryLayer::Agent, 2, None)
            .await
            .unwrap();
        assert_eq!(page1.len(), 2);
        assert!(cursor.is_some());

        let (page2, cursor2) = provider
            .list(ctx.clone(), MemoryLayer::Agent, 2, cursor)
            .await
            .unwrap();
        assert_eq!(page2.len(), 2);
        assert!(cursor2.is_some());

        let (page3, cursor3) = provider
            .list(ctx.clone(), MemoryLayer::Agent, 2, cursor2)
            .await
            .unwrap();
        assert_eq!(page3.len(), 1);
        assert!(cursor3.is_none());
    }
}
