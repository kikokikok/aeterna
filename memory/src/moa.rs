use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, thiserror::Error)]
pub enum MoaError {
    #[error("No agent responses provided")]
    EmptyResponses,
    #[error("Aggregation failed: {reason}")]
    AggregationFailed { reason: String },
    #[error("Convergence not reached after {iterations} iterations")]
    ConvergenceNotReached { iterations: u32 },
    #[error("Invalid confidence value {value}: must be in [0.0, 1.0]")]
    InvalidConfidence { value: f32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResponse {
    pub agent_id: String,
    pub content: String,
    pub confidence: f32,
}

impl AgentResponse {
    pub fn new(agent_id: impl Into<String>, content: impl Into<String>, confidence: f32) -> Self {
        Self {
            agent_id: agent_id.into(),
            content: content.into(),
            confidence,
        }
    }

    fn validate(&self) -> Result<(), MoaError> {
        if !(0.0..=1.0).contains(&self.confidence) {
            return Err(MoaError::InvalidConfidence {
                value: self.confidence,
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoaConfig {
    pub max_iterations: u32,
    pub convergence_threshold: f32,
    pub min_confidence: f32,
}

impl Default for MoaConfig {
    fn default() -> Self {
        Self {
            max_iterations: 5,
            convergence_threshold: 0.05,
            min_confidence: 0.1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedResponse {
    pub content: String,
    pub combined_confidence: f32,
    pub contributing_agents: Vec<String>,
    pub weights: HashMap<String, f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefinementResult {
    pub final_response: AggregatedResponse,
    pub iterations_used: u32,
    pub converged: bool,
    pub history: Vec<AggregatedResponse>,
}

#[derive(Debug, Clone)]
pub struct MixtureOfAgents {
    config: MoaConfig,
}

impl MixtureOfAgents {
    pub fn new(config: MoaConfig) -> Self {
        Self { config }
    }

    pub fn with_defaults() -> Self {
        Self::new(MoaConfig::default())
    }

    pub fn aggregate(&self, responses: &[AgentResponse]) -> Result<AggregatedResponse, MoaError> {
        if responses.is_empty() {
            return Err(MoaError::EmptyResponses);
        }

        for r in responses {
            r.validate()?;
        }

        let filtered: Vec<&AgentResponse> = responses
            .iter()
            .filter(|r| r.confidence >= self.config.min_confidence)
            .collect();

        if filtered.is_empty() {
            return Err(MoaError::AggregationFailed {
                reason: format!(
                    "All responses below min_confidence threshold {}",
                    self.config.min_confidence
                ),
            });
        }

        let total_confidence: f32 = filtered.iter().map(|r| r.confidence).sum();

        let mut weights = HashMap::new();
        let mut contributing_agents = Vec::new();
        let mut best_idx = 0;
        let mut best_weight: f32 = 0.0;

        for (i, resp) in filtered.iter().enumerate() {
            let weight = resp.confidence / total_confidence;
            weights.insert(resp.agent_id.clone(), weight);
            contributing_agents.push(resp.agent_id.clone());
            if weight > best_weight {
                best_weight = weight;
                best_idx = i;
            }
        }

        let combined_confidence = total_confidence / filtered.len() as f32;

        Ok(AggregatedResponse {
            content: filtered[best_idx].content.clone(),
            combined_confidence,
            contributing_agents,
            weights,
        })
    }

    pub fn refine(
        &self,
        initial_responses: &[AgentResponse],
        mut refine_fn: impl FnMut(&AggregatedResponse, u32) -> Vec<AgentResponse>,
    ) -> Result<RefinementResult, MoaError> {
        let mut current = self.aggregate(initial_responses)?;
        let mut history = vec![current.clone()];

        for iteration in 1..=self.config.max_iterations {
            let refined_responses = refine_fn(&current, iteration);
            if refined_responses.is_empty() {
                return Ok(RefinementResult {
                    final_response: current,
                    iterations_used: iteration,
                    converged: true,
                    history,
                });
            }

            let next = self.aggregate(&refined_responses)?;

            let delta = (next.combined_confidence - current.combined_confidence).abs();
            history.push(next.clone());

            if delta < self.config.convergence_threshold {
                return Ok(RefinementResult {
                    final_response: next,
                    iterations_used: iteration,
                    converged: true,
                    history,
                });
            }

            current = next;
        }

        Ok(RefinementResult {
            final_response: current,
            iterations_used: self.config.max_iterations,
            converged: false,
            history,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_responses() -> Vec<AgentResponse> {
        vec![
            AgentResponse::new("agent-1", "Rust is fast", 0.9),
            AgentResponse::new("agent-2", "Rust is safe", 0.7),
            AgentResponse::new("agent-3", "Rust is concurrent", 0.5),
        ]
    }

    #[test]
    fn test_aggregate_weighted_by_confidence() {
        let moa = MixtureOfAgents::with_defaults();
        let responses = sample_responses();

        let result = moa
            .aggregate(&responses)
            .expect("aggregation should succeed");

        assert_eq!(result.contributing_agents.len(), 3);
        assert_eq!(result.content, "Rust is fast");

        let w1 = result.weights.get("agent-1").copied().unwrap_or(0.0);
        let w2 = result.weights.get("agent-2").copied().unwrap_or(0.0);
        assert!(w1 > w2, "higher confidence agent should have higher weight");

        let total_weight: f32 = result.weights.values().sum();
        assert!(
            (total_weight - 1.0).abs() < 1e-5,
            "weights should sum to 1.0"
        );
    }

    #[test]
    fn test_aggregate_empty_responses_errors() {
        let moa = MixtureOfAgents::with_defaults();
        let result = moa.aggregate(&[]);
        assert!(matches!(result, Err(MoaError::EmptyResponses)));
    }

    #[test]
    fn test_aggregate_filters_low_confidence() {
        let config = MoaConfig {
            min_confidence: 0.6,
            ..Default::default()
        };
        let moa = MixtureOfAgents::new(config);
        let responses = sample_responses();

        let result = moa
            .aggregate(&responses)
            .expect("aggregation should succeed");
        assert_eq!(result.contributing_agents.len(), 2);
        assert!(!result.contributing_agents.contains(&"agent-3".to_string()));
    }

    #[test]
    fn test_invalid_confidence_rejected() {
        let moa = MixtureOfAgents::with_defaults();
        let responses = vec![AgentResponse::new("bad", "content", 1.5)];

        let result = moa.aggregate(&responses);
        assert!(matches!(result, Err(MoaError::InvalidConfidence { .. })));
    }

    #[test]
    fn test_refine_converges() {
        let config = MoaConfig {
            max_iterations: 10,
            convergence_threshold: 0.05,
            min_confidence: 0.0,
        };
        let moa = MixtureOfAgents::new(config);
        let initial = sample_responses();

        let result = moa
            .refine(&initial, |_prev, iteration| {
                vec![
                    AgentResponse::new(
                        "agent-1",
                        "converged answer",
                        0.8 + 0.01 * iteration as f32,
                    ),
                    AgentResponse::new(
                        "agent-2",
                        "converged answer",
                        0.75 + 0.01 * iteration as f32,
                    ),
                ]
            })
            .expect("refine should succeed");

        assert!(result.converged);
        assert!(result.iterations_used <= 10);
        assert!(result.history.len() >= 2);
    }

    #[test]
    fn test_refine_stops_on_empty_refinement() {
        let moa = MixtureOfAgents::with_defaults();
        let initial = sample_responses();

        let result = moa
            .refine(&initial, |_prev, _iter| vec![])
            .expect("refine should succeed");

        assert!(result.converged);
        assert_eq!(result.iterations_used, 1);
    }
}
