//! RLM Decomposition Trainer.
//!
//! This module implements reinforcement learning for RLM decomposition
//! strategies, learning optimal action sequences from search outcomes.

use crate::rlm::executor::RlmTrajectory;
use chrono::{DateTime, Utc};
use mk_core::types::{SearchQuery, TenantContext};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Training outcome for a decomposition trajectory.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TrainingOutcome {
    /// The search result was used in context and led to task success.
    ResultUsed { quality_score: f32 },
    /// The search result was ignored or not used.
    ResultIgnored,
    /// The user refined the query (partial success).
    QueryRefined { new_query: String },
    /// No signal available (neutral outcome).
    NoSignal,
}

/// Reward configuration for trajectory evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewardConfig {
    pub success_weight: f32,
    pub efficiency_weight: f32,
}

impl Default for RewardConfig {
    fn default() -> Self {
        Self {
            success_weight: 1.0,
            efficiency_weight: 0.3,
        }
    }
}

impl RewardConfig {
    /// Compute reward for a trajectory based on outcome and efficiency.
    pub fn compute(&self, trajectory: &DecompositionTrajectory) -> f32 {
        let success = match &trajectory.outcome {
            Some(TrainingOutcome::ResultUsed { quality_score }) => *quality_score,
            Some(TrainingOutcome::QueryRefined { .. }) => 0.3,
            Some(TrainingOutcome::ResultIgnored) => -0.5,
            Some(TrainingOutcome::NoSignal) | None => 0.0,
        };

        let efficiency = 1.0 - (trajectory.tokens_used as f32 / 100_000.0).min(1.0);

        (self.success_weight * success + self.efficiency_weight * efficiency).clamp(-1.0, 1.0)
    }
}

/// Timestamped action for trajectory tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimestampedAction {
    pub action: crate::rlm::strategy::DecompositionAction,
    pub timestamp: DateTime<Utc>,
    pub intermediate_reward: f32,
}

/// Internal decomposition trajectory for training.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecompositionTrajectory {
    pub id: String,
    pub query: String,
    pub tenant_context: TenantContext,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,

    // Action sequence
    pub actions: Vec<TimestampedAction>,

    // Outcome
    pub result_count: usize,
    pub outcome: Option<TrainingOutcome>,

    // Costs
    pub tokens_used: usize,
    pub max_depth: u8,

    // Computed reward (after outcome known)
    pub reward: Option<f32>,
}

impl DecompositionTrajectory {
    pub fn new(query: SearchQuery, tenant_context: TenantContext) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            query: query.text,
            tenant_context,
            started_at: Utc::now(),
            completed_at: None,
            actions: Vec::new(),
            result_count: 0,
            outcome: None,
            tokens_used: 0,
            max_depth: 0,
            reward: None,
        }
    }

    /// Convert from RlmTrajectory (internal executor format).
    pub fn from_rlm_trajectory(rlm_traj: RlmTrajectory, tenant_context: TenantContext) -> Self {
        let started_at = Utc::now();
        let actions = rlm_traj
            .steps
            .iter()
            .map(|step| TimestampedAction {
                action: step.action.clone(),
                timestamp: started_at,
                intermediate_reward: step.reward,
            })
            .collect();

        Self {
            id: uuid::Uuid::new_v4().to_string(),
            query: rlm_traj.query.text,
            tenant_context,
            started_at,
            completed_at: Some(Utc::now()),
            actions,
            result_count: rlm_traj.steps.len(),
            outcome: None,
            tokens_used: 0, // Would be tracked during actual execution
            max_depth: 0,   // Would be tracked during actual execution
            reward: None,
        }
    }

    /// Record training outcome and compute reward.
    pub fn record_outcome(&mut self, outcome: TrainingOutcome, config: &RewardConfig) {
        self.outcome = Some(outcome);
        self.completed_at = Some(Utc::now());
        self.reward = Some(config.compute(self));
    }

    /// Add an action to the trajectory.
    pub fn add_action(&mut self, action: crate::rlm::strategy::DecompositionAction, reward: f32) {
        self.actions.push(TimestampedAction {
            action,
            timestamp: Utc::now(),
            intermediate_reward: reward,
        });
        self.max_depth = self.max_depth.max(self.actions.len() as u8);
    }

    /// Check if trajectory is complete.
    pub fn is_complete(&self) -> bool {
        self.outcome.is_some() && self.completed_at.is_some()
    }
}

/// Policy state for action selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyState {
    /// Weights for each action type (key: action type, value: weight).
    pub action_weights: HashMap<String, f32>,
    /// Exploration rate (epsilon).
    pub epsilon: f32,
    /// Number of training steps.
    pub step_count: usize,
}

impl Default for PolicyState {
    fn default() -> Self {
        Self {
            action_weights: HashMap::new(),
            epsilon: 0.1,
            step_count: 0,
        }
    }
}

impl PolicyState {
    /// Get weight for an action type.
    pub fn get_weight(&self, action_type: &str) -> f32 {
        *self.action_weights.get(action_type).unwrap_or(&0.5)
    }

    /// Update weight for an action type.
    pub fn update_weight(&mut self, action_type: &str, delta: f32) {
        let weight = self.action_weights.get(action_type).copied().unwrap_or(0.5);
        let new_weight = (weight + delta).clamp(0.0, 1.0);
        self.action_weights
            .insert(action_type.to_string(), new_weight);
    }

    /// Decay exploration rate.
    pub fn decay_epsilon(&mut self, decay_rate: f32) {
        self.epsilon = (self.epsilon * decay_rate).max(0.01);
    }
}

/// RLM Decomposition Trainer for learning optimal decomposition strategies.
pub struct DecompositionTrainer {
    learning_rate: f32,
    gamma: f32,
    #[allow(dead_code)]
    reward_config: RewardConfig,
    policy_state: PolicyState,
}

impl DecompositionTrainer {
    pub fn new(reward_config: RewardConfig) -> Self {
        Self {
            learning_rate: 0.001,
            gamma: 0.95,
            reward_config,
            policy_state: PolicyState::default(),
        }
    }

    pub fn with_config(learning_rate: f32, gamma: f32, reward_config: RewardConfig) -> Self {
        Self {
            learning_rate,
            gamma,
            reward_config,
            policy_state: PolicyState::default(),
        }
    }

    #[tracing::instrument(skip(self, trajectory), fields(trajectory_id = %trajectory.id, action_count = trajectory.actions.len()))]
    pub async fn train(&mut self, trajectory: &DecompositionTrajectory) -> anyhow::Result<()> {
        if trajectory.reward.is_none() {
            return Err(anyhow::anyhow!("Cannot train trajectory without reward"));
        }

        let reward = trajectory.reward.unwrap_or(0.0);
        metrics::histogram!("rlm_training_reward").record(reward as f64);

        let rewards = self.compute_returns(trajectory)?;
        self.update_policy(trajectory, &rewards).await?;

        self.policy_state.step_count += 1;

        if self.policy_state.step_count.is_multiple_of(100) {
            self.policy_state.decay_epsilon(0.995);
        }

        tracing::debug!(
            reward = reward,
            step_count = self.policy_state.step_count,
            epsilon = self.policy_state.epsilon,
            "Trained on trajectory"
        );

        Ok(())
    }

    /// Compute discounted returns for each step.
    pub fn compute_returns(
        &self,
        trajectory: &DecompositionTrajectory,
    ) -> anyhow::Result<Vec<f32>> {
        let mut returns = Vec::with_capacity(trajectory.actions.len());
        let mut running_return = 0.0;

        for action in trajectory.actions.iter().rev() {
            running_return = action.intermediate_reward + self.gamma * running_return;
            returns.push(running_return);
        }

        returns.reverse();
        Ok(returns)
    }

    /// Update policy using REINFORCE-style policy gradient.
    async fn update_policy(
        &mut self,
        trajectory: &DecompositionTrajectory,
        returns: &[f32],
    ) -> anyhow::Result<()> {
        for (action, &return_val) in trajectory.actions.iter().zip(returns.iter()) {
            let action_type = format!("{:?}", action.action);

            // Policy gradient update: increase weight for high-return actions
            let delta = self.learning_rate * return_val;

            self.policy_state.update_weight(&action_type, delta);
        }

        Ok(())
    }

    /// Select action with exploration/exploitation (epsilon-greedy).
    pub fn select_action<'a>(
        &'a self,
        available_actions: &'a [crate::rlm::strategy::DecompositionAction],
    ) -> &'a crate::rlm::strategy::DecompositionAction {
        if available_actions.is_empty() {
            panic!("No actions available for selection");
        }

        if rand::random::<f32>() < self.policy_state.epsilon {
            let idx = rand::random::<usize>() % available_actions.len();
            return &available_actions[idx];
        }

        let mut best_action = &available_actions[0];
        let mut best_weight = -1.0;

        for action in available_actions {
            let action_type = format!("{:?}", action);
            let weight = self.policy_state.get_weight(&action_type);

            if weight > best_weight {
                best_weight = weight;
                best_action = action;
            }
        }

        best_action
    }

    /// Export policy state for persistence.
    pub fn export_state(&self) -> anyhow::Result<PolicyState> {
        Ok(self.policy_state.clone())
    }

    /// Import policy state from persistence.
    pub fn import_state(&mut self, state: PolicyState) -> anyhow::Result<()> {
        let weights_count = state.action_weights.len();
        let epsilon = state.epsilon;
        self.policy_state = state;
        tracing::info!(
            "Imported policy state with {} action weights, epsilon: {:.3}",
            weights_count,
            epsilon
        );
        Ok(())
    }

    /// Get current exploration rate.
    pub fn epsilon(&self) -> f32 {
        self.policy_state.epsilon
    }

    /// Get action weight for a specific action type.
    pub fn action_weight(&self, action_type: &str) -> f32 {
        self.policy_state.get_weight(action_type)
    }
}

impl Default for DecompositionTrainer {
    fn default() -> Self {
        Self::new(RewardConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rlm::executor::TrajectoryStep;
    use mk_core::types::MemoryLayer;

    #[test]
    fn test_reward_config_compute_positive() {
        let config = RewardConfig::default();

        let mut trajectory = DecompositionTrajectory::new(
            SearchQuery {
                text: "test query".to_string(),
                ..Default::default()
            },
            TenantContext::default(),
        );
        trajectory.tokens_used = 10_000;
        trajectory.record_outcome(TrainingOutcome::ResultUsed { quality_score: 0.8 }, &config);

        assert_eq!(trajectory.reward, Some(1.0));
    }

    #[test]
    fn test_reward_config_compute_negative() {
        let config = RewardConfig::default();

        let mut trajectory = DecompositionTrajectory::new(
            SearchQuery {
                text: "test query".to_string(),
                ..Default::default()
            },
            TenantContext::default(),
        );
        trajectory.tokens_used = 50_000;
        trajectory.record_outcome(TrainingOutcome::ResultIgnored, &config);

        assert_eq!(trajectory.reward, Some(-0.5 * 1.0 + 0.5 * 0.3));
    }

    #[test]
    fn test_trajectory_from_rlm() {
        let rlm_traj = RlmTrajectory {
            query: SearchQuery {
                text: "test".to_string(),
                ..Default::default()
            },
            steps: vec![TrajectoryStep {
                action: crate::rlm::strategy::DecompositionAction::SearchLayer {
                    layer: MemoryLayer::Project,
                    query: "subquery".to_string(),
                },
                observation: "result".to_string(),
                reward: 0.5,
                involved_memory_ids: vec!["id1".to_string()],
            }],
            total_reward: 0.5,
        };

        let traj = DecompositionTrajectory::from_rlm_trajectory(rlm_traj, TenantContext::default());
        assert_eq!(traj.query, "test");
        assert_eq!(traj.actions.len(), 1);
    }

    #[test]
    fn test_compute_returns() {
        let trainer = DecompositionTrainer::default();

        let mut trajectory = DecompositionTrajectory::new(
            SearchQuery {
                text: "test".to_string(),
                ..Default::default()
            },
            TenantContext::default(),
        );
        trajectory.add_action(
            crate::rlm::strategy::DecompositionAction::SearchLayer {
                layer: MemoryLayer::Project,
                query: "q1".to_string(),
            },
            1.0,
        );
        trajectory.add_action(
            crate::rlm::strategy::DecompositionAction::Aggregate {
                strategy: crate::rlm::strategy::AggregationStrategy::Summary,
                results: vec![],
            },
            0.5,
        );

        let returns = trainer.compute_returns(&trajectory).unwrap();
        assert_eq!(returns.len(), 2);
        assert!(returns[0] > returns[1]);
    }

    #[test]
    fn test_policy_state_weight_clamping() {
        let mut state = PolicyState::default();
        state.update_weight("test", 1.5);
        assert_eq!(state.get_weight("test"), 1.0);

        state.update_weight("test", -0.5);
        assert_eq!(state.get_weight("test"), 0.5);

        state.update_weight("test", -1.0);
        assert_eq!(state.get_weight("test"), 0.0);
    }

    #[test]
    fn test_policy_state_epsilon_decay() {
        let mut state = PolicyState::default();
        assert_eq!(state.epsilon, 0.1);

        state.decay_epsilon(0.5);
        assert_eq!(state.epsilon, 0.05);

        for _ in 0..100 {
            state.decay_epsilon(0.9);
        }
        assert_eq!(state.epsilon, 0.01);
    }

    #[test]
    fn test_decomposition_trainer_export_import() {
        let trainer1 = DecompositionTrainer::default();
        let state1 = trainer1.export_state().unwrap();

        let mut trainer2 = DecompositionTrainer::default();
        trainer2.import_state(state1).unwrap();

        assert_eq!(trainer1.epsilon(), trainer2.epsilon());
    }

    #[test]
    fn test_decomposition_trainer_select_action_exploitation() {
        let trainer = DecompositionTrainer::default();

        let action1 = crate::rlm::strategy::DecompositionAction::SearchLayer {
            layer: MemoryLayer::Project,
            query: "q1".to_string(),
        };
        let action2 = crate::rlm::strategy::DecompositionAction::SearchLayer {
            layer: MemoryLayer::Team,
            query: "q2".to_string(),
        };

        let actions = vec![action1, action2];

        let selected = trainer.select_action(&actions);
        assert!(actions.contains(selected));
    }
}
