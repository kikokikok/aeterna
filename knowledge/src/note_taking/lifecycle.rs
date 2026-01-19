use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{info, instrument, warn};

use super::generator::GeneratedNote;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum NoteStatus {
    #[default]
    Draft,
    Proposed,
    Accepted,
    Rejected,
    Deprecated,
}

impl NoteStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Proposed => "proposed",
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
            Self::Deprecated => "deprecated",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Rejected | Self::Deprecated)
    }

    pub fn is_active(&self) -> bool {
        matches!(self, Self::Draft | Self::Proposed | Self::Accepted)
    }
}

#[derive(Debug, Clone)]
pub struct LifecycleConfig {
    pub auto_propose_usefulness_threshold: f32,
    pub auto_propose_retrieval_threshold: u32,
    pub deprecation_retrieval_threshold: u32,
    pub deprecation_usefulness_ratio: f32,
}

impl Default for LifecycleConfig {
    fn default() -> Self {
        Self {
            auto_propose_usefulness_threshold: 0.8,
            auto_propose_retrieval_threshold: 5,
            deprecation_retrieval_threshold: 10,
            deprecation_usefulness_ratio: 0.1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteWithLifecycle {
    pub note: GeneratedNote,
    pub status: NoteStatus,
    pub usefulness_score: f32,
    pub retrieval_count: u32,
    pub status_changed_at: u64,
    pub review_flagged: bool,
    pub deprecation_reason: Option<String>,
}

impl NoteWithLifecycle {
    pub fn new(note: GeneratedNote) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Self {
            note,
            status: NoteStatus::Draft,
            usefulness_score: 0.0,
            retrieval_count: 0,
            status_changed_at: now,
            review_flagged: false,
            deprecation_reason: None,
        }
    }

    pub fn id(&self) -> &str {
        &self.note.id
    }

    pub fn usefulness_ratio(&self) -> f32 {
        if self.retrieval_count == 0 {
            return 0.0;
        }
        self.usefulness_score / self.retrieval_count as f32
    }

    #[instrument(skip(self), fields(note_id = %self.id(), current_status = ?self.status))]
    pub fn transition_to(
        &mut self,
        new_status: NoteStatus,
    ) -> Result<NoteStatus, LifecycleTransitionError> {
        let old_status = self.status;

        if !Self::is_valid_transition(old_status, new_status) {
            return Err(LifecycleTransitionError::InvalidTransition {
                from: old_status,
                to: new_status,
            });
        }

        self.status = new_status;
        self.status_changed_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        info!(
            from = ?old_status,
            to = ?new_status,
            "Note status transitioned"
        );

        Ok(old_status)
    }

    fn is_valid_transition(from: NoteStatus, to: NoteStatus) -> bool {
        use NoteStatus::*;

        matches!(
            (from, to),
            (Draft, Proposed)
                | (Draft, Rejected)
                | (Draft, Deprecated)
                | (Proposed, Accepted)
                | (Proposed, Rejected)
                | (Proposed, Draft)
                | (Accepted, Deprecated)
        )
    }

    pub fn record_retrieval(&mut self) {
        self.retrieval_count = self.retrieval_count.saturating_add(1);
    }

    pub fn record_positive_feedback(&mut self, weight: f32) {
        self.usefulness_score += weight.clamp(0.0, 1.0);
    }

    pub fn record_negative_feedback(&mut self, weight: f32) {
        self.usefulness_score = (self.usefulness_score - weight.clamp(0.0, 1.0)).max(0.0);
    }

    pub fn should_auto_propose(&self, config: &LifecycleConfig) -> bool {
        self.status == NoteStatus::Draft
            && self.usefulness_score >= config.auto_propose_usefulness_threshold
            && self.retrieval_count >= config.auto_propose_retrieval_threshold
    }

    pub fn should_flag_for_review(&self, config: &LifecycleConfig) -> bool {
        self.retrieval_count >= config.deprecation_retrieval_threshold
            && self.usefulness_ratio() < config.deprecation_usefulness_ratio
    }

    #[instrument(skip(self, config), fields(note_id = %self.id()))]
    pub fn evaluate_auto_transitions(
        &mut self,
        config: &LifecycleConfig,
    ) -> Option<AutoTransitionResult> {
        if self.should_auto_propose(config) {
            match self.transition_to(NoteStatus::Proposed) {
                Ok(old) => {
                    info!("Auto-proposed note due to high usefulness");
                    return Some(AutoTransitionResult::Proposed { from: old });
                }
                Err(e) => {
                    warn!(error = %e, "Failed to auto-propose note");
                }
            }
        }

        if self.should_flag_for_review(config) && !self.review_flagged {
            self.review_flagged = true;
            info!(
                usefulness_ratio = self.usefulness_ratio(),
                retrieval_count = self.retrieval_count,
                "Note flagged for review due to low usefulness"
            );
            return Some(AutoTransitionResult::FlaggedForReview);
        }

        None
    }

    pub fn deprecate(&mut self, reason: impl Into<String>) -> Result<(), LifecycleTransitionError> {
        self.deprecation_reason = Some(reason.into());
        self.transition_to(NoteStatus::Deprecated)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AutoTransitionResult {
    Proposed { from: NoteStatus },
    FlaggedForReview,
}

#[derive(Debug, Clone, Error)]
pub enum LifecycleTransitionError {
    #[error("Invalid transition from {from:?} to {to:?}")]
    InvalidTransition { from: NoteStatus, to: NoteStatus },
}

pub struct NoteLifecycleManager {
    config: LifecycleConfig,
}

impl NoteLifecycleManager {
    pub fn new(config: LifecycleConfig) -> Self {
        Self { config }
    }

    pub fn wrap_note(&self, note: GeneratedNote) -> NoteWithLifecycle {
        NoteWithLifecycle::new(note)
    }

    pub fn record_retrieval(&self, note: &mut NoteWithLifecycle) {
        note.record_retrieval();
        note.evaluate_auto_transitions(&self.config);
    }

    pub fn record_feedback(&self, note: &mut NoteWithLifecycle, positive: bool, weight: f32) {
        if positive {
            note.record_positive_feedback(weight);
        } else {
            note.record_negative_feedback(weight);
        }
        note.evaluate_auto_transitions(&self.config);
    }

    pub fn propose(&self, note: &mut NoteWithLifecycle) -> Result<(), LifecycleTransitionError> {
        note.transition_to(NoteStatus::Proposed)?;
        Ok(())
    }

    pub fn accept(&self, note: &mut NoteWithLifecycle) -> Result<(), LifecycleTransitionError> {
        note.transition_to(NoteStatus::Accepted)?;
        Ok(())
    }

    pub fn reject(&self, note: &mut NoteWithLifecycle) -> Result<(), LifecycleTransitionError> {
        note.transition_to(NoteStatus::Rejected)?;
        Ok(())
    }

    pub fn deprecate(
        &self,
        note: &mut NoteWithLifecycle,
        reason: impl Into<String>,
    ) -> Result<(), LifecycleTransitionError> {
        note.deprecate(reason)
    }

    pub fn evaluate_batch(
        &self,
        notes: &mut [NoteWithLifecycle],
    ) -> Vec<(String, AutoTransitionResult)> {
        notes
            .iter_mut()
            .filter_map(|note| {
                note.evaluate_auto_transitions(&self.config)
                    .map(|result| (note.id().to_string(), result))
            })
            .collect()
    }

    pub fn config(&self) -> &LifecycleConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_note(id: &str) -> GeneratedNote {
        GeneratedNote {
            id: id.to_string(),
            title: "Test Note".to_string(),
            content: "Test content".to_string(),
            tags: vec!["test".to_string()],
            source_distillation_id: "dist-123".to_string(),
            created_at: 1000,
            quality_score: 0.8,
        }
    }

    #[test]
    fn test_new_note_is_draft() {
        let note = NoteWithLifecycle::new(sample_note("1"));
        assert_eq!(note.status, NoteStatus::Draft);
        assert_eq!(note.usefulness_score, 0.0);
        assert_eq!(note.retrieval_count, 0);
    }

    #[test]
    fn test_valid_transitions() {
        let mut note = NoteWithLifecycle::new(sample_note("1"));

        assert!(note.transition_to(NoteStatus::Proposed).is_ok());
        assert_eq!(note.status, NoteStatus::Proposed);

        assert!(note.transition_to(NoteStatus::Accepted).is_ok());
        assert_eq!(note.status, NoteStatus::Accepted);

        assert!(note.transition_to(NoteStatus::Deprecated).is_ok());
        assert_eq!(note.status, NoteStatus::Deprecated);
    }

    #[test]
    fn test_invalid_transition_draft_to_accepted() {
        let mut note = NoteWithLifecycle::new(sample_note("1"));
        let result = note.transition_to(NoteStatus::Accepted);

        assert!(matches!(
            result,
            Err(LifecycleTransitionError::InvalidTransition { .. })
        ));
        assert_eq!(note.status, NoteStatus::Draft);
    }

    #[test]
    fn test_invalid_transition_accepted_to_proposed() {
        let mut note = NoteWithLifecycle::new(sample_note("1"));
        note.transition_to(NoteStatus::Proposed).unwrap();
        note.transition_to(NoteStatus::Accepted).unwrap();

        let result = note.transition_to(NoteStatus::Proposed);
        assert!(result.is_err());
    }

    #[test]
    fn test_record_retrieval_increments_count() {
        let mut note = NoteWithLifecycle::new(sample_note("1"));

        note.record_retrieval();
        assert_eq!(note.retrieval_count, 1);

        note.record_retrieval();
        assert_eq!(note.retrieval_count, 2);
    }

    #[test]
    fn test_record_positive_feedback() {
        let mut note = NoteWithLifecycle::new(sample_note("1"));

        note.record_positive_feedback(0.5);
        assert!((note.usefulness_score - 0.5).abs() < 0.001);

        note.record_positive_feedback(0.3);
        assert!((note.usefulness_score - 0.8).abs() < 0.001);
    }

    #[test]
    fn test_record_negative_feedback() {
        let mut note = NoteWithLifecycle::new(sample_note("1"));
        note.usefulness_score = 1.0;

        note.record_negative_feedback(0.3);
        assert!((note.usefulness_score - 0.7).abs() < 0.001);
    }

    #[test]
    fn test_negative_feedback_clamps_at_zero() {
        let mut note = NoteWithLifecycle::new(sample_note("1"));
        note.usefulness_score = 0.2;

        note.record_negative_feedback(0.5);
        assert_eq!(note.usefulness_score, 0.0);
    }

    #[test]
    fn test_usefulness_ratio_calculation() {
        let mut note = NoteWithLifecycle::new(sample_note("1"));

        assert_eq!(note.usefulness_ratio(), 0.0);

        note.retrieval_count = 10;
        note.usefulness_score = 2.0;

        assert!((note.usefulness_ratio() - 0.2).abs() < 0.001);
    }

    #[test]
    fn test_should_auto_propose() {
        let config = LifecycleConfig {
            auto_propose_usefulness_threshold: 0.8,
            auto_propose_retrieval_threshold: 5,
            ..Default::default()
        };
        let mut note = NoteWithLifecycle::new(sample_note("1"));

        assert!(!note.should_auto_propose(&config));

        note.usefulness_score = 0.9;
        assert!(!note.should_auto_propose(&config));

        note.retrieval_count = 5;
        assert!(note.should_auto_propose(&config));

        note.transition_to(NoteStatus::Proposed).unwrap();
        assert!(!note.should_auto_propose(&config));
    }

    #[test]
    fn test_should_flag_for_review() {
        let config = LifecycleConfig {
            deprecation_retrieval_threshold: 10,
            deprecation_usefulness_ratio: 0.1,
            ..Default::default()
        };
        let mut note = NoteWithLifecycle::new(sample_note("1"));

        assert!(!note.should_flag_for_review(&config));

        note.retrieval_count = 15;
        note.usefulness_score = 0.5;

        assert!(note.should_flag_for_review(&config));
    }

    #[test]
    fn test_evaluate_auto_transitions_proposes() {
        let config = LifecycleConfig {
            auto_propose_usefulness_threshold: 0.8,
            auto_propose_retrieval_threshold: 5,
            ..Default::default()
        };
        let mut note = NoteWithLifecycle::new(sample_note("1"));
        note.usefulness_score = 0.9;
        note.retrieval_count = 6;

        let result = note.evaluate_auto_transitions(&config);

        assert!(matches!(
            result,
            Some(AutoTransitionResult::Proposed {
                from: NoteStatus::Draft
            })
        ));
        assert_eq!(note.status, NoteStatus::Proposed);
    }

    #[test]
    fn test_evaluate_auto_transitions_flags_for_review() {
        let config = LifecycleConfig {
            deprecation_retrieval_threshold: 10,
            deprecation_usefulness_ratio: 0.1,
            ..Default::default()
        };
        let mut note = NoteWithLifecycle::new(sample_note("1"));
        note.retrieval_count = 15;
        note.usefulness_score = 0.5;

        let result = note.evaluate_auto_transitions(&config);

        assert!(matches!(
            result,
            Some(AutoTransitionResult::FlaggedForReview)
        ));
        assert!(note.review_flagged);
    }

    #[test]
    fn test_flag_for_review_only_once() {
        let config = LifecycleConfig {
            deprecation_retrieval_threshold: 10,
            deprecation_usefulness_ratio: 0.1,
            ..Default::default()
        };
        let mut note = NoteWithLifecycle::new(sample_note("1"));
        note.retrieval_count = 15;
        note.usefulness_score = 0.5;

        let result1 = note.evaluate_auto_transitions(&config);
        assert!(result1.is_some());

        let result2 = note.evaluate_auto_transitions(&config);
        assert!(result2.is_none());
    }

    #[test]
    fn test_deprecate_with_reason() {
        let mut note = NoteWithLifecycle::new(sample_note("1"));
        note.transition_to(NoteStatus::Proposed).unwrap();
        note.transition_to(NoteStatus::Accepted).unwrap();

        note.deprecate("No longer relevant").unwrap();

        assert_eq!(note.status, NoteStatus::Deprecated);
        assert_eq!(
            note.deprecation_reason,
            Some("No longer relevant".to_string())
        );
    }

    #[test]
    fn test_lifecycle_manager_record_retrieval() {
        let manager = NoteLifecycleManager::new(LifecycleConfig::default());
        let note = sample_note("1");
        let mut wrapped = manager.wrap_note(note);

        manager.record_retrieval(&mut wrapped);

        assert_eq!(wrapped.retrieval_count, 1);
    }

    #[test]
    fn test_lifecycle_manager_record_feedback() {
        let manager = NoteLifecycleManager::new(LifecycleConfig::default());
        let note = sample_note("1");
        let mut wrapped = manager.wrap_note(note);

        manager.record_feedback(&mut wrapped, true, 0.5);
        assert!((wrapped.usefulness_score - 0.5).abs() < 0.001);

        manager.record_feedback(&mut wrapped, false, 0.2);
        assert!((wrapped.usefulness_score - 0.3).abs() < 0.001);
    }

    #[test]
    fn test_lifecycle_manager_workflow() {
        let manager = NoteLifecycleManager::new(LifecycleConfig::default());
        let note = sample_note("1");
        let mut wrapped = manager.wrap_note(note);

        assert!(manager.propose(&mut wrapped).is_ok());
        assert_eq!(wrapped.status, NoteStatus::Proposed);

        assert!(manager.accept(&mut wrapped).is_ok());
        assert_eq!(wrapped.status, NoteStatus::Accepted);

        assert!(manager.deprecate(&mut wrapped, "Outdated").is_ok());
        assert_eq!(wrapped.status, NoteStatus::Deprecated);
    }

    #[test]
    fn test_lifecycle_manager_reject() {
        let manager = NoteLifecycleManager::new(LifecycleConfig::default());
        let note = sample_note("1");
        let mut wrapped = manager.wrap_note(note);

        manager.propose(&mut wrapped).unwrap();
        assert!(manager.reject(&mut wrapped).is_ok());
        assert_eq!(wrapped.status, NoteStatus::Rejected);
    }

    #[test]
    fn test_evaluate_batch() {
        let config = LifecycleConfig {
            auto_propose_usefulness_threshold: 0.8,
            auto_propose_retrieval_threshold: 5,
            ..Default::default()
        };
        let manager = NoteLifecycleManager::new(config);

        let mut notes: Vec<NoteWithLifecycle> = (0..3)
            .map(|i| {
                let mut note = manager.wrap_note(sample_note(&i.to_string()));
                if i == 1 {
                    note.usefulness_score = 0.9;
                    note.retrieval_count = 6;
                }
                note
            })
            .collect();

        let transitions = manager.evaluate_batch(&mut notes);

        assert_eq!(transitions.len(), 1);
        assert_eq!(transitions[0].0, "1");
        assert!(matches!(
            transitions[0].1,
            AutoTransitionResult::Proposed { .. }
        ));
    }

    #[test]
    fn test_note_status_as_str() {
        assert_eq!(NoteStatus::Draft.as_str(), "draft");
        assert_eq!(NoteStatus::Proposed.as_str(), "proposed");
        assert_eq!(NoteStatus::Accepted.as_str(), "accepted");
        assert_eq!(NoteStatus::Rejected.as_str(), "rejected");
        assert_eq!(NoteStatus::Deprecated.as_str(), "deprecated");
    }

    #[test]
    fn test_note_status_is_terminal() {
        assert!(!NoteStatus::Draft.is_terminal());
        assert!(!NoteStatus::Proposed.is_terminal());
        assert!(!NoteStatus::Accepted.is_terminal());
        assert!(NoteStatus::Rejected.is_terminal());
        assert!(NoteStatus::Deprecated.is_terminal());
    }

    #[test]
    fn test_note_status_is_active() {
        assert!(NoteStatus::Draft.is_active());
        assert!(NoteStatus::Proposed.is_active());
        assert!(NoteStatus::Accepted.is_active());
        assert!(!NoteStatus::Rejected.is_active());
        assert!(!NoteStatus::Deprecated.is_active());
    }

    #[test]
    fn test_proposed_can_return_to_draft() {
        let mut note = NoteWithLifecycle::new(sample_note("1"));
        note.transition_to(NoteStatus::Proposed).unwrap();

        assert!(note.transition_to(NoteStatus::Draft).is_ok());
        assert_eq!(note.status, NoteStatus::Draft);
    }

    #[test]
    fn test_feedback_weight_clamped() {
        let mut note = NoteWithLifecycle::new(sample_note("1"));

        note.record_positive_feedback(2.0);
        assert!((note.usefulness_score - 1.0).abs() < 0.001);

        note.record_positive_feedback(-1.0);
        assert!((note.usefulness_score - 1.0).abs() < 0.001);
    }
}
