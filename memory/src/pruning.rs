use mk_core::traits::LlmService;
use mk_core::types::{MemoryEntry, MemoryOperation, MemoryTrajectoryEvent, TenantContext};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct PruningManager {
    trajectories: Arc<RwLock<HashMap<String, Vec<MemoryTrajectoryEvent>>>>,
}

impl PruningManager {
    pub fn new(trajectories: Arc<RwLock<HashMap<String, Vec<MemoryTrajectoryEvent>>>>) -> Self {
        Self { trajectories }
    }

    pub async fn evaluate(&self, entry: &MemoryEntry, threshold: f32) -> bool {
        let score = entry.importance_score.unwrap_or(0.5);
        if score < threshold {
            return true;
        }

        let trajectories = self.trajectories.read().await;
        if let Some(history) = trajectories.get(&entry.id) {
            let last_events = if history.len() > 10 {
                &history[history.len() - 10..]
            } else {
                &history[..]
            };

            let negative_rewards = last_events
                .iter()
                .filter(|e| {
                    if let Some(reward) = &e.reward {
                        reward.score < 0.0
                    } else {
                        false
                    }
                })
                .count();

            if negative_rewards >= 3 {
                return true;
            }

            let has_recent_utility = last_events
                .iter()
                .any(|e| matches!(e.operation, MemoryOperation::Retrieve) || e.reward.is_some());

            if !has_recent_utility && history.len() >= 20 {
                return true;
            }
        }

        false
    }
}

pub struct CompressionManager {
    llm_service:
        Arc<dyn LlmService<Error = Box<dyn std::error::Error + Send + Sync>> + Send + Sync>,
    trajectories: Arc<RwLock<HashMap<String, Vec<MemoryTrajectoryEvent>>>>,
}

impl CompressionManager {
    pub fn new(
        llm_service: Arc<
            dyn LlmService<Error = Box<dyn std::error::Error + Send + Sync>> + Send + Sync,
        >,
        trajectories: Arc<RwLock<HashMap<String, Vec<MemoryTrajectoryEvent>>>>,
    ) -> Self {
        Self {
            llm_service,
            trajectories,
        }
    }

    pub async fn compress(
        &self,
        _ctx: &TenantContext,
        memories: &[MemoryEntry],
    ) -> Result<MemoryEntry, Box<dyn std::error::Error + Send + Sync>> {
        if memories.is_empty() {
            return Err("No memories to compress".into());
        }

        let mut prompt = String::from(
            "Compress the following related memories into a single, high-density summary. Preserve all key facts and entities:\n\n",
        );
        for (i, m) in memories.iter().enumerate() {
            prompt.push_str(&format!("{}. {}\n", i + 1, m.content));
        }

        let compressed_content = self.llm_service.generate(&prompt).await?;

        let mut metadata = HashMap::new();
        let source_ids: Vec<String> = memories.iter().map(|m| m.id.clone()).collect();
        metadata.insert("compressed_from".to_string(), serde_json::json!(source_ids));
        metadata.insert(
            "compression_ratio".to_string(),
            serde_json::json!(memories.len()),
        );

        let avg_importance = memories
            .iter()
            .filter_map(|m| m.importance_score)
            .sum::<f32>()
            / memories.len() as f32;

        let entry = MemoryEntry {
            id: format!("compressed_{}", uuid::Uuid::new_v4()),
            content: compressed_content,
            embedding: None,
            layer: memories[0].layer,
            summaries: HashMap::new(),
            context_vector: None,
            importance_score: Some(avg_importance),
            metadata,
            created_at: chrono::Utc::now().timestamp(),
            updated_at: chrono::Utc::now().timestamp(),
        };

        let source_trajectories: Vec<Vec<MemoryTrajectoryEvent>> = {
            let trajectories = self.trajectories.read().await;
            memories
                .iter()
                .filter_map(|m| trajectories.get(&m.id).cloned())
                .collect()
        };

        if !source_trajectories.is_empty() {
            let mut trajectories = self.trajectories.write().await;
            let mut events = Vec::new();

            for (i, source_history) in source_trajectories.into_iter().enumerate() {
                for event in source_history {
                    if event.reward.is_some() {
                        events.push(MemoryTrajectoryEvent {
                            operation: MemoryOperation::Noop,
                            entry_id: entry.id.clone(),
                            reward: event.reward,
                            reasoning: Some(format!(
                                "Inherited reward from source memory {}",
                                memories[i].id
                            )),
                            timestamp: chrono::Utc::now().timestamp(),
                        });
                    }
                }
            }

            if !events.is_empty() {
                trajectories.insert(entry.id.clone(), events);
            }
        }

        Ok(entry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::mock::MockLlmService;
    use mk_core::types::{MemoryLayer, RewardSignal, RewardType};

    fn create_test_entry(id: &str, importance: Option<f32>) -> MemoryEntry {
        MemoryEntry {
            id: id.to_string(),
            content: format!("Content for {}", id),
            embedding: None,
            layer: MemoryLayer::User,
            summaries: HashMap::new(),
            context_vector: None,
            importance_score: importance,
            metadata: HashMap::new(),
            created_at: 0,
            updated_at: 0,
        }
    }

    fn create_trajectory_event(
        entry_id: &str,
        operation: MemoryOperation,
        reward: Option<RewardSignal>,
    ) -> MemoryTrajectoryEvent {
        MemoryTrajectoryEvent {
            operation,
            entry_id: entry_id.to_string(),
            reward,
            reasoning: None,
            timestamp: chrono::Utc::now().timestamp(),
        }
    }

    #[tokio::test]
    async fn test_pruning_low_importance_score_triggers_prune() {
        let trajectories = Arc::new(RwLock::new(HashMap::new()));
        let manager = PruningManager::new(trajectories);

        let entry = create_test_entry("low-score", Some(0.1));
        let should_prune = manager.evaluate(&entry, 0.3).await;

        assert!(should_prune, "Low importance score should trigger pruning");
    }

    #[tokio::test]
    async fn test_pruning_high_importance_score_no_prune() {
        let trajectories = Arc::new(RwLock::new(HashMap::new()));
        let manager = PruningManager::new(trajectories);

        let entry = create_test_entry("high-score", Some(0.9));
        let should_prune = manager.evaluate(&entry, 0.3).await;

        assert!(
            !should_prune,
            "High importance score should not trigger pruning"
        );
    }

    #[tokio::test]
    async fn test_pruning_three_negative_rewards_triggers_prune() {
        let trajectories = Arc::new(RwLock::new(HashMap::new()));
        let manager = PruningManager::new(trajectories.clone());

        let entry = create_test_entry("negative-rewards", Some(0.8));

        {
            let mut t = trajectories.write().await;
            let events = vec![
                create_trajectory_event(
                    "negative-rewards",
                    MemoryOperation::Retrieve,
                    Some(RewardSignal {
                        reward_type: RewardType::Irrelevant,
                        score: -0.5,
                        reasoning: None,
                        agent_id: None,
                        timestamp: chrono::Utc::now().timestamp(),
                    }),
                ),
                create_trajectory_event(
                    "negative-rewards",
                    MemoryOperation::Retrieve,
                    Some(RewardSignal {
                        reward_type: RewardType::Irrelevant,
                        score: -0.3,
                        reasoning: None,
                        agent_id: None,
                        timestamp: chrono::Utc::now().timestamp(),
                    }),
                ),
                create_trajectory_event(
                    "negative-rewards",
                    MemoryOperation::Retrieve,
                    Some(RewardSignal {
                        reward_type: RewardType::Irrelevant,
                        score: -0.8,
                        reasoning: None,
                        agent_id: None,
                        timestamp: chrono::Utc::now().timestamp(),
                    }),
                ),
            ];
            t.insert("negative-rewards".to_string(), events);
        }

        let should_prune = manager.evaluate(&entry, 0.3).await;
        assert!(
            should_prune,
            "Three negative rewards should trigger pruning even with high importance"
        );
    }

    #[tokio::test]
    async fn test_pruning_no_utility_in_long_history_triggers_prune() {
        let trajectories = Arc::new(RwLock::new(HashMap::new()));
        let manager = PruningManager::new(trajectories.clone());

        let entry = create_test_entry("stale-memory", Some(0.6));

        {
            let mut t = trajectories.write().await;
            let events: Vec<_> = (0..25)
                .map(|_| create_trajectory_event("stale-memory", MemoryOperation::Add, None))
                .collect();
            t.insert("stale-memory".to_string(), events);
        }

        let should_prune = manager.evaluate(&entry, 0.3).await;
        assert!(
            should_prune,
            "Memory with no recent utility and long history should be pruned"
        );
    }

    #[tokio::test]
    async fn test_pruning_recent_retrieval_prevents_prune() {
        let trajectories = Arc::new(RwLock::new(HashMap::new()));
        let manager = PruningManager::new(trajectories.clone());

        let entry = create_test_entry("recently-used", Some(0.6));

        {
            let mut t = trajectories.write().await;
            let mut events: Vec<_> = (0..20)
                .map(|_| create_trajectory_event("recently-used", MemoryOperation::Add, None))
                .collect();
            events.push(create_trajectory_event(
                "recently-used",
                MemoryOperation::Retrieve,
                None,
            ));
            t.insert("recently-used".to_string(), events);
        }

        let should_prune = manager.evaluate(&entry, 0.3).await;
        assert!(!should_prune, "Recent retrieval should prevent pruning");
    }

    #[tokio::test]
    async fn test_pruning_default_importance_score() {
        let trajectories = Arc::new(RwLock::new(HashMap::new()));
        let manager = PruningManager::new(trajectories);

        let entry = create_test_entry("no-score", None);
        let should_prune = manager.evaluate(&entry, 0.3).await;

        assert!(
            !should_prune,
            "Default importance (0.5) should not trigger pruning with 0.3 threshold"
        );
    }

    #[tokio::test]
    async fn test_compression_empty_memories_error() {
        let trajectories = Arc::new(RwLock::new(HashMap::new()));
        let llm = Arc::new(MockLlmService::new());
        let manager = CompressionManager::new(llm, trajectories);

        let ctx = TenantContext::default();
        let result = manager.compress(&ctx, &[]).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No memories"));
    }

    #[tokio::test]
    async fn test_compression_creates_merged_entry() {
        let trajectories = Arc::new(RwLock::new(HashMap::new()));
        let llm = Arc::new(MockLlmService::new());
        let manager = CompressionManager::new(llm, trajectories);

        let ctx = TenantContext::default();
        let memories = vec![
            create_test_entry("m1", Some(0.8)),
            create_test_entry("m2", Some(0.6)),
        ];

        let result = manager.compress(&ctx, &memories).await.unwrap();

        assert!(result.id.starts_with("compressed_"));
        assert!(result.metadata.contains_key("compressed_from"));
        assert!(result.metadata.contains_key("compression_ratio"));
        // Use approximate comparison for floating point
        let score = result.importance_score.unwrap();
        assert!((score - 0.7).abs() < 0.001, "Expected ~0.7, got {}", score);
    }

    #[tokio::test]
    async fn test_compression_inherits_trajectory_rewards() {
        let trajectories = Arc::new(RwLock::new(HashMap::new()));
        let llm = Arc::new(MockLlmService::new());
        let manager = CompressionManager::new(llm, trajectories.clone());

        {
            let mut t = trajectories.write().await;
            t.insert(
                "m1".to_string(),
                vec![create_trajectory_event(
                    "m1",
                    MemoryOperation::Retrieve,
                    Some(RewardSignal {
                        reward_type: RewardType::Helpful,
                        score: 0.9,
                        reasoning: Some("useful".to_string()),
                        agent_id: None,
                        timestamp: chrono::Utc::now().timestamp(),
                    }),
                )],
            );
        }

        let ctx = TenantContext::default();
        let memories = vec![create_test_entry("m1", Some(0.8))];

        let result = manager.compress(&ctx, &memories).await.unwrap();

        let t = trajectories.read().await;
        let inherited = t.get(&result.id);
        assert!(
            inherited.is_some(),
            "Compressed entry should inherit trajectory"
        );
        assert_eq!(inherited.unwrap().len(), 1);
        assert!(inherited.unwrap()[0].reward.is_some());
    }
}
