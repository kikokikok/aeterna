//! Combined Memory Trainer for R1 + RLM training.
//!
//! This module provides a unified training interface for both:
//! - Memory-R1 Trainer: Learns which memories are valuable
//! - RLM Decomposition Trainer: Learns optimal decomposition strategies
//!
//! The combined trainer manages both training pipelines and provides a unified
//! `train_step()` method for batch training operations.

use crate::rlm::trainer::{RewardConfig, TrainingOutcome};
use crate::trainer::{MemoryR1Trainer, R1TrainerConfig, TrainerError, TrainingMetrics};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Unified memory training metrics
#[derive(Debug, Clone, Default)]
pub struct CombinedTrainingMetrics {
    /// Memory-R1 training metrics
    pub r1_metrics: TrainingMetrics,
    /// Decomposition training metrics
    pub decomposition_metrics: DecompositionTrainingMetrics,
    /// Total trajectories processed
    pub total_trajectories: usize,
}

/// Decomposition training metrics
#[derive(Debug, Clone, Default)]
pub struct DecompositionTrainingMetrics {
    /// Number of decomposition trajectories trained
    pub trajectories_trained: usize,
    /// Average reward for decomposition
    pub average_reward: f32,
    /// Current exploration rate (epsilon)
    pub exploration_rate: f32,
}

/// Combined Memory Trainer for unified R1 + RLM training.
///
/// Manages both Memory-R1 trainer (for memory selection weights)
/// and Decomposition trainer (for RLM action selection strategies).
pub struct CombinedMemoryTrainer {
    /// Memory-R1 trainer for memory selection weights
    r1_trainer: MemoryR1Trainer,
    /// RLM Decomposition trainer for action strategies
    decomposition_trainer: crate::rlm::trainer::DecompositionTrainer,
    /// Storage for decomposition trajectories
    pub(crate) decomposition_trajectories:
        Arc<RwLock<Vec<crate::rlm::trainer::DecompositionTrajectory>>>,
}

impl CombinedMemoryTrainer {
    /// Creates a new combined trainer.
    ///
    /// # Arguments
    ///
    /// * `r1_config` - Configuration for Memory-R1 training
    /// * `decomposition_config` - Reward config for decomposition training
    /// * `r1_trajectories` - Shared trajectory storage for Memory-R1
    pub fn new(
        r1_config: R1TrainerConfig,
        decomposition_config: RewardConfig,
        r1_trajectories: Arc<
            RwLock<std::collections::HashMap<String, Vec<mk_core::types::MemoryTrajectoryEvent>>>,
        >,
    ) -> Self {
        Self {
            r1_trainer: MemoryR1Trainer::new(r1_config, r1_trajectories),
            decomposition_trainer: crate::rlm::trainer::DecompositionTrainer::new(
                decomposition_config,
            ),
            decomposition_trajectories: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Runs a unified training step for both trainers.
    ///
    /// This is the main entry point for training. It:
    /// 1. Trains Memory-R1 on accumulated trajectories
    /// 2. Trains RLM Decomposition on accumulated trajectories
    /// 3. Returns combined metrics
    ///
    /// # Returns
    ///
    /// Combined training metrics, or error if training failed
    pub async fn train_step(&mut self) -> Result<CombinedTrainingMetrics, TrainerError> {
        let r1_result = self.r1_trainer.train_step().await?;

        let decomposition_result = self.train_decomposition().await?;

        Ok(CombinedTrainingMetrics {
            r1_metrics: r1_result.clone(),
            decomposition_metrics: decomposition_result.clone(),
            total_trajectories: r1_result.memories_processed
                + decomposition_result.trajectories_trained,
        })
    }

    /// Trains decomposition trainer on accumulated trajectories.
    pub async fn train_decomposition(
        &mut self,
    ) -> Result<DecompositionTrainingMetrics, TrainerError> {
        let complete_trajectories: Vec<_> = {
            let trajectories = self.decomposition_trajectories.read().await;
            if trajectories.is_empty() {
                return Ok(DecompositionTrainingMetrics {
                    trajectories_trained: 0,
                    average_reward: 0.0,
                    exploration_rate: self.decomposition_trainer.epsilon(),
                });
            }

            trajectories
                .iter()
                .filter(|t| t.is_complete())
                .cloned()
                .collect()
        };

        if complete_trajectories.is_empty() {
            return Ok(DecompositionTrainingMetrics {
                trajectories_trained: 0,
                average_reward: 0.0,
                exploration_rate: self.decomposition_trainer.epsilon(),
            });
        }

        let mut total_reward = 0.0;
        let mut trained_count = 0;

        for trajectory in complete_trajectories.iter() {
            let _ = self.decomposition_trainer.train(trajectory).await;
            total_reward += trajectory.reward.unwrap_or(0.0);
            trained_count += 1;
        }

        let average_reward = if trained_count > 0 {
            total_reward / trained_count as f32
        } else {
            0.0
        };

        let exploration_rate = self.decomposition_trainer.epsilon();

        Ok(DecompositionTrainingMetrics {
            trajectories_trained: trained_count,
            average_reward,
            exploration_rate,
        })
    }

    /// Adds a decomposition trajectory for training.
    ///
    /// # Arguments
    ///
    /// * `trajectory` - The decomposition trajectory to add
    pub async fn add_decomposition_trajectory(
        &self,
        trajectory: crate::rlm::trainer::DecompositionTrajectory,
    ) {
        let mut trajectories = self.decomposition_trajectories.write().await;
        trajectories.push(trajectory);
    }

    /// Records outcome for a decomposition trajectory.
    ///
    /// # Arguments
    ///
    /// * `trajectory_id` - ID of the trajectory to update
    /// * `outcome` - The training outcome
    pub async fn record_decomposition_outcome(
        &self,
        trajectory_id: &str,
        outcome: TrainingOutcome,
    ) -> Result<(), TrainerError> {
        let mut trajectories = self.decomposition_trajectories.write().await;

        if let Some(trajectory) = trajectories.iter_mut().find(|t| t.id == trajectory_id) {
            let reward_config = RewardConfig::default();
            trajectory.record_outcome(outcome, &reward_config);
            Ok(())
        } else {
            Err(TrainerError::TrainingFailed(format!(
                "Trajectory {} not found",
                trajectory_id
            )))
        }
    }

    /// Gets Memory-R1 trainer for direct access.
    pub fn r1_trainer(&self) -> &MemoryR1Trainer {
        &self.r1_trainer
    }

    /// Gets mutable reference to Memory-R1 trainer.
    pub fn r1_trainer_mut(&mut self) -> &mut MemoryR1Trainer {
        &mut self.r1_trainer
    }

    /// Gets Decomposition trainer for direct access.
    pub fn decomposition_trainer(&self) -> &crate::rlm::trainer::DecompositionTrainer {
        &self.decomposition_trainer
    }

    /// Gets mutable reference to Decomposition trainer.
    pub fn decomposition_trainer_mut(&mut self) -> &mut crate::rlm::trainer::DecompositionTrainer {
        &mut self.decomposition_trainer
    }

    /// Gets the count of decomposition trajectories.
    pub async fn decomposition_trajectory_count(&self) -> usize {
        let trajectories = self.decomposition_trajectories.read().await;
        trajectories.len()
    }

    /// Exports combined trainer state for persistence.
    pub fn export_state(&self) -> CombinedTrainerState {
        CombinedTrainerState {
            r1_state: self.r1_trainer.export_state(),
            decomposition_state: self
                .decomposition_trainer
                .export_state()
                .unwrap_or_default(),
        }
    }

    /// Imports a previously exported trainer state.
    pub fn import_state(&mut self, state: CombinedTrainerState) -> Result<(), TrainerError> {
        self.r1_trainer.import_state(state.r1_state);
        self.decomposition_trainer
            .import_state(state.decomposition_state)
            .map_err(|e| TrainerError::TrainingFailed(e.to_string()))?;
        Ok(())
    }

    /// Clears all accumulated decomposition trajectories.
    pub async fn clear_decomposition_trajectories(&self) {
        let mut trajectories = self.decomposition_trajectories.write().await;
        trajectories.clear();
    }

    pub async fn save_policy_state<S: storage::RlmWeightStorage>(
        &self,
        storage: &S,
        tenant_id: &str,
    ) -> Result<(), TrainerError> {
        let policy_state = self
            .decomposition_trainer
            .export_state()
            .map_err(|e| TrainerError::TrainingFailed(e.to_string()))?;

        let stored = storage::StoredPolicyState {
            tenant_id: tenant_id.to_string(),
            action_weights: policy_state.action_weights,
            epsilon: policy_state.epsilon,
            step_count: policy_state.step_count,
            updated_at: chrono::Utc::now().timestamp(),
        };

        storage
            .save_policy_state(&stored)
            .await
            .map_err(|e| TrainerError::TrainingFailed(e.to_string()))?;

        tracing::info!(
            "Saved RLM policy state for tenant {} with {} weights",
            tenant_id,
            stored.action_weights.len()
        );

        Ok(())
    }

    pub async fn load_policy_state<S: storage::RlmWeightStorage>(
        &mut self,
        storage: &S,
        tenant_id: &str,
    ) -> Result<bool, TrainerError> {
        let stored = storage
            .load_policy_state(tenant_id)
            .await
            .map_err(|e| TrainerError::TrainingFailed(e.to_string()))?;

        if let Some(stored) = stored {
            let policy_state = crate::rlm::trainer::PolicyState {
                action_weights: stored.action_weights,
                epsilon: stored.epsilon,
                step_count: stored.step_count,
            };

            self.decomposition_trainer
                .import_state(policy_state)
                .map_err(|e| TrainerError::TrainingFailed(e.to_string()))?;

            tracing::info!(
                "Loaded RLM policy state for tenant {} (step_count: {}, epsilon: {:.4})",
                tenant_id,
                stored.step_count,
                stored.epsilon
            );

            Ok(true)
        } else {
            tracing::debug!(
                "No existing RLM policy state found for tenant {}",
                tenant_id
            );
            Ok(false)
        }
    }
}

/// Serializable combined trainer state for persistence.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CombinedTrainerState {
    pub r1_state: crate::trainer::TrainerState,
    pub decomposition_state: crate::rlm::trainer::PolicyState,
}

#[cfg(test)]
mod tests {
    use super::*;
    use mk_core::types::{SearchQuery, TenantContext};

    fn create_test_combined_trainer() -> CombinedMemoryTrainer {
        let r1_config = R1TrainerConfig {
            min_batch_size: 1,
            ..Default::default()
        };
        let r1_trajectories: Arc<
            RwLock<std::collections::HashMap<String, Vec<mk_core::types::MemoryTrajectoryEvent>>>,
        > = Arc::new(RwLock::new(std::collections::HashMap::new()));
        let decomposition_config = RewardConfig::default();

        CombinedMemoryTrainer::new(r1_config, decomposition_config, r1_trajectories)
    }

    #[test]
    fn test_combined_trainer_creation() {
        let trainer = create_test_combined_trainer();
        assert_eq!(trainer.decomposition_trainer().epsilon(), 0.1);
    }

    #[tokio::test]
    async fn test_add_decomposition_trajectory() {
        let trainer = create_test_combined_trainer();
        let trajectory = crate::rlm::trainer::DecompositionTrajectory::new(
            SearchQuery {
                text: "test".to_string(),
                ..Default::default()
            },
            TenantContext::default(),
        );

        trainer
            .add_decomposition_trajectory(trajectory.clone())
            .await;

        let trajectories = trainer.decomposition_trajectories.read().await;
        assert_eq!(trajectories.len(), 1);
        assert_eq!(trajectories[0].query, "test");
    }

    #[tokio::test]
    async fn test_record_decomposition_outcome() {
        let trainer = create_test_combined_trainer();
        let trajectory = crate::rlm::trainer::DecompositionTrajectory::new(
            SearchQuery {
                text: "test".to_string(),
                ..Default::default()
            },
            TenantContext::default(),
        );

        trainer
            .add_decomposition_trajectory(trajectory.clone())
            .await;

        let result = trainer
            .record_decomposition_outcome(
                &trajectory.id,
                TrainingOutcome::ResultUsed { quality_score: 0.8 },
            )
            .await;

        assert!(result.is_ok());

        let trajectories = trainer.decomposition_trajectories.read().await;
        assert!(trajectories[0].reward.is_some());
    }

    #[tokio::test]
    async fn test_train_decomposition_empty() {
        let metrics = create_test_combined_trainer()
            .train_decomposition()
            .await
            .unwrap();
        assert_eq!(metrics.trajectories_trained, 0);
        assert_eq!(metrics.average_reward, 0.0);
    }

    #[test]
    fn test_export_import_state() {
        let trainer = create_test_combined_trainer();

        let state = trainer.export_state();
        assert_eq!(state.decomposition_state.epsilon, 0.1);

        let mut trainer2 = create_test_combined_trainer();
        let result = trainer2.import_state(state);
        assert!(result.is_ok());
        assert_eq!(trainer2.decomposition_trainer().epsilon(), 0.1);
    }

    #[tokio::test]
    async fn test_clear_decomposition_trajectories() {
        let trainer = create_test_combined_trainer();

        let trajectory = crate::rlm::trainer::DecompositionTrajectory::new(
            SearchQuery {
                text: "test".to_string(),
                ..Default::default()
            },
            TenantContext::default(),
        );

        trainer.add_decomposition_trajectory(trajectory).await;
        assert_eq!(trainer.decomposition_trajectories.read().await.len(), 1);

        trainer.clear_decomposition_trajectories().await;
        assert_eq!(trainer.decomposition_trajectories.read().await.len(), 0);
    }

    #[tokio::test]
    async fn test_train_decomposition_with_data() {
        let mut trainer = create_test_combined_trainer();

        let trajectory1 = crate::rlm::trainer::DecompositionTrajectory::new(
            SearchQuery {
                text: "test 1".to_string(),
                ..Default::default()
            },
            TenantContext::default(),
        );
        let trajectory2 = crate::rlm::trainer::DecompositionTrajectory::new(
            SearchQuery {
                text: "test 2".to_string(),
                ..Default::default()
            },
            TenantContext::default(),
        );

        trainer.add_decomposition_trajectory(trajectory1).await;
        trainer.add_decomposition_trajectory(trajectory2).await;

        let config = RewardConfig::default();
        {
            let mut trajectories = trainer.decomposition_trajectories.write().await;
            for t in trajectories.iter_mut() {
                t.record_outcome(TrainingOutcome::ResultUsed { quality_score: 0.8 }, &config);
            }
        }

        let metrics = trainer.train_decomposition().await.unwrap();
        assert_eq!(metrics.trajectories_trained, 2);
        assert!((metrics.average_reward - 1.0).abs() < 0.01);
    }

    mod persistence_tests {
        use super::*;
        use std::collections::HashMap;
        use std::sync::Mutex;

        struct MockRlmWeightStorage {
            states: Mutex<HashMap<String, storage::StoredPolicyState>>,
        }

        impl MockRlmWeightStorage {
            fn new() -> Self {
                Self {
                    states: Mutex::new(HashMap::new()),
                }
            }
        }

        #[async_trait::async_trait]
        impl storage::RlmWeightStorage for MockRlmWeightStorage {
            type Error = std::io::Error;

            async fn save_policy_state(
                &self,
                state: &storage::StoredPolicyState,
            ) -> Result<(), Self::Error> {
                let mut states = self.states.lock().unwrap();
                states.insert(state.tenant_id.clone(), state.clone());
                Ok(())
            }

            async fn load_policy_state(
                &self,
                tenant_id: &str,
            ) -> Result<Option<storage::StoredPolicyState>, Self::Error> {
                let states = self.states.lock().unwrap();
                Ok(states.get(tenant_id).cloned())
            }

            async fn delete_policy_state(&self, tenant_id: &str) -> Result<(), Self::Error> {
                let mut states = self.states.lock().unwrap();
                states.remove(tenant_id);
                Ok(())
            }

            async fn list_policy_states(
                &self,
                limit: usize,
            ) -> Result<Vec<storage::StoredPolicyState>, Self::Error> {
                let states = self.states.lock().unwrap();
                Ok(states.values().take(limit).cloned().collect())
            }
        }

        #[tokio::test]
        async fn test_save_and_load_policy_state() {
            let mut trainer = create_test_combined_trainer();
            let storage = MockRlmWeightStorage::new();

            trainer
                .decomposition_trainer_mut()
                .import_state(crate::rlm::trainer::PolicyState {
                    action_weights: {
                        let mut w = HashMap::new();
                        w.insert("SearchLayer".to_string(), 0.75);
                        w
                    },
                    epsilon: 0.05,
                    step_count: 500,
                })
                .unwrap();

            trainer
                .save_policy_state(&storage, "test-tenant")
                .await
                .unwrap();

            let mut trainer2 = create_test_combined_trainer();
            let loaded = trainer2
                .load_policy_state(&storage, "test-tenant")
                .await
                .unwrap();

            assert!(loaded);
            assert_eq!(trainer2.decomposition_trainer().epsilon(), 0.05);
            assert_eq!(
                trainer2
                    .decomposition_trainer()
                    .action_weight("SearchLayer"),
                0.75
            );
        }

        #[tokio::test]
        async fn test_load_nonexistent_state_returns_false() {
            let mut trainer = create_test_combined_trainer();
            let storage = MockRlmWeightStorage::new();

            let loaded = trainer
                .load_policy_state(&storage, "nonexistent")
                .await
                .unwrap();

            assert!(!loaded);
            assert_eq!(trainer.decomposition_trainer().epsilon(), 0.1);
        }
    }
}
