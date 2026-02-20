//! # Memory-R1 Trainer
//!
//! Implements outcome-driven policy updates for memory selection using
//! reinforcement learning concepts. The trainer learns which memories are
//! valuable based on trajectory data and reward signals.
//!
//! ## Core Concepts
//!
//! - **Trajectories**: Sequences of memory operations with optional rewards
//! - **Policy Weights**: Per-memory selection weights learned from rewards
//! - **Discounted Returns**: Future rewards discounted by γ (gamma)
//! - **Baseline**: Running average for variance reduction in policy updates
//!
//! ## Example
//!
//! ```ignore
//! use memory::trainer::{MemoryR1Trainer, R1TrainerConfig};
//!
//! let trainer = MemoryR1Trainer::new(
//!     R1TrainerConfig::default(),
//!     trajectories.clone(),
//! );
//!
//! // Run a training step
//! let metrics = trainer.train_step().await?;
//! println!("Average reward: {}", metrics.average_reward);
//! ```

use mk_core::types::MemoryTrajectoryEvent;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Configuration for Memory-R1 training.
///
/// These hyperparameters control the learning dynamics of the trainer.
#[derive(Debug, Clone)]
pub struct R1TrainerConfig {
    /// Learning rate for policy weight updates (α).
    /// Higher values mean faster learning but potentially less stable.
    /// Default: 0.01
    pub learning_rate: f32,

    /// Discount factor for future rewards (γ).
    /// Values closer to 1.0 weight future rewards more heavily.
    /// Default: 0.99
    pub discount_factor: f32,

    /// Minimum number of trajectories required for a training step.
    /// Prevents training on too little data.
    /// Default: 10
    pub min_batch_size: usize,

    /// Maximum trajectory length to consider per memory.
    /// Limits computational cost for long-running memories.
    /// Default: 100
    pub max_trajectory_length: usize,

    /// Decay rate for the running baseline (exponential moving average).
    /// Default: 0.9
    pub baseline_decay: f32,

    /// Minimum weight value to prevent weights from going too low.
    /// Default: 0.1
    pub min_weight: f32,

    /// Maximum weight value to prevent weights from dominating.
    /// Default: 10.0
    pub max_weight: f32,
}

impl Default for R1TrainerConfig {
    fn default() -> Self {
        Self {
            learning_rate: 0.01,
            discount_factor: 0.99,
            min_batch_size: 10,
            max_trajectory_length: 100,
            baseline_decay: 0.9,
            min_weight: 0.1,
            max_weight: 10.0,
        }
    }
}

/// Memory-R1 Trainer for outcome-driven policy updates.
///
/// This trainer implements a simplified policy gradient approach where:
/// - Each memory has an associated selection weight
/// - Weights are updated based on discounted returns from trajectory rewards
/// - A running baseline reduces variance in updates
///
/// The trainer does not directly modify memories; instead, it maintains
/// selection weights that can be used by the memory manager to prioritize
/// memories during search and retrieval.
pub struct MemoryR1Trainer {
    config: R1TrainerConfig,
    trajectories: Arc<RwLock<HashMap<String, Vec<MemoryTrajectoryEvent>>>>,
    /// Running baseline for variance reduction (exponential moving average of
    /// returns)
    baseline: f32,
    /// Memory selection weights (memory_id -> weight)
    /// Weights > 1.0 indicate high-value memories
    /// Weights < 1.0 indicate low-value memories
    selection_weights: HashMap<String, f32>,
    /// Cumulative returns per memory for aggregation
    cumulative_returns: HashMap<String, f32>,
    /// Number of updates per memory (for averaging)
    update_counts: HashMap<String, usize>,
}

impl MemoryR1Trainer {
    /// Creates a new Memory-R1 Trainer.
    ///
    /// # Arguments
    ///
    /// * `config` - Training configuration with hyperparameters
    /// * `trajectories` - Shared trajectory storage from MemoryManager
    pub fn new(
        config: R1TrainerConfig,
        trajectories: Arc<RwLock<HashMap<String, Vec<MemoryTrajectoryEvent>>>>,
    ) -> Self {
        Self {
            config,
            trajectories,
            baseline: 0.0,
            selection_weights: HashMap::new(),
            cumulative_returns: HashMap::new(),
            update_counts: HashMap::new(),
        }
    }

    /// Creates a trainer with pre-initialized weights.
    ///
    /// Useful for continuing training from a checkpoint.
    pub fn with_weights(mut self, weights: HashMap<String, f32>) -> Self {
        self.selection_weights = weights;
        self
    }

    /// Creates a trainer with a pre-set baseline.
    ///
    /// Useful for continuing training from a checkpoint.
    pub fn with_baseline(mut self, baseline: f32) -> Self {
        self.baseline = baseline;
        self
    }

    /// Computes discounted returns for a sequence of rewards.
    ///
    /// Uses the formula: R_t = r_t + γ * R_{t+1}
    ///
    /// # Arguments
    ///
    /// * `rewards` - Sequence of reward values in temporal order
    ///
    /// # Returns
    ///
    /// Vector of discounted returns, same length as input
    pub fn compute_returns(&self, rewards: &[f32]) -> Vec<f32> {
        if rewards.is_empty() {
            return Vec::new();
        }

        let mut returns = vec![0.0; rewards.len()];
        let gamma = self.config.discount_factor;

        // Compute returns backwards: R_t = r_t + γ * R_{t+1}
        returns[rewards.len() - 1] = rewards[rewards.len() - 1];
        for i in (0..rewards.len() - 1).rev() {
            returns[i] = rewards[i] + gamma * returns[i + 1];
        }

        returns
    }

    /// Aggregates rewards from a memory's trajectory events.
    ///
    /// Computes the total discounted return for a specific memory
    /// based on its trajectory history.
    ///
    /// # Arguments
    ///
    /// * `memory_id` - The memory ID to aggregate rewards for
    ///
    /// # Returns
    ///
    /// The total discounted return, or 0.0 if no rewards found
    pub async fn aggregate_trajectory_rewards(&self, memory_id: &str) -> f32 {
        let trajectories = self.trajectories.read().await;

        let events = match trajectories.get(memory_id) {
            Some(events) => events,
            None => return 0.0,
        };

        let events_to_process = if events.len() > self.config.max_trajectory_length {
            &events[events.len() - self.config.max_trajectory_length..]
        } else {
            &events[..]
        };

        let rewards: Vec<f32> = events_to_process
            .iter()
            .map(|e| e.reward.as_ref().map(|r| r.score).unwrap_or(0.0))
            .collect();

        if rewards.is_empty() {
            return 0.0;
        }

        let returns = self.compute_returns(&rewards);

        returns.first().copied().unwrap_or(0.0)
    }

    fn has_rewards(events: &[MemoryTrajectoryEvent]) -> bool {
        events.iter().any(|e| e.reward.is_some())
    }

    /// Updates policy weights based on trajectory outcomes.
    ///
    /// Implements a simplified policy gradient update:
    /// w = w + α * (R - baseline)
    ///
    /// Where:
    /// - w is the selection weight for a memory
    /// - α is the learning rate
    /// - R is the discounted return
    /// - baseline is the running average return (for variance reduction)
    ///
    /// # Returns
    ///
    /// Training metrics including number of memories processed
    pub async fn update_policy(&mut self) -> Result<TrainingMetrics, TrainerError> {
        let trajectories = self.trajectories.read().await;

        let memory_ids_with_rewards: Vec<String> = trajectories
            .iter()
            .filter(|(_, events)| Self::has_rewards(events))
            .map(|(id, _)| id.clone())
            .collect();

        if memory_ids_with_rewards.len() < self.config.min_batch_size {
            return Err(TrainerError::InsufficientData(
                memory_ids_with_rewards.len(),
                self.config.min_batch_size,
            ));
        }

        drop(trajectories);

        let mut total_return = 0.0;
        let mut memories_processed = 0;
        let mut policy_loss = 0.0;

        for memory_id in &memory_ids_with_rewards {
            let return_value = self.aggregate_trajectory_rewards(memory_id).await;

            let advantage = return_value - self.baseline;
            let current_weight = self
                .selection_weights
                .get(memory_id)
                .copied()
                .unwrap_or(1.0);
            let new_weight = (current_weight + self.config.learning_rate * advantage)
                .clamp(self.config.min_weight, self.config.max_weight);

            self.selection_weights.insert(memory_id.clone(), new_weight);

            total_return += return_value;
            memories_processed += 1;
            policy_loss += -advantage;

            *self
                .cumulative_returns
                .entry(memory_id.clone())
                .or_insert(0.0) += return_value;
            *self.update_counts.entry(memory_id.clone()).or_insert(0) += 1;
        }
        if memories_processed > 0 {
            let avg_return = total_return / memories_processed as f32;
            self.baseline = self.config.baseline_decay * self.baseline
                + (1.0 - self.config.baseline_decay) * avg_return;
        }

        Ok(TrainingMetrics {
            memories_processed,
            average_reward: if memories_processed > 0 {
                total_return / memories_processed as f32
            } else {
                0.0
            },
            policy_loss: if memories_processed > 0 {
                policy_loss / memories_processed as f32
            } else {
                0.0
            },
            baseline_value: self.baseline,
        })
    }

    /// Runs a complete training step on accumulated trajectories.
    ///
    /// This is the main entry point for training. It:
    /// 1. Checks if enough data is available
    /// 2. Computes returns for all trajectories
    /// 3. Updates policy weights
    /// 4. Returns training metrics
    ///
    /// # Returns
    ///
    /// Training metrics on success, or error if insufficient data
    pub async fn train_step(&mut self) -> Result<TrainingMetrics, TrainerError> {
        self.update_policy().await
    }

    /// Gets the selection weight for a specific memory.
    ///
    /// Weights > 1.0 indicate the memory has received positive rewards
    /// and should be prioritized in retrieval.
    ///
    /// # Arguments
    ///
    /// * `memory_id` - The memory to get weight for
    ///
    /// # Returns
    ///
    /// The selection weight (defaults to 1.0 for unknown memories)
    pub fn get_weight(&self, memory_id: &str) -> f32 {
        self.selection_weights
            .get(memory_id)
            .copied()
            .unwrap_or(1.0)
    }

    /// Gets all current selection weights.
    ///
    /// Useful for persistence or debugging.
    pub fn get_all_weights(&self) -> &HashMap<String, f32> {
        &self.selection_weights
    }

    /// Gets the current baseline value.
    pub fn get_baseline(&self) -> f32 {
        self.baseline
    }

    /// Gets the average return for a specific memory across all updates.
    pub fn get_average_return(&self, memory_id: &str) -> Option<f32> {
        let cumulative = self.cumulative_returns.get(memory_id)?;
        let count = self.update_counts.get(memory_id)?;
        if *count == 0 {
            return None;
        }
        Some(cumulative / *count as f32)
    }

    /// Resets all learned weights to default (1.0).
    ///
    /// Use with caution - this discards all learned information.
    pub fn reset_weights(&mut self) {
        self.selection_weights.clear();
        self.cumulative_returns.clear();
        self.update_counts.clear();
        self.baseline = 0.0;
    }

    /// Removes weights for memories that no longer exist.
    ///
    /// Call periodically to clean up weights for deleted memories.
    pub async fn prune_stale_weights(&mut self) {
        let trajectories = self.trajectories.read().await;
        let active_ids: std::collections::HashSet<&String> = trajectories.keys().collect();

        self.selection_weights
            .retain(|id, _| active_ids.contains(id));
        self.cumulative_returns
            .retain(|id, _| active_ids.contains(id));
        self.update_counts.retain(|id, _| active_ids.contains(id));
    }

    /// Exports the current trainer state for persistence.
    pub fn export_state(&self) -> TrainerState {
        TrainerState {
            baseline: self.baseline,
            selection_weights: self.selection_weights.clone(),
            cumulative_returns: self.cumulative_returns.clone(),
            update_counts: self.update_counts.clone(),
        }
    }

    /// Imports a previously exported trainer state.
    pub fn import_state(&mut self, state: TrainerState) {
        self.baseline = state.baseline;
        self.selection_weights = state.selection_weights;
        self.cumulative_returns = state.cumulative_returns;
        self.update_counts = state.update_counts;
    }
}

/// Metrics from a training step.
#[derive(Debug, Clone, Default)]
pub struct TrainingMetrics {
    /// Number of memories processed in this step
    pub memories_processed: usize,
    /// Average reward across all processed memories
    pub average_reward: f32,
    /// Average policy loss (negative advantage)
    pub policy_loss: f32,
    /// Current baseline value after update
    pub baseline_value: f32,
}

/// Serializable trainer state for persistence.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TrainerState {
    pub baseline: f32,
    pub selection_weights: HashMap<String, f32>,
    pub cumulative_returns: HashMap<String, f32>,
    pub update_counts: HashMap<String, usize>,
}

/// Errors that can occur during training.
#[derive(Debug, thiserror::Error)]
pub enum TrainerError {
    #[error("Insufficient data for training: {0} samples (need {1})")]
    InsufficientData(usize, usize),

    #[error("Training failed: {0}")]
    TrainingFailed(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use mk_core::types::{MemoryOperation, RewardSignal, RewardType};

    fn create_test_trajectories() -> Arc<RwLock<HashMap<String, Vec<MemoryTrajectoryEvent>>>> {
        Arc::new(RwLock::new(HashMap::new()))
    }

    fn create_reward(score: f32) -> RewardSignal {
        RewardSignal {
            reward_type: if score >= 0.0 {
                RewardType::Helpful
            } else {
                RewardType::Irrelevant
            },
            score,
            reasoning: Some(format!("Test reward with score {}", score)),
            agent_id: None,
            timestamp: chrono::Utc::now().timestamp(),
        }
    }

    fn create_event(
        entry_id: &str,
        operation: MemoryOperation,
        reward: Option<RewardSignal>,
    ) -> MemoryTrajectoryEvent {
        MemoryTrajectoryEvent {
            operation,
            entry_id: entry_id.to_string(),
            reward,
            reasoning: Some("Test event".to_string()),
            timestamp: chrono::Utc::now().timestamp(),
        }
    }

    #[test]
    fn test_config_default() {
        let config = R1TrainerConfig::default();
        assert_eq!(config.learning_rate, 0.01);
        assert_eq!(config.discount_factor, 0.99);
        assert_eq!(config.min_batch_size, 10);
        assert_eq!(config.max_trajectory_length, 100);
        assert_eq!(config.baseline_decay, 0.9);
        assert_eq!(config.min_weight, 0.1);
        assert_eq!(config.max_weight, 10.0);
    }

    #[test]
    fn test_compute_returns_empty() {
        let trajectories = create_test_trajectories();
        let trainer = MemoryR1Trainer::new(R1TrainerConfig::default(), trajectories);

        let returns = trainer.compute_returns(&[]);
        assert!(returns.is_empty());
    }

    #[test]
    fn test_compute_returns_single() {
        let trajectories = create_test_trajectories();
        let trainer = MemoryR1Trainer::new(R1TrainerConfig::default(), trajectories);

        let returns = trainer.compute_returns(&[1.0]);
        assert_eq!(returns.len(), 1);
        assert_eq!(returns[0], 1.0);
    }

    #[test]
    fn test_compute_returns_sequence() {
        let trajectories = create_test_trajectories();
        let config = R1TrainerConfig {
            discount_factor: 0.9,
            ..Default::default()
        };
        let trainer = MemoryR1Trainer::new(config, trajectories);

        // Rewards: [1.0, 1.0, 1.0]
        // Returns: [1 + 0.9*(1 + 0.9*1), 1 + 0.9*1, 1]
        //        = [1 + 0.9*1.9, 1.9, 1]
        //        = [2.71, 1.9, 1.0]
        let returns = trainer.compute_returns(&[1.0, 1.0, 1.0]);
        assert_eq!(returns.len(), 3);
        assert!((returns[2] - 1.0).abs() < 0.001);
        assert!((returns[1] - 1.9).abs() < 0.001);
        assert!((returns[0] - 2.71).abs() < 0.001);
    }

    #[test]
    fn test_compute_returns_with_zeros() {
        let trajectories = create_test_trajectories();
        let config = R1TrainerConfig {
            discount_factor: 0.5,
            ..Default::default()
        };
        let trainer = MemoryR1Trainer::new(config, trajectories);

        // Rewards: [0.0, 0.0, 1.0]
        // Returns: [0 + 0.5*(0 + 0.5*1), 0 + 0.5*1, 1]
        //        = [0.25, 0.5, 1.0]
        let returns = trainer.compute_returns(&[0.0, 0.0, 1.0]);
        assert_eq!(returns.len(), 3);
        assert!((returns[2] - 1.0).abs() < 0.001);
        assert!((returns[1] - 0.5).abs() < 0.001);
        assert!((returns[0] - 0.25).abs() < 0.001);
    }

    #[test]
    fn test_compute_returns_negative() {
        let trajectories = create_test_trajectories();
        let config = R1TrainerConfig {
            discount_factor: 0.5,
            ..Default::default()
        };
        let trainer = MemoryR1Trainer::new(config, trajectories);

        // Rewards: [1.0, -1.0]
        // Returns: [1 + 0.5*(-1), -1]
        //        = [0.5, -1.0]
        let returns = trainer.compute_returns(&[1.0, -1.0]);
        assert_eq!(returns.len(), 2);
        assert!((returns[1] - (-1.0)).abs() < 0.001);
        assert!((returns[0] - 0.5).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_aggregate_trajectory_rewards_no_trajectory() {
        let trajectories = create_test_trajectories();
        let trainer = MemoryR1Trainer::new(R1TrainerConfig::default(), trajectories);

        let reward = trainer.aggregate_trajectory_rewards("nonexistent").await;
        assert_eq!(reward, 0.0);
    }

    #[tokio::test]
    async fn test_aggregate_trajectory_rewards_no_rewards() {
        let trajectories = create_test_trajectories();

        // Add events without rewards
        {
            let mut traj = trajectories.write().await;
            traj.insert(
                "mem1".to_string(),
                vec![
                    create_event("mem1", MemoryOperation::Add, None),
                    create_event("mem1", MemoryOperation::Retrieve, None),
                ],
            );
        }

        let trainer = MemoryR1Trainer::new(R1TrainerConfig::default(), trajectories);
        let reward = trainer.aggregate_trajectory_rewards("mem1").await;
        assert_eq!(reward, 0.0);
    }

    #[tokio::test]
    async fn test_aggregate_trajectory_rewards_with_rewards() {
        let trajectories = create_test_trajectories();

        {
            let mut traj = trajectories.write().await;
            traj.insert(
                "mem1".to_string(),
                vec![
                    create_event("mem1", MemoryOperation::Add, None),
                    create_event("mem1", MemoryOperation::Retrieve, Some(create_reward(0.5))),
                    create_event("mem1", MemoryOperation::Retrieve, Some(create_reward(0.3))),
                ],
            );
        }

        let config = R1TrainerConfig {
            discount_factor: 0.9,
            ..Default::default()
        };
        let trainer = MemoryR1Trainer::new(config, trajectories);
        let reward = trainer.aggregate_trajectory_rewards("mem1").await;

        // Rewards sequence: [0.0, 0.5, 0.3]
        // Returns: [0 + 0.9*(0.5 + 0.9*0.3), 0.5 + 0.9*0.3, 0.3]
        //        = [0 + 0.9*0.77, 0.77, 0.3]
        //        = [0.693, 0.77, 0.3]
        assert!((reward - 0.693).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_aggregate_trajectory_rewards_max_length() {
        let trajectories = create_test_trajectories();

        {
            let mut traj = trajectories.write().await;
            let mut events = Vec::new();
            for _ in 0..150 {
                events.push(create_event(
                    "mem1",
                    MemoryOperation::Retrieve,
                    Some(create_reward(0.1)),
                ));
            }
            traj.insert("mem1".to_string(), events);
        }

        let config = R1TrainerConfig {
            max_trajectory_length: 10,
            discount_factor: 0.9,
            ..Default::default()
        };
        let trainer = MemoryR1Trainer::new(config, trajectories);

        // Should only process last 10 events
        let reward = trainer.aggregate_trajectory_rewards("mem1").await;
        // 10 rewards of 0.1 with γ=0.9
        // First return ≈ sum of discounted 0.1s
        assert!(reward > 0.0 && reward < 1.0);
    }

    #[tokio::test]
    async fn test_update_policy_insufficient_data() {
        let trajectories = create_test_trajectories();

        // Only add 5 trajectories (less than min_batch_size of 10)
        {
            let mut traj = trajectories.write().await;
            for i in 0..5 {
                traj.insert(
                    format!("mem{}", i),
                    vec![create_event(
                        &format!("mem{}", i),
                        MemoryOperation::Retrieve,
                        Some(create_reward(0.5)),
                    )],
                );
            }
        }

        let mut trainer = MemoryR1Trainer::new(R1TrainerConfig::default(), trajectories);
        let result = trainer.update_policy().await;

        assert!(result.is_err());
        match result {
            Err(TrainerError::InsufficientData(got, need)) => {
                assert_eq!(got, 5);
                assert_eq!(need, 10);
            }
            _ => panic!("Expected InsufficientData error"),
        }
    }

    #[tokio::test]
    async fn test_update_policy_success() {
        let trajectories = create_test_trajectories();

        // Add 15 trajectories with rewards
        {
            let mut traj = trajectories.write().await;
            for i in 0..15 {
                let score = if i % 2 == 0 { 0.5 } else { -0.3 };
                traj.insert(
                    format!("mem{}", i),
                    vec![create_event(
                        &format!("mem{}", i),
                        MemoryOperation::Retrieve,
                        Some(create_reward(score)),
                    )],
                );
            }
        }

        let mut trainer = MemoryR1Trainer::new(R1TrainerConfig::default(), trajectories);
        let metrics = trainer.update_policy().await.unwrap();

        assert_eq!(metrics.memories_processed, 15);
        assert!(metrics.average_reward != 0.0);
        assert!(metrics.baseline_value != 0.0);

        // Check that weights were updated
        assert_eq!(trainer.selection_weights.len(), 15);
    }

    #[tokio::test]
    async fn test_train_step() {
        let trajectories = create_test_trajectories();

        {
            let mut traj = trajectories.write().await;
            for i in 0..12 {
                traj.insert(
                    format!("mem{}", i),
                    vec![create_event(
                        &format!("mem{}", i),
                        MemoryOperation::Retrieve,
                        Some(create_reward(0.5)),
                    )],
                );
            }
        }

        let mut trainer = MemoryR1Trainer::new(R1TrainerConfig::default(), trajectories);
        let metrics = trainer.train_step().await.unwrap();

        assert_eq!(metrics.memories_processed, 12);
    }

    #[test]
    fn test_get_weight_default() {
        let trajectories = create_test_trajectories();
        let trainer = MemoryR1Trainer::new(R1TrainerConfig::default(), trajectories);

        assert_eq!(trainer.get_weight("unknown"), 1.0);
    }

    #[tokio::test]
    async fn test_get_weight_after_training() {
        let trajectories = create_test_trajectories();

        {
            let mut traj = trajectories.write().await;
            for i in 0..12 {
                let score = if i == 0 { 0.9 } else { 0.1 };
                traj.insert(
                    format!("mem{}", i),
                    vec![create_event(
                        &format!("mem{}", i),
                        MemoryOperation::Retrieve,
                        Some(create_reward(score)),
                    )],
                );
            }
        }

        let mut trainer = MemoryR1Trainer::new(R1TrainerConfig::default(), trajectories);
        trainer.train_step().await.unwrap();

        // mem0 had highest reward, should have higher weight
        let weight0 = trainer.get_weight("mem0");
        let weight1 = trainer.get_weight("mem1");

        assert!(weight0 > weight1);
    }

    #[test]
    fn test_with_weights() {
        let trajectories = create_test_trajectories();
        let weights = HashMap::from([("mem1".to_string(), 2.0), ("mem2".to_string(), 0.5)]);

        let trainer =
            MemoryR1Trainer::new(R1TrainerConfig::default(), trajectories).with_weights(weights);

        assert_eq!(trainer.get_weight("mem1"), 2.0);
        assert_eq!(trainer.get_weight("mem2"), 0.5);
    }

    #[test]
    fn test_with_baseline() {
        let trajectories = create_test_trajectories();
        let trainer =
            MemoryR1Trainer::new(R1TrainerConfig::default(), trajectories).with_baseline(0.5);

        assert_eq!(trainer.get_baseline(), 0.5);
    }

    #[test]
    fn test_reset_weights() {
        let trajectories = create_test_trajectories();
        let weights = HashMap::from([("mem1".to_string(), 2.0)]);

        let mut trainer = MemoryR1Trainer::new(R1TrainerConfig::default(), trajectories)
            .with_weights(weights)
            .with_baseline(0.5);

        trainer.reset_weights();

        assert_eq!(trainer.get_weight("mem1"), 1.0);
        assert_eq!(trainer.get_baseline(), 0.0);
        assert!(trainer.selection_weights.is_empty());
    }

    #[tokio::test]
    async fn test_prune_stale_weights() {
        let trajectories = create_test_trajectories();

        {
            let mut traj = trajectories.write().await;
            traj.insert(
                "active".to_string(),
                vec![create_event("active", MemoryOperation::Add, None)],
            );
        }

        let weights = HashMap::from([("active".to_string(), 2.0), ("deleted".to_string(), 1.5)]);

        let mut trainer =
            MemoryR1Trainer::new(R1TrainerConfig::default(), trajectories).with_weights(weights);

        trainer.prune_stale_weights().await;

        assert_eq!(trainer.get_weight("active"), 2.0);
        assert_eq!(trainer.get_weight("deleted"), 1.0); // default, not stored
        assert_eq!(trainer.selection_weights.len(), 1);
    }

    #[tokio::test]
    async fn test_export_import_state() {
        let trajectories = create_test_trajectories();

        {
            let mut traj = trajectories.write().await;
            for i in 0..12 {
                traj.insert(
                    format!("mem{}", i),
                    vec![create_event(
                        &format!("mem{}", i),
                        MemoryOperation::Retrieve,
                        Some(create_reward(0.5)),
                    )],
                );
            }
        }

        let mut trainer = MemoryR1Trainer::new(R1TrainerConfig::default(), trajectories.clone());
        trainer.train_step().await.unwrap();

        let state = trainer.export_state();
        assert!(!state.selection_weights.is_empty());
        assert!(state.baseline != 0.0);

        // Create new trainer and import state
        let mut new_trainer = MemoryR1Trainer::new(R1TrainerConfig::default(), trajectories);
        new_trainer.import_state(state.clone());

        assert_eq!(new_trainer.get_baseline(), state.baseline);
        assert_eq!(
            new_trainer.selection_weights.len(),
            state.selection_weights.len()
        );
    }

    #[tokio::test]
    async fn test_weight_clamping() {
        let trajectories = create_test_trajectories();

        // Create extreme positive rewards to test max clamping
        {
            let mut traj = trajectories.write().await;
            for i in 0..12 {
                let mut events = Vec::new();
                for _ in 0..50 {
                    events.push(create_event(
                        &format!("mem{}", i),
                        MemoryOperation::Retrieve,
                        Some(create_reward(1.0)),
                    ));
                }
                traj.insert(format!("mem{}", i), events);
            }
        }

        let config = R1TrainerConfig {
            learning_rate: 1.0, // High learning rate for faster updates
            max_weight: 5.0,
            ..Default::default()
        };

        let mut trainer = MemoryR1Trainer::new(config, trajectories);

        // Train multiple times to accumulate weight
        for _ in 0..10 {
            let _ = trainer.train_step().await;
        }

        // All weights should be clamped to max
        for (_, weight) in trainer.get_all_weights() {
            assert!(*weight <= 5.0);
        }
    }

    #[tokio::test]
    async fn test_get_average_return() {
        let trajectories = create_test_trajectories();

        {
            let mut traj = trajectories.write().await;
            for i in 0..12 {
                traj.insert(
                    format!("mem{}", i),
                    vec![create_event(
                        &format!("mem{}", i),
                        MemoryOperation::Retrieve,
                        Some(create_reward(0.5)),
                    )],
                );
            }
        }

        let mut trainer = MemoryR1Trainer::new(R1TrainerConfig::default(), trajectories);
        trainer.train_step().await.unwrap();

        // All memories should have average return tracked
        for i in 0..12 {
            let avg = trainer.get_average_return(&format!("mem{}", i));
            assert!(avg.is_some());
            assert!(avg.unwrap() > 0.0);
        }

        // Unknown memory should return None
        assert!(trainer.get_average_return("unknown").is_none());
    }

    #[test]
    fn test_training_metrics_default() {
        let metrics = TrainingMetrics::default();
        assert_eq!(metrics.memories_processed, 0);
        assert_eq!(metrics.average_reward, 0.0);
        assert_eq!(metrics.policy_loss, 0.0);
        assert_eq!(metrics.baseline_value, 0.0);
    }

    #[test]
    fn test_trainer_state_serialization() {
        let state = TrainerState {
            baseline: 0.5,
            selection_weights: HashMap::from([("mem1".to_string(), 1.5)]),
            cumulative_returns: HashMap::from([("mem1".to_string(), 2.0)]),
            update_counts: HashMap::from([("mem1".to_string(), 2)]),
        };

        let json = serde_json::to_string(&state).unwrap();
        let deserialized: TrainerState = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.baseline, state.baseline);
        assert_eq!(deserialized.selection_weights, state.selection_weights);
    }

    #[test]
    fn test_trainer_error_display() {
        let err = TrainerError::InsufficientData(5, 10);
        assert!(err.to_string().contains("5 samples"));
        assert!(err.to_string().contains("need 10"));

        let err2 = TrainerError::TrainingFailed("test error".to_string());
        assert!(err2.to_string().contains("test error"));
    }

    #[tokio::test]
    async fn test_baseline_update_with_exponential_moving_average() {
        let trajectories = create_test_trajectories();

        {
            let mut traj = trajectories.write().await;
            for i in 0..12 {
                traj.insert(
                    format!("mem{}", i),
                    vec![create_event(
                        &format!("mem{}", i),
                        MemoryOperation::Retrieve,
                        Some(create_reward(1.0)),
                    )],
                );
            }
        }

        let config = R1TrainerConfig {
            baseline_decay: 0.5, // Faster updates for testing
            ..Default::default()
        };

        let mut trainer = MemoryR1Trainer::new(config, trajectories);

        // Initial baseline is 0
        assert_eq!(trainer.get_baseline(), 0.0);

        // After first training, baseline should be updated
        trainer.train_step().await.unwrap();
        let baseline1 = trainer.get_baseline();
        assert!(baseline1 > 0.0);

        // Baseline should use EMA formula
        // new_baseline = 0.5 * old_baseline + 0.5 * avg_return
    }

    #[tokio::test]
    async fn test_has_rewards_helper() {
        let events_with_rewards = vec![
            create_event("mem1", MemoryOperation::Add, None),
            create_event("mem1", MemoryOperation::Retrieve, Some(create_reward(0.5))),
        ];

        let events_without_rewards = vec![
            create_event("mem1", MemoryOperation::Add, None),
            create_event("mem1", MemoryOperation::Retrieve, None),
        ];

        assert!(MemoryR1Trainer::has_rewards(&events_with_rewards));
        assert!(!MemoryR1Trainer::has_rewards(&events_without_rewards));
    }
}
