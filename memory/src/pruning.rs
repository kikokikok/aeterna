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
