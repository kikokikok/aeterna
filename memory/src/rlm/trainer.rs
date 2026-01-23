use crate::rlm::executor::RlmTrajectory;

pub struct DecompositionTrainer {
    _learning_rate: f32,
    gamma: f32,
}

impl DecompositionTrainer {
    pub fn new() -> Self {
        Self {
            _learning_rate: 0.001,
            gamma: 0.95,
        }
    }

    pub async fn train(&self, trajectory: RlmTrajectory) -> anyhow::Result<()> {
        let rewards = self.calculate_discounted_rewards(&trajectory);

        tracing::debug!(
            "Training on trajectory with {} steps. Total reward: {}. Discounted rewards: {:?}",
            trajectory.steps.len(),
            trajectory.total_reward,
            rewards
        );

        Ok(())
    }

    fn calculate_discounted_rewards(&self, trajectory: &RlmTrajectory) -> Vec<f32> {
        let mut rewards = Vec::with_capacity(trajectory.steps.len());
        let mut running_reward = 0.0;

        for step in trajectory.steps.iter().rev() {
            running_reward = step.reward + self.gamma * running_reward;
            rewards.push(running_reward);
        }

        rewards.reverse();
        rewards
    }
}
