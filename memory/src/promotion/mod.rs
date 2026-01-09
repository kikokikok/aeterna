use crate::manager::MemoryManager;
use anyhow::{Context, Result};
use mk_core::types::{MemoryEntry, MemoryLayer};
use std::sync::Arc;

pub struct PromotionService {
    memory_manager: Arc<MemoryManager>,
    promotion_threshold: f32,
    promote_important: bool
}

impl PromotionService {
    pub fn new(memory_manager: Arc<MemoryManager>) -> Self {
        Self {
            memory_manager,
            promotion_threshold: 0.8,
            promote_important: true
        }
    }

    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.promotion_threshold = threshold;
        self
    }

    pub fn with_promote_important(mut self, promote: bool) -> Self {
        self.promote_important = promote;
        self
    }

    pub async fn evaluate_and_promote(&self, entry: &MemoryEntry) -> Result<Option<String>> {
        if !self.promote_important {
            return Ok(None);
        }

        let score = self.calculate_importance_score(entry);

        if score >= self.promotion_threshold {
            if let Some(target) = self.determine_target_layer(entry.layer) {
                tracing::info!(
                    "Promoting memory {} from {:?} to {:?} (score: {:.2})",
                    entry.id,
                    entry.layer,
                    target,
                    score
                );

                let mut promoted_entry = entry.clone();
                promoted_entry.id = format!("{}_promoted", entry.id);
                promoted_entry.layer = target;
                promoted_entry.metadata.insert(
                    "original_memory_id".to_string(),
                    serde_json::json!(entry.id)
                );
                promoted_entry.metadata.insert(
                    "promoted_at".to_string(),
                    serde_json::json!(chrono::Utc::now().timestamp())
                );
                promoted_entry
                    .metadata
                    .insert("promotion_score".to_string(), serde_json::json!(score));

                let new_id = self
                    .memory_manager
                    .add_to_layer(target, promoted_entry)
                    .await
                    .map_err(|e| anyhow::anyhow!(e))
                    .context("Failed to add promoted memory to target layer")?;

                return Ok(Some(new_id));
            }
        }

        Ok(None)
    }

    fn calculate_importance_score(&self, entry: &MemoryEntry) -> f32 {
        let explicit_score = entry
            .metadata
            .get("score")
            .and_then(|v| v.as_f64())
            .map(|v| v as f32)
            .unwrap_or(0.0);

        let access_count = entry
            .metadata
            .get("access_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as f32;

        let last_accessed = entry
            .metadata
            .get("last_accessed_at")
            .and_then(|v| v.as_i64())
            .unwrap_or_else(|| chrono::Utc::now().timestamp()) as f32;

        let now_ts = chrono::Utc::now().timestamp() as f32;
        let days_since_last_access = (now_ts - last_accessed).max(0.0) / 86400.0;
        let recency_score = (1.0f32 - days_since_last_access).max(0.0f32);

        let frequency_score = (access_count / 10.0).min(1.0);

        (explicit_score * 0.6) + (frequency_score * 0.3) + (recency_score * 0.1)
    }

    pub async fn promote_layer_memories(
        &self,
        layer: MemoryLayer,
        _identifiers: &mk_core::types::LayerIdentifiers
    ) -> Result<Vec<String>> {
        let entries = self
            .memory_manager
            .list_all_from_layer(layer)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

        let mut promoted_ids = Vec::new();
        for entry in entries {
            if let Some(new_id) = self.evaluate_and_promote(&entry).await? {
                promoted_ids.push(new_id);
            }
        }
        Ok(promoted_ids)
    }

    fn determine_target_layer(&self, current_layer: MemoryLayer) -> Option<MemoryLayer> {
        match current_layer {
            MemoryLayer::Agent => Some(MemoryLayer::User),
            MemoryLayer::Session => Some(MemoryLayer::Project),
            _ => None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::MockProvider;
    use mk_core::types::MemoryEntry;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_evaluate_and_promote_high_score() {
        let manager = Arc::new(MemoryManager::new());
        let mock_session = Box::new(MockProvider::new());
        let mock_project = Box::new(MockProvider::new());

        manager
            .register_provider(MemoryLayer::Session, mock_session)
            .await;
        manager
            .register_provider(MemoryLayer::Project, mock_project)
            .await;

        let service = PromotionService::new(manager.clone()).with_threshold(0.7);

        let entry = MemoryEntry {
            id: "mem_1".to_string(),
            content: "important stuff".to_string(),
            embedding: None,
            layer: MemoryLayer::Session,
            metadata: {
                let mut m = HashMap::new();
                m.insert("score".to_string(), serde_json::json!(1.0));
                m.insert("access_count".to_string(), serde_json::json!(10));
                m.insert(
                    "last_accessed_at".to_string(),
                    serde_json::json!(chrono::Utc::now().timestamp())
                );
                m
            },
            created_at: 0,
            updated_at: 0
        };

        let result = service.evaluate_and_promote(&entry).await.unwrap();
        assert!(result.is_some());
        assert!(result.unwrap().contains("mem_1_promoted"));

        let promoted = manager
            .get_from_layer(MemoryLayer::Project, "mem_1_promoted")
            .await
            .unwrap();
        assert!(promoted.is_some());
        assert_eq!(
            promoted
                .unwrap()
                .metadata
                .get("original_memory_id")
                .unwrap(),
            "mem_1"
        );
    }

    #[tokio::test]
    async fn test_evaluate_and_promote_low_score() {
        let manager = Arc::new(MemoryManager::new());
        let service = PromotionService::new(manager).with_threshold(0.8);

        let entry = MemoryEntry {
            id: "mem_low".to_string(),
            content: "boring stuff".to_string(),
            embedding: None,
            layer: MemoryLayer::Session,
            metadata: {
                let mut m = HashMap::new();
                m.insert("score".to_string(), serde_json::json!(0.2));
                m
            },
            created_at: 0,
            updated_at: 0
        };

        let result = service.evaluate_and_promote(&entry).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_calculate_importance_score() {
        let manager = Arc::new(MemoryManager::new());
        let service = PromotionService::new(manager);

        let entry = MemoryEntry {
            id: "test".to_string(),
            content: "content".to_string(),
            embedding: None,
            layer: MemoryLayer::Session,
            metadata: {
                let mut m = HashMap::new();
                m.insert("score".to_string(), serde_json::json!(0.5));
                m.insert("access_count".to_string(), serde_json::json!(10));
                m.insert(
                    "last_accessed_at".to_string(),
                    serde_json::json!(chrono::Utc::now().timestamp())
                );
                m
            },
            created_at: 0,
            updated_at: 0
        };

        let score = service.calculate_importance_score(&entry);
        assert!((score - 0.7).abs() < 0.01);
    }
}
