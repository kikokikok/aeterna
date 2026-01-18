use std::collections::HashSet;
use std::sync::Arc;

use mk_core::types::{ErrorSignature, HindsightNote};
use storage::postgres::{PostgresBackend, PostgresError};
use tracing::{Instrument, info_span};

#[derive(Debug, Clone)]
pub struct HindsightRetrievalConfig {
    pub max_results: usize,
    pub max_candidate_notes: usize,
    pub relevance_threshold: f32,
    pub semantic_threshold: f32,
    pub recency_weight: f32,
    pub success_weight: f32,
    pub enable_tag_filtering: bool
}

impl Default for HindsightRetrievalConfig {
    fn default() -> Self {
        Self {
            max_results: 5,
            max_candidate_notes: 50,
            relevance_threshold: 0.4,
            semantic_threshold: 0.8,
            recency_weight: 0.2,
            success_weight: 0.2,
            enable_tag_filtering: true
        }
    }
}

#[derive(Debug, Clone)]
pub struct HindsightRetrievalFilter {
    pub tags: Option<Vec<String>>,
    pub min_success_rate: Option<f32>,
    pub created_after: Option<i64>,
    pub created_before: Option<i64>
}

impl Default for HindsightRetrievalFilter {
    fn default() -> Self {
        Self {
            tags: None,
            min_success_rate: None,
            created_after: None,
            created_before: None
        }
    }
}

impl HindsightRetrievalFilter {
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = Some(tags);
        self
    }

    pub fn with_min_success_rate(mut self, rate: f32) -> Self {
        self.min_success_rate = Some(rate);
        self
    }

    pub fn created_after(mut self, ts: i64) -> Self {
        self.created_after = Some(ts);
        self
    }

    pub fn created_before(mut self, ts: i64) -> Self {
        self.created_before = Some(ts);
        self
    }
}

#[derive(Debug, Clone)]
pub struct ScoredHindsightNote {
    pub note: HindsightNote,
    pub relevance_score: f32,
    pub recency_score: f32,
    pub success_score: f32,
    pub combined_score: f32
}

impl ScoredHindsightNote {
    fn new(
        note: HindsightNote,
        relevance_score: f32,
        recency_score: f32,
        success_score: f32,
        config: &HindsightRetrievalConfig
    ) -> Self {
        let relevance_weight = 1.0 - config.recency_weight - config.success_weight;
        let combined_score = relevance_score * relevance_weight
            + recency_score * config.recency_weight
            + success_score * config.success_weight;

        Self {
            note,
            relevance_score,
            recency_score,
            success_score,
            combined_score
        }
    }
}

pub struct HindsightRetriever {
    storage: Arc<PostgresBackend>,
    config: HindsightRetrievalConfig
}

impl HindsightRetriever {
    pub fn new(storage: Arc<PostgresBackend>, config: HindsightRetrievalConfig) -> Self {
        Self { storage, config }
    }

    pub async fn retrieve(
        &self,
        tenant_id: &str,
        error: &ErrorSignature,
        filter: Option<&HindsightRetrievalFilter>
    ) -> Result<Vec<ScoredHindsightNote>, PostgresError> {
        let span = info_span!(
            "retrieve_hindsight_notes",
            tenant_id,
            error_type = %error.error_type,
            has_filter = filter.is_some(),
            max_candidates = self.config.max_candidate_notes
        );

        async move {
            let notes = self
                .storage
                .list_hindsight_notes(tenant_id, self.config.max_candidate_notes as i64, 0)
                .await?;

            Ok(rank_notes(error, &notes, &self.config, filter))
        }
        .instrument(span)
        .await
    }
}

fn rank_notes(
    error: &ErrorSignature,
    notes: &[HindsightNote],
    config: &HindsightRetrievalConfig,
    filter: Option<&HindsightRetrievalFilter>
) -> Vec<ScoredHindsightNote> {
    let now = chrono::Utc::now().timestamp();

    let mut scored: Vec<ScoredHindsightNote> = notes
        .iter()
        .filter(|note| matches_filter(note, config, filter))
        .filter_map(|note| {
            let relevance = relevance_score(error, note, config);
            if relevance < config.relevance_threshold {
                return None;
            }
            let recency = recency_score(note.created_at, now);
            let success = success_score(note);
            Some(ScoredHindsightNote::new(
                note.clone(),
                relevance,
                recency,
                success,
                config
            ))
        })
        .collect();

    scored.sort_by(|a, b| {
        b.combined_score
            .partial_cmp(&a.combined_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    scored.truncate(config.max_results);
    scored
}

fn matches_filter(
    note: &HindsightNote,
    config: &HindsightRetrievalConfig,
    filter: Option<&HindsightRetrievalFilter>
) -> bool {
    let Some(filter) = filter else {
        return true;
    };

    if let Some(min_success) = filter.min_success_rate {
        if success_score(note) < min_success {
            return false;
        }
    }

    if let Some(after) = filter.created_after {
        if note.created_at < after {
            return false;
        }
    }

    if let Some(before) = filter.created_before {
        if note.created_at > before {
            return false;
        }
    }

    if config.enable_tag_filtering {
        if let Some(tags) = &filter.tags {
            if !tags.is_empty() {
                let matches = tags.iter().any(|tag| note.tags.contains(tag));
                if !matches {
                    return false;
                }
            }
        }
    }

    true
}

fn relevance_score(
    error: &ErrorSignature,
    note: &HindsightNote,
    config: &HindsightRetrievalConfig
) -> f32 {
    if let (Some(query_vec), Some(note_vec)) = (
        error.embedding.as_ref(),
        note.error_signature.embedding.as_ref()
    ) {
        let sim = cosine_similarity(query_vec, note_vec);
        if sim >= config.semantic_threshold {
            return sim;
        }
    }

    let mut score = 0.0;

    if error.error_type == note.error_signature.error_type {
        score += 0.6;
    }

    let msg_sim = jaccard_similarity(
        &tokenize(&error.message_pattern),
        &tokenize(&note.error_signature.message_pattern)
    );
    score += msg_sim * 0.3;

    let ctx_sim = jaccard_similarity(
        &error.context_patterns,
        &note.error_signature.context_patterns
    );
    score += ctx_sim * 0.1;

    score
}

fn success_score(note: &HindsightNote) -> f32 {
    note.resolutions
        .iter()
        .map(|r| r.success_rate)
        .fold(0.0_f32, f32::max)
}

fn recency_score(created_at: i64, now: i64) -> f32 {
    if now <= created_at {
        return 1.0;
    }

    let age_seconds = (now - created_at) as f32;
    let one_week = 604800.0_f32;

    let score = 1.0 - (age_seconds / one_week).min(1.0);
    score.max(0.0)
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

fn tokenize(input: &str) -> Vec<String> {
    input
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_lowercase())
        .collect()
}

fn jaccard_similarity(a: &[String], b: &[String]) -> f32 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }

    let a_set: HashSet<_> = a.iter().collect();
    let b_set: HashSet<_> = b.iter().collect();

    let intersection = a_set.intersection(&b_set).count() as f32;
    let union = a_set.union(&b_set).count() as f32;

    if union == 0.0 {
        0.0
    } else {
        intersection / union
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mk_core::types::Resolution;

    fn signature(error_type: &str, message: &str) -> ErrorSignature {
        ErrorSignature {
            error_type: error_type.to_string(),
            message_pattern: message.to_string(),
            stack_patterns: vec![],
            context_patterns: vec!["tool:cargo_test".to_string()],
            embedding: None
        }
    }

    fn note(id: &str, signature: ErrorSignature, created_at: i64, success: f32) -> HindsightNote {
        HindsightNote {
            id: id.to_string(),
            error_signature: signature,
            resolutions: vec![Resolution {
                id: format!("res-{id}"),
                error_signature_id: "err".to_string(),
                description: "fix".to_string(),
                changes: vec![],
                success_rate: success,
                application_count: 1,
                last_success_at: 0
            }],
            content: "content".to_string(),
            tags: vec!["rust".to_string()],
            created_at,
            updated_at: created_at
        }
    }

    #[test]
    fn test_rank_notes_prefers_relevance() {
        let cfg = HindsightRetrievalConfig {
            max_results: 2,
            relevance_threshold: 0.0,
            recency_weight: 0.0,
            success_weight: 0.0,
            ..Default::default()
        };
        let err = signature("TypeError", "cannot read property");
        let notes = vec![
            note("1", signature("TypeError", "cannot read property"), 0, 0.5),
            note("2", signature("Other", "different"), 0, 0.5),
        ];

        let ranked = rank_notes(&err, &notes, &cfg, None);

        assert_eq!(ranked.len(), 2);
        assert_eq!(ranked[0].note.id, "1");
    }

    #[test]
    fn test_rank_notes_filters_by_tags() {
        let cfg = HindsightRetrievalConfig {
            max_results: 2,
            relevance_threshold: 0.0,
            ..Default::default()
        };
        let err = signature("TypeError", "cannot read property");
        let note1 = note("1", signature("TypeError", "cannot read property"), 0, 0.5);
        let mut note2 = note("2", signature("TypeError", "cannot read property"), 0, 0.5);
        note2.tags = vec!["python".to_string()];
        let filter = HindsightRetrievalFilter::default().with_tags(vec!["rust".to_string()]);

        let ranked = rank_notes(&err, &[note1, note2], &cfg, Some(&filter));

        assert_eq!(ranked.len(), 1);
        assert_eq!(ranked[0].note.id, "1");
    }

    #[test]
    fn test_rank_notes_filters_by_success_rate() {
        let cfg = HindsightRetrievalConfig {
            max_results: 5,
            relevance_threshold: 0.0,
            ..Default::default()
        };
        let err = signature("TypeError", "cannot read property");
        let notes = vec![
            note("1", signature("TypeError", "cannot read property"), 0, 0.3),
            note("2", signature("TypeError", "cannot read property"), 0, 0.9),
        ];
        let filter = HindsightRetrievalFilter::default().with_min_success_rate(0.5);

        let ranked = rank_notes(&err, &notes, &cfg, Some(&filter));

        assert_eq!(ranked.len(), 1);
        assert_eq!(ranked[0].note.id, "2");
    }

    #[test]
    fn test_rank_notes_recency_weight() {
        let cfg = HindsightRetrievalConfig {
            max_results: 2,
            relevance_threshold: 0.0,
            recency_weight: 0.5,
            success_weight: 0.0,
            ..Default::default()
        };
        let err = signature("TypeError", "cannot read property");
        let now = chrono::Utc::now().timestamp();
        let notes = vec![
            note(
                "1",
                signature("TypeError", "cannot read property"),
                now - 604800,
                0.5
            ),
            note(
                "2",
                signature("TypeError", "cannot read property"),
                now,
                0.5
            ),
        ];

        let ranked = rank_notes(&err, &notes, &cfg, None);

        assert_eq!(ranked[0].note.id, "2");
        assert!(ranked[0].recency_score > ranked[1].recency_score);
    }

    #[test]
    fn test_rank_notes_success_weight() {
        let cfg = HindsightRetrievalConfig {
            max_results: 2,
            relevance_threshold: 0.0,
            recency_weight: 0.0,
            success_weight: 0.5,
            ..Default::default()
        };
        let err = signature("TypeError", "cannot read property");
        let notes = vec![
            note("1", signature("TypeError", "cannot read property"), 0, 0.2),
            note("2", signature("TypeError", "cannot read property"), 0, 0.9),
        ];

        let ranked = rank_notes(&err, &notes, &cfg, None);

        assert_eq!(ranked[0].note.id, "2");
        assert!(ranked[0].success_score > ranked[1].success_score);
    }

    #[test]
    fn test_cosine_similarity_valid() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 0.001);
    }
}
