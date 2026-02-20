use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, thiserror::Error)]
pub enum ActiveLearningError {
    #[error("No similarity scores provided")]
    EmptyScores,
    #[error("Score out of range [{min}, {max}]: {value}")]
    ScoreOutOfRange { value: f32, min: f32, max: f32 },
    #[error("Feedback storage failed: {reason}")]
    StorageFailed { reason: String },
    #[error("No candidate examples available")]
    NoCandidates,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UncertaintyScore {
    pub score: f32,
    pub reason: String,
}

impl UncertaintyScore {
    pub fn new(score: f32, reason: impl Into<String>) -> Self {
        Self {
            score,
            reason: reason.into(),
        }
    }

    pub fn is_high(&self, threshold: f32) -> bool {
        self.score >= threshold
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackRequest {
    pub query: String,
    pub uncertainty: UncertaintyScore,
    pub candidate_examples: Vec<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackRecord {
    pub request_id: String,
    pub query: String,
    pub selected_example: Option<String>,
    pub reward: f32,
    pub feedback_text: Option<String>,
    pub recorded_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveLearnerConfig {
    pub uncertainty_threshold: f32,
    pub max_candidates: usize,
    pub reward_positive: f32,
    pub reward_negative: f32,
    pub entropy_base: f32,
}

impl Default for ActiveLearnerConfig {
    fn default() -> Self {
        Self {
            uncertainty_threshold: 0.6,
            max_candidates: 5,
            reward_positive: 1.0,
            reward_negative: -0.5,
            entropy_base: 2.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ActiveLearner {
    config: ActiveLearnerConfig,
    feedback_store: Vec<FeedbackRecord>,
    reward_accumulator: HashMap<String, f32>,
}

impl ActiveLearner {
    pub fn new(config: ActiveLearnerConfig) -> Self {
        Self {
            config,
            feedback_store: Vec::new(),
            reward_accumulator: HashMap::new(),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(ActiveLearnerConfig::default())
    }

    pub fn score_uncertainty(
        &self,
        similarity_scores: &[f32],
    ) -> Result<UncertaintyScore, ActiveLearningError> {
        if similarity_scores.is_empty() {
            return Err(ActiveLearningError::EmptyScores);
        }

        for &s in similarity_scores {
            if !(0.0..=1.0).contains(&s) {
                return Err(ActiveLearningError::ScoreOutOfRange {
                    value: s,
                    min: 0.0,
                    max: 1.0,
                });
            }
        }

        let entropy = Self::normalized_entropy(similarity_scores, self.config.entropy_base);

        let max_score = similarity_scores
            .iter()
            .copied()
            .fold(f32::NEG_INFINITY, f32::max);
        let margin = if similarity_scores.len() >= 2 {
            let mut sorted: Vec<f32> = similarity_scores.to_vec();
            sorted.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
            sorted[0] - sorted[1]
        } else {
            max_score
        };

        let uncertainty = 0.6 * entropy + 0.4 * (1.0 - margin);

        let reason = if entropy > 0.8 {
            "High entropy: similarity scores are nearly uniform, indicating ambiguity"
        } else if margin < 0.1 {
            "Low margin: top candidates are too close in similarity"
        } else if max_score < 0.5 {
            "Low confidence: best match has low similarity score"
        } else {
            "Moderate uncertainty from combined entropy and margin analysis"
        };

        Ok(UncertaintyScore::new(uncertainty, reason))
    }

    pub fn request_feedback(
        &self,
        query: &str,
        uncertainty: UncertaintyScore,
        candidate_labels: &[String],
    ) -> Result<Option<FeedbackRequest>, ActiveLearningError> {
        if !uncertainty.is_high(self.config.uncertainty_threshold) {
            return Ok(None);
        }

        if candidate_labels.is_empty() {
            return Err(ActiveLearningError::NoCandidates);
        }

        let candidates: Vec<String> = candidate_labels
            .iter()
            .take(self.config.max_candidates)
            .cloned()
            .collect();

        Ok(Some(FeedbackRequest {
            query: query.to_string(),
            uncertainty,
            candidate_examples: candidates,
            created_at: Utc::now(),
        }))
    }

    pub fn record_feedback(
        &mut self,
        request_id: impl Into<String>,
        query: impl Into<String>,
        selected_example: Option<String>,
        is_positive: bool,
        feedback_text: Option<String>,
    ) -> FeedbackRecord {
        let reward = if is_positive {
            self.config.reward_positive
        } else {
            self.config.reward_negative
        };

        let query_str = query.into();
        let record = FeedbackRecord {
            request_id: request_id.into(),
            query: query_str.clone(),
            selected_example: selected_example.clone(),
            reward,
            feedback_text,
            recorded_at: Utc::now(),
        };

        let acc_key = selected_example.unwrap_or(query_str);
        *self.reward_accumulator.entry(acc_key).or_insert(0.0) += reward;

        self.feedback_store.push(record.clone());
        record
    }

    pub fn get_accumulated_reward(&self, key: &str) -> f32 {
        self.reward_accumulator.get(key).copied().unwrap_or(0.0)
    }

    pub fn feedback_count(&self) -> usize {
        self.feedback_store.len()
    }

    pub fn recent_feedback(&self, n: usize) -> &[FeedbackRecord] {
        let start = self.feedback_store.len().saturating_sub(n);
        &self.feedback_store[start..]
    }

    fn normalized_entropy(scores: &[f32], base: f32) -> f32 {
        let sum: f32 = scores.iter().sum();
        if sum < f32::EPSILON {
            return 1.0;
        }

        let n = scores.len() as f32;
        let max_entropy = n.log(base);
        if max_entropy < f32::EPSILON {
            return 0.0;
        }

        let entropy: f32 = scores
            .iter()
            .map(|&s| {
                let p = s / sum;
                if p < f32::EPSILON {
                    0.0
                } else {
                    -p * p.log(base)
                }
            })
            .sum();

        (entropy / max_entropy).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_high_entropy_means_high_uncertainty() {
        let learner = ActiveLearner::with_defaults();
        let uniform_scores = vec![0.25, 0.25, 0.25, 0.25];

        let result = learner
            .score_uncertainty(&uniform_scores)
            .expect("scoring should succeed");

        assert!(
            result.score > 0.5,
            "uniform distribution should yield high uncertainty, got {}",
            result.score
        );
    }

    #[test]
    fn test_low_entropy_means_low_uncertainty() {
        let learner = ActiveLearner::with_defaults();
        let peaked_scores = vec![0.95, 0.02, 0.01, 0.01];

        let result = learner
            .score_uncertainty(&peaked_scores)
            .expect("scoring should succeed");

        assert!(
            result.score < 0.5,
            "peaked distribution should yield low uncertainty, got {}",
            result.score
        );
    }

    #[test]
    fn test_score_uncertainty_empty_scores_errors() {
        let learner = ActiveLearner::with_defaults();
        let result = learner.score_uncertainty(&[]);
        assert!(matches!(result, Err(ActiveLearningError::EmptyScores)));
    }

    #[test]
    fn test_score_out_of_range_errors() {
        let learner = ActiveLearner::with_defaults();
        let result = learner.score_uncertainty(&[0.5, 1.5]);
        assert!(matches!(
            result,
            Err(ActiveLearningError::ScoreOutOfRange { .. })
        ));
    }

    #[test]
    fn test_request_feedback_returns_none_below_threshold() {
        let learner = ActiveLearner::with_defaults();
        let low_uncertainty = UncertaintyScore::new(0.3, "low");
        let candidates = vec!["a".to_string()];

        let result = learner
            .request_feedback("query", low_uncertainty, &candidates)
            .expect("request should succeed");

        assert!(
            result.is_none(),
            "should not request feedback for low uncertainty"
        );
    }

    #[test]
    fn test_request_feedback_returns_request_above_threshold() {
        let learner = ActiveLearner::with_defaults();
        let high_uncertainty = UncertaintyScore::new(0.8, "high");
        let candidates = vec!["a".to_string(), "b".to_string()];

        let result = learner
            .request_feedback("query", high_uncertainty, &candidates)
            .expect("request should succeed");

        assert!(result.is_some());
        let req = result.expect("should have feedback request");
        assert_eq!(req.candidate_examples.len(), 2);
        assert_eq!(req.query, "query");
    }

    #[test]
    fn test_request_feedback_no_candidates_errors() {
        let learner = ActiveLearner::with_defaults();
        let high_uncertainty = UncertaintyScore::new(0.9, "high");
        let result = learner.request_feedback("query", high_uncertainty, &[]);
        assert!(matches!(result, Err(ActiveLearningError::NoCandidates)));
    }

    #[test]
    fn test_record_feedback_stores_reward() {
        let mut learner = ActiveLearner::with_defaults();

        let record = learner.record_feedback(
            "req-1",
            "test query",
            Some("example-a".to_string()),
            true,
            None,
        );

        assert_eq!(record.reward, 1.0);
        assert_eq!(learner.feedback_count(), 1);
        assert!((learner.get_accumulated_reward("example-a") - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_record_negative_feedback() {
        let mut learner = ActiveLearner::with_defaults();

        let record = learner.record_feedback(
            "req-2",
            "query",
            Some("example-b".to_string()),
            false,
            Some("not helpful".to_string()),
        );

        assert_eq!(record.reward, -0.5);
        assert!((learner.get_accumulated_reward("example-b") - (-0.5)).abs() < f32::EPSILON);
    }

    #[test]
    fn test_reward_accumulates_across_feedback() {
        let mut learner = ActiveLearner::with_defaults();

        learner.record_feedback("r1", "q", Some("ex".to_string()), true, None);
        learner.record_feedback("r2", "q", Some("ex".to_string()), true, None);
        learner.record_feedback("r3", "q", Some("ex".to_string()), false, None);

        let accumulated = learner.get_accumulated_reward("ex");
        let expected = 1.0 + 1.0 + (-0.5);
        assert!(
            (accumulated - expected).abs() < f32::EPSILON,
            "expected {expected}, got {accumulated}"
        );
        assert_eq!(learner.feedback_count(), 3);
    }

    #[test]
    fn test_recent_feedback_returns_latest() {
        let mut learner = ActiveLearner::with_defaults();

        for i in 0..5 {
            learner.record_feedback(format!("r{i}"), format!("q{i}"), None, true, None);
        }

        let recent = learner.recent_feedback(2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].request_id, "r3");
        assert_eq!(recent[1].request_id, "r4");
    }
}
