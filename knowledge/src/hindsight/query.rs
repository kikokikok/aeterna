use mk_core::types::{ErrorSignature, HindsightNote, Resolution};

#[derive(Debug, Clone)]
pub struct HindsightQueryConfig {
    pub semantic_threshold: f32,
    pub max_results: usize
}

impl Default for HindsightQueryConfig {
    fn default() -> Self {
        Self {
            semantic_threshold: 0.8,
            max_results: 5
        }
    }
}

#[derive(Debug, Clone)]
pub struct HindsightQuery {
    cfg: HindsightQueryConfig
}

impl HindsightQuery {
    pub fn new(cfg: HindsightQueryConfig) -> Self {
        Self { cfg }
    }

    pub fn query_hindsight(
        &self,
        error: &ErrorSignature,
        notes: &[HindsightNote]
    ) -> Vec<HindsightMatch> {
        let mut matches: Vec<HindsightMatch> = notes
            .iter()
            .filter_map(|note| self.match_note(error, note))
            .collect();

        matches.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        matches.truncate(self.cfg.max_results);
        matches
    }

    fn match_note(&self, err: &ErrorSignature, note: &HindsightNote) -> Option<HindsightMatch> {
        let mut score = 0.0;

        if err.error_type == note.error_signature.error_type {
            score += 0.6;
        }

        let msg_sim = jaccard_similarity(
            &tokenize(&err.message_pattern),
            &tokenize(&note.error_signature.message_pattern)
        );
        score += msg_sim * 0.3;

        let ctx_sim = jaccard_similarity(
            &note.error_signature.context_patterns,
            &err.context_patterns
        );
        score += ctx_sim * 0.1;

        if score < 0.01 {
            return None;
        }

        Some(HindsightMatch {
            note_id: note.id.clone(),
            score,
            note: note.clone(),
            best_resolution: select_best_resolution(&note.resolutions)
        })
    }
}

#[derive(Debug, Clone)]
pub struct HindsightMatch {
    pub note_id: String,
    pub score: f32,
    pub note: HindsightNote,
    pub best_resolution: Option<Resolution>
}

fn select_best_resolution(resolutions: &[Resolution]) -> Option<Resolution> {
    resolutions.iter().cloned().max_by(|a, b| {
        let a_key = (a.success_rate, a.application_count as f32);
        let b_key = (b.success_rate, b.application_count as f32);
        a_key
            .partial_cmp(&b_key)
            .unwrap_or(std::cmp::Ordering::Equal)
    })
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

    let a_set: std::collections::HashSet<_> = a.iter().collect();
    let b_set: std::collections::HashSet<_> = b.iter().collect();

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

    fn sig(error_type: &str, msg: &str) -> ErrorSignature {
        ErrorSignature {
            error_type: error_type.to_string(),
            message_pattern: msg.to_string(),
            stack_patterns: vec![],
            context_patterns: vec!["tool:cargo_test".to_string()],
            embedding: None
        }
    }

    fn note(id: &str, sig: ErrorSignature, resolutions: Vec<Resolution>) -> HindsightNote {
        let now = chrono::Utc::now().timestamp();
        HindsightNote {
            id: id.to_string(),
            error_signature: sig,
            resolutions,
            content: "content".to_string(),
            tags: vec!["tag".to_string()],
            created_at: now,
            updated_at: now
        }
    }

    fn resolution(id: &str, sig_id: &str, success: f32, count: u32) -> Resolution {
        Resolution {
            id: id.to_string(),
            error_signature_id: sig_id.to_string(),
            description: "desc".to_string(),
            changes: vec![],
            success_rate: success,
            application_count: count,
            last_success_at: 0
        }
    }

    #[test]
    fn test_query_prefers_same_error_type() {
        let query = HindsightQuery::new(HindsightQueryConfig::default());
        let err = sig("TypeError", "cannot read property");

        let n1 = note("1", sig("TypeError", "cannot read property"), vec![]);
        let n2 = note("2", sig("OtherError", "cannot read property"), vec![]);

        let matches = query.query_hindsight(&err, &[n1, n2]);

        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].note_id, "1");
    }

    #[test]
    fn test_match_filters_unrelated() {
        let query = HindsightQuery::new(HindsightQueryConfig::default());
        let err = sig("TypeError", "foo bar baz");
        let unrelated = note("1", sig("Other", "completely different"), vec![]);

        let matches = query.query_hindsight(&err, &[unrelated]);

        assert_eq!(matches.len(), 1);
        assert!(matches[0].score > 0.0);
    }

    #[test]
    fn test_select_best_resolution() {
        let r1 = resolution("r1", "e", 0.9, 1);
        let r2 = resolution("r2", "e", 0.8, 100);

        let best = select_best_resolution(&[r1.clone(), r2.clone()]).unwrap();
        assert_eq!(best.id, "r1");
    }

    #[test]
    fn test_tokenize_and_jaccard() {
        let a = tokenize("Foo bar");
        let b = tokenize("foo baz");

        let sim = jaccard_similarity(&a, &b);

        assert!(sim > 0.0);
        assert!(sim < 1.0);
    }

    #[test]
    fn test_query_max_results() {
        let cfg = HindsightQueryConfig {
            max_results: 2,
            ..Default::default()
        };
        let query = HindsightQuery::new(cfg);
        let err = sig("E", "m");

        let notes = vec![
            note("1", sig("E", "m"), vec![]),
            note("2", sig("E", "m"), vec![]),
            note("3", sig("E", "m"), vec![]),
        ];

        let matches = query.query_hindsight(&err, &notes);

        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn test_best_resolution_in_match() {
        let query = HindsightQuery::new(HindsightQueryConfig::default());
        let err = sig("E", "m");
        let r1 = resolution("r1", "e", 0.9, 1);
        let r2 = resolution("r2", "e", 0.95, 10);
        let n = note("1", sig("E", "m"), vec![r1.clone(), r2.clone()]);

        let matches = query.query_hindsight(&err, &[n]);

        assert_eq!(matches.len(), 1);
        assert!(matches[0].best_resolution.is_some());
        assert_eq!(matches[0].best_resolution.clone().unwrap().id, "r2");
    }
}
