use std::collections::HashMap;

use super::generator::GeneratedNote;

#[derive(Debug, Clone)]
pub struct RetrievalConfig {
    pub default_limit: usize,
    pub relevance_threshold: f32,
    pub recency_weight: f32,
    pub success_weight: f32,
    pub enable_tag_filtering: bool,
}

impl Default for RetrievalConfig {
    fn default() -> Self {
        Self {
            default_limit: 10,
            relevance_threshold: 0.5,
            recency_weight: 0.2,
            success_weight: 0.1,
            enable_tag_filtering: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RetrievalFilter {
    pub tags: Option<Vec<String>>,
    pub min_quality_score: Option<f32>,
    pub created_after: Option<u64>,
    pub created_before: Option<u64>,
}

impl Default for RetrievalFilter {
    fn default() -> Self {
        Self {
            tags: None,
            min_quality_score: None,
            created_after: None,
            created_before: None,
        }
    }
}

impl RetrievalFilter {
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = Some(tags);
        self
    }

    pub fn with_min_quality(mut self, score: f32) -> Self {
        self.min_quality_score = Some(score);
        self
    }

    pub fn with_recency(mut self, after: u64) -> Self {
        self.created_after = Some(after);
        self
    }

    pub fn created_between(mut self, after: u64, before: u64) -> Self {
        self.created_after = Some(after);
        self.created_before = Some(before);
        self
    }
}

#[derive(Debug, Clone)]
pub struct ScoredNote {
    pub note: GeneratedNote,
    pub relevance_score: f32,
    pub recency_score: f32,
    pub combined_score: f32,
}

impl ScoredNote {
    pub fn new(
        note: GeneratedNote,
        relevance_score: f32,
        recency_score: f32,
        config: &RetrievalConfig,
    ) -> Self {
        let combined_score = Self::compute_combined_score(
            relevance_score,
            recency_score,
            note.quality_score,
            config,
        );

        Self {
            note,
            relevance_score,
            recency_score,
            combined_score,
        }
    }

    fn compute_combined_score(
        relevance: f32,
        recency: f32,
        quality: f32,
        config: &RetrievalConfig,
    ) -> f32 {
        let relevance_weight = 1.0 - config.recency_weight - config.success_weight;
        relevance * relevance_weight
            + recency * config.recency_weight
            + quality * config.success_weight
    }
}

pub struct NoteIndex {
    notes: HashMap<String, (GeneratedNote, Vec<f32>)>,
    config: RetrievalConfig,
}

impl NoteIndex {
    pub fn new(config: RetrievalConfig) -> Self {
        Self {
            notes: HashMap::new(),
            config,
        }
    }

    pub fn add_note(&mut self, note: GeneratedNote, embedding: Vec<f32>) {
        self.notes.insert(note.id.clone(), (note, embedding));
    }

    pub fn remove_note(&mut self, note_id: &str) -> Option<GeneratedNote> {
        self.notes.remove(note_id).map(|(note, _)| note)
    }

    pub fn get_note(&self, note_id: &str) -> Option<&GeneratedNote> {
        self.notes.get(note_id).map(|(note, _)| note)
    }

    pub fn note_count(&self) -> usize {
        self.notes.len()
    }

    pub fn retrieve_relevant(
        &self,
        query_embedding: &[f32],
        limit: usize,
        filter: Option<&RetrievalFilter>,
    ) -> Vec<ScoredNote> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let mut scored: Vec<ScoredNote> = self
            .notes
            .values()
            .filter(|(note, _)| self.matches_filter(note, filter))
            .map(|(note, embedding)| {
                let relevance = cosine_similarity(query_embedding, embedding);
                let recency = self.compute_recency_score(note.created_at, now);
                ScoredNote::new(note.clone(), relevance, recency, &self.config)
            })
            .filter(|scored| scored.relevance_score >= self.config.relevance_threshold)
            .collect();

        scored.sort_by(|a, b| {
            b.combined_score
                .partial_cmp(&a.combined_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        scored.truncate(limit);
        scored
    }

    fn matches_filter(&self, note: &GeneratedNote, filter: Option<&RetrievalFilter>) -> bool {
        let filter = match filter {
            Some(f) => f,
            None => return true,
        };

        if let Some(min_quality) = filter.min_quality_score {
            if note.quality_score < min_quality {
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

        if self.config.enable_tag_filtering {
            if let Some(ref required_tags) = filter.tags {
                if !required_tags.is_empty() {
                    let has_matching_tag = required_tags.iter().any(|t| note.tags.contains(t));
                    if !has_matching_tag {
                        return false;
                    }
                }
            }
        }

        true
    }

    fn compute_recency_score(&self, created_at: u64, now: u64) -> f32 {
        if now <= created_at {
            return 1.0;
        }

        let age_seconds = (now - created_at) as f32;
        let one_week = 604800.0_f32;

        let score = 1.0 - (age_seconds / one_week).min(1.0);
        score.max(0.0)
    }
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

pub struct NoteRetriever<E> {
    index: NoteIndex,
    embedder: E,
}

impl<E: NoteEmbedder> NoteRetriever<E> {
    pub fn new(config: RetrievalConfig, embedder: E) -> Self {
        Self {
            index: NoteIndex::new(config),
            embedder,
        }
    }

    pub async fn add_note(&mut self, note: GeneratedNote) -> Result<(), E::Error> {
        let text = format!("{}\n{}", note.title, note.content);
        let embedding = self.embedder.embed(&text).await?;
        self.index.add_note(note, embedding);
        Ok(())
    }

    pub async fn retrieve_relevant(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ScoredNote>, E::Error> {
        let query_embedding = self.embedder.embed(query).await?;
        Ok(self.index.retrieve_relevant(&query_embedding, limit, None))
    }

    pub async fn retrieve_with_filter(
        &self,
        query: &str,
        limit: usize,
        filter: &RetrievalFilter,
    ) -> Result<Vec<ScoredNote>, E::Error> {
        let query_embedding = self.embedder.embed(query).await?;
        Ok(self
            .index
            .retrieve_relevant(&query_embedding, limit, Some(filter)))
    }

    pub fn note_count(&self) -> usize {
        self.index.note_count()
    }

    pub fn get_note(&self, note_id: &str) -> Option<&GeneratedNote> {
        self.index.get_note(note_id)
    }

    pub fn remove_note(&mut self, note_id: &str) -> Option<GeneratedNote> {
        self.index.remove_note(note_id)
    }
}

#[async_trait::async_trait]
pub trait NoteEmbedder: Send + Sync {
    type Error: std::error::Error + Send + Sync;

    async fn embed(&self, text: &str) -> Result<Vec<f32>, Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_note(id: &str, title: &str, quality: f32, created_at: u64) -> GeneratedNote {
        GeneratedNote {
            id: id.to_string(),
            title: title.to_string(),
            content: format!("Content for {title}"),
            tags: vec!["rust".to_string(), "async".to_string()],
            source_distillation_id: format!("dist-{id}"),
            created_at,
            quality_score: quality,
        }
    }

    fn sample_embedding() -> Vec<f32> {
        vec![0.1, 0.2, 0.3, 0.4, 0.5]
    }

    fn query_embedding_similar() -> Vec<f32> {
        vec![0.1, 0.2, 0.3, 0.4, 0.5]
    }

    fn query_embedding_different() -> Vec<f32> {
        vec![-0.5, -0.4, -0.3, -0.2, -0.1]
    }

    #[test]
    fn test_note_index_add_and_get() {
        let mut index = NoteIndex::new(RetrievalConfig::default());
        let note = sample_note("1", "Test Note", 0.8, 1000);

        index.add_note(note.clone(), sample_embedding());

        assert_eq!(index.note_count(), 1);
        assert!(index.get_note("1").is_some());
        assert_eq!(index.get_note("1").unwrap().title, "Test Note");
    }

    #[test]
    fn test_note_index_remove() {
        let mut index = NoteIndex::new(RetrievalConfig::default());
        let note = sample_note("1", "Test Note", 0.8, 1000);
        index.add_note(note, sample_embedding());

        let removed = index.remove_note("1");

        assert!(removed.is_some());
        assert_eq!(index.note_count(), 0);
        assert!(index.get_note("1").is_none());
    }

    #[test]
    fn test_retrieve_relevant_returns_similar_notes() {
        let config = RetrievalConfig {
            relevance_threshold: 0.0,
            ..Default::default()
        };
        let mut index = NoteIndex::new(config);

        let note = sample_note("1", "Async error handling", 0.8, 1000);
        index.add_note(note, sample_embedding());

        let results = index.retrieve_relevant(&query_embedding_similar(), 10, None);

        assert_eq!(results.len(), 1);
        assert!(results[0].relevance_score > 0.9);
    }

    #[test]
    fn test_retrieve_relevant_filters_by_threshold() {
        let config = RetrievalConfig {
            relevance_threshold: 0.9,
            ..Default::default()
        };
        let mut index = NoteIndex::new(config);

        let note = sample_note("1", "Async error handling", 0.8, 1000);
        index.add_note(note, sample_embedding());

        let results = index.retrieve_relevant(&query_embedding_different(), 10, None);

        assert!(results.is_empty());
    }

    #[test]
    fn test_retrieve_relevant_respects_limit() {
        let config = RetrievalConfig {
            relevance_threshold: 0.0,
            ..Default::default()
        };
        let mut index = NoteIndex::new(config);

        for i in 0..5 {
            let note = sample_note(&format!("{i}"), &format!("Note {i}"), 0.8, 1000);
            index.add_note(note, sample_embedding());
        }

        let results = index.retrieve_relevant(&query_embedding_similar(), 2, None);

        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_retrieve_with_quality_filter() {
        let config = RetrievalConfig {
            relevance_threshold: 0.0,
            ..Default::default()
        };
        let mut index = NoteIndex::new(config);

        index.add_note(
            sample_note("1", "Low quality", 0.3, 1000),
            sample_embedding(),
        );
        index.add_note(
            sample_note("2", "High quality", 0.9, 1000),
            sample_embedding(),
        );

        let filter = RetrievalFilter::default().with_min_quality(0.5);
        let results = index.retrieve_relevant(&query_embedding_similar(), 10, Some(&filter));

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].note.id, "2");
    }

    #[test]
    fn test_retrieve_with_tag_filter() {
        let config = RetrievalConfig {
            relevance_threshold: 0.0,
            enable_tag_filtering: true,
            ..Default::default()
        };
        let mut index = NoteIndex::new(config);

        let mut note1 = sample_note("1", "Rust note", 0.8, 1000);
        note1.tags = vec!["rust".to_string()];

        let mut note2 = sample_note("2", "Python note", 0.8, 1000);
        note2.tags = vec!["python".to_string()];

        index.add_note(note1, sample_embedding());
        index.add_note(note2, sample_embedding());

        let filter = RetrievalFilter::default().with_tags(vec!["python".to_string()]);
        let results = index.retrieve_relevant(&query_embedding_similar(), 10, Some(&filter));

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].note.id, "2");
    }

    #[test]
    fn test_retrieve_with_recency_filter() {
        let config = RetrievalConfig {
            relevance_threshold: 0.0,
            ..Default::default()
        };
        let mut index = NoteIndex::new(config);

        index.add_note(sample_note("1", "Old note", 0.8, 1000), sample_embedding());
        index.add_note(sample_note("2", "New note", 0.8, 5000), sample_embedding());

        let filter = RetrievalFilter::default().with_recency(3000);
        let results = index.retrieve_relevant(&query_embedding_similar(), 10, Some(&filter));

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].note.id, "2");
    }

    #[test]
    fn test_retrieve_with_date_range_filter() {
        let config = RetrievalConfig {
            relevance_threshold: 0.0,
            ..Default::default()
        };
        let mut index = NoteIndex::new(config);

        index.add_note(sample_note("1", "Too old", 0.8, 1000), sample_embedding());
        index.add_note(sample_note("2", "In range", 0.8, 3000), sample_embedding());
        index.add_note(sample_note("3", "Too new", 0.8, 5000), sample_embedding());

        let filter = RetrievalFilter::default().created_between(2000, 4000);
        let results = index.retrieve_relevant(&query_embedding_similar(), 10, Some(&filter));

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].note.id, "2");
    }

    #[test]
    fn test_scoring_includes_recency() {
        let config = RetrievalConfig {
            relevance_threshold: 0.0,
            recency_weight: 0.3,
            success_weight: 0.1,
            ..Default::default()
        };
        let mut index = NoteIndex::new(config);

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        index.add_note(
            sample_note("1", "Old note", 0.8, now - 604800),
            sample_embedding(),
        );
        index.add_note(sample_note("2", "New note", 0.8, now), sample_embedding());

        let results = index.retrieve_relevant(&query_embedding_similar(), 10, None);

        assert_eq!(results.len(), 2);
        assert!(results[0].recency_score > results[1].recency_score);
        assert_eq!(results[0].note.id, "2");
    }

    #[test]
    fn test_scoring_includes_quality() {
        let config = RetrievalConfig {
            relevance_threshold: 0.0,
            recency_weight: 0.0,
            success_weight: 0.5,
            ..Default::default()
        };
        let mut index = NoteIndex::new(config);

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        index.add_note(
            sample_note("1", "Low quality", 0.3, now),
            sample_embedding(),
        );
        index.add_note(
            sample_note("2", "High quality", 0.9, now),
            sample_embedding(),
        );

        let results = index.retrieve_relevant(&query_embedding_similar(), 10, None);

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].note.id, "2");
    }

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];

        let sim = cosine_similarity(&a, &b);

        assert!((sim - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];

        let sim = cosine_similarity(&a, &b);

        assert!(sim.abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![-1.0, 0.0, 0.0];

        let sim = cosine_similarity(&a, &b);

        assert!((sim - (-1.0)).abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_different_lengths() {
        let a = vec![1.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];

        let sim = cosine_similarity(&a, &b);

        assert_eq!(sim, 0.0);
    }

    #[test]
    fn test_cosine_similarity_empty() {
        let a: Vec<f32> = vec![];
        let b: Vec<f32> = vec![];

        let sim = cosine_similarity(&a, &b);

        assert_eq!(sim, 0.0);
    }

    #[test]
    fn test_scored_note_combined_score() {
        let config = RetrievalConfig {
            recency_weight: 0.2,
            success_weight: 0.1,
            ..Default::default()
        };
        let note = sample_note("1", "Test", 0.8, 1000);

        let scored = ScoredNote::new(note, 0.9, 0.5, &config);

        let expected = 0.9 * 0.7 + 0.5 * 0.2 + 0.8 * 0.1;
        assert!((scored.combined_score - expected).abs() < 0.001);
    }

    #[test]
    fn test_retrieval_filter_builder() {
        let filter = RetrievalFilter::default()
            .with_tags(vec!["rust".to_string()])
            .with_min_quality(0.7)
            .with_recency(1000);

        assert_eq!(filter.tags, Some(vec!["rust".to_string()]));
        assert_eq!(filter.min_quality_score, Some(0.7));
        assert_eq!(filter.created_after, Some(1000));
    }

    struct MockEmbedder;

    #[derive(Debug)]
    struct MockError;

    impl std::fmt::Display for MockError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "MockError")
        }
    }

    impl std::error::Error for MockError {}

    #[async_trait::async_trait]
    impl NoteEmbedder for MockEmbedder {
        type Error = MockError;

        async fn embed(&self, _text: &str) -> Result<Vec<f32>, Self::Error> {
            Ok(vec![0.1, 0.2, 0.3, 0.4, 0.5])
        }
    }

    #[tokio::test]
    async fn test_note_retriever_add_and_retrieve() {
        let config = RetrievalConfig {
            relevance_threshold: 0.0,
            ..Default::default()
        };
        let mut retriever = NoteRetriever::new(config, MockEmbedder);

        let note = sample_note("1", "Async error handling", 0.8, 1000);
        retriever.add_note(note).await.unwrap();

        assert_eq!(retriever.note_count(), 1);

        let results = retriever
            .retrieve_relevant("async errors", 10)
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_note_retriever_with_filter() {
        let config = RetrievalConfig {
            relevance_threshold: 0.0,
            ..Default::default()
        };
        let mut retriever = NoteRetriever::new(config, MockEmbedder);

        let note = sample_note("1", "Test", 0.9, 1000);
        retriever.add_note(note).await.unwrap();

        let filter = RetrievalFilter::default().with_min_quality(0.8);
        let results = retriever
            .retrieve_with_filter("test", 10, &filter)
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_note_retriever_get_and_remove() {
        let config = RetrievalConfig::default();
        let mut retriever = NoteRetriever::new(config, MockEmbedder);

        let note = sample_note("1", "Test", 0.8, 1000);
        retriever.add_note(note).await.unwrap();

        assert!(retriever.get_note("1").is_some());

        let removed = retriever.remove_note("1");
        assert!(removed.is_some());
        assert!(retriever.get_note("1").is_none());
    }
}
