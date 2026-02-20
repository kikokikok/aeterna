use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum MatryoshkaError {
    #[error("Embedding too short: got {got} dimensions, need at least {need}")]
    EmbeddingTooShort { got: usize, need: usize },
    #[error("Empty embedding provided")]
    EmptyEmbedding,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Dimension {
    D256,
    D384,
    D768,
    D1536,
}

impl Dimension {
    pub fn value(self) -> usize {
        match self {
            Self::D256 => 256,
            Self::D384 => 384,
            Self::D768 => 768,
            Self::D1536 => 1536,
        }
    }

    pub fn all_ascending() -> &'static [Dimension] {
        &[Self::D256, Self::D384, Self::D768, Self::D1536]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UseCase {
    FastSearch,
    Balanced,
    HighAccuracy,
    Storage,
}

impl UseCase {
    pub fn description(self) -> &'static str {
        match self {
            Self::FastSearch => "Low latency search with acceptable accuracy trade-off",
            Self::Balanced => "Balance between speed and accuracy for general use",
            Self::HighAccuracy => "Maximum accuracy for critical retrieval tasks",
            Self::Storage => "Minimal dimensions for long-term archival storage",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingResult {
    pub embedding: Vec<f32>,
    pub dimension: Dimension,
    pub original_dimension: usize,
    pub normalized: bool,
}

#[derive(Debug, Clone)]
pub struct MatryoshkaEmbedder {
    default_dimension: Dimension,
}

impl MatryoshkaEmbedder {
    pub fn new(default_dimension: Dimension) -> Self {
        Self { default_dimension }
    }

    pub fn with_defaults() -> Self {
        Self::new(Dimension::D768)
    }

    pub fn select_dimension(use_case: UseCase) -> Dimension {
        match use_case {
            UseCase::FastSearch => Dimension::D256,
            UseCase::Balanced => Dimension::D768,
            UseCase::HighAccuracy => Dimension::D1536,
            UseCase::Storage => Dimension::D256,
        }
    }

    pub fn embed(
        &self,
        full_embedding: &[f32],
        dimension: Dimension,
    ) -> Result<EmbeddingResult, MatryoshkaError> {
        if full_embedding.is_empty() {
            return Err(MatryoshkaError::EmptyEmbedding);
        }

        let target = dimension.value();
        if full_embedding.len() < target {
            return Err(MatryoshkaError::EmbeddingTooShort {
                got: full_embedding.len(),
                need: target,
            });
        }

        let truncated: Vec<f32> = full_embedding[..target].to_vec();
        let normalized = Self::l2_normalize(&truncated);

        Ok(EmbeddingResult {
            embedding: normalized,
            dimension,
            original_dimension: full_embedding.len(),
            normalized: true,
        })
    }

    pub fn embed_default(
        &self,
        full_embedding: &[f32],
    ) -> Result<EmbeddingResult, MatryoshkaError> {
        self.embed(full_embedding, self.default_dimension)
    }

    pub fn embed_for_use_case(
        &self,
        full_embedding: &[f32],
        use_case: UseCase,
    ) -> Result<EmbeddingResult, MatryoshkaError> {
        let dimension = Self::select_dimension(use_case);
        self.embed(full_embedding, dimension)
    }

    pub fn embed_multi(
        &self,
        full_embedding: &[f32],
        dimensions: &[Dimension],
    ) -> Result<Vec<EmbeddingResult>, MatryoshkaError> {
        dimensions
            .iter()
            .map(|&dim| self.embed(full_embedding, dim))
            .collect()
    }

    fn l2_normalize(vec: &[f32]) -> Vec<f32> {
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm < f32::EPSILON {
            return vec.to_vec();
        }
        vec.iter().map(|x| x / norm).collect()
    }
}

impl Default for MatryoshkaEmbedder {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_embedding(dim: usize) -> Vec<f32> {
        (0..dim).map(|i| (i as f32 + 1.0) * 0.01).collect()
    }

    #[test]
    fn test_embed_truncates_to_correct_dimension() {
        let embedder = MatryoshkaEmbedder::with_defaults();
        let full = make_embedding(1536);

        let result = embedder
            .embed(&full, Dimension::D256)
            .expect("embed should succeed");

        assert_eq!(result.embedding.len(), 256);
        assert_eq!(result.dimension, Dimension::D256);
        assert_eq!(result.original_dimension, 1536);
        assert!(result.normalized);
    }

    #[test]
    fn test_embed_normalizes_output() {
        let embedder = MatryoshkaEmbedder::with_defaults();
        let full = make_embedding(1536);

        let result = embedder
            .embed(&full, Dimension::D384)
            .expect("embed should succeed");

        let l2: f32 = result.embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            (l2 - 1.0).abs() < 1e-5,
            "output should be L2-normalized, got norm = {l2}"
        );
    }

    #[test]
    fn test_embed_too_short_errors() {
        let embedder = MatryoshkaEmbedder::with_defaults();
        let short = make_embedding(100);

        let result = embedder.embed(&short, Dimension::D256);
        assert!(matches!(
            result,
            Err(MatryoshkaError::EmbeddingTooShort {
                got: 100,
                need: 256
            })
        ));
    }

    #[test]
    fn test_embed_empty_errors() {
        let embedder = MatryoshkaEmbedder::with_defaults();
        let result = embedder.embed(&[], Dimension::D256);
        assert!(matches!(result, Err(MatryoshkaError::EmptyEmbedding)));
    }

    #[test]
    fn test_select_dimension_mapping() {
        assert_eq!(
            MatryoshkaEmbedder::select_dimension(UseCase::FastSearch),
            Dimension::D256
        );
        assert_eq!(
            MatryoshkaEmbedder::select_dimension(UseCase::Balanced),
            Dimension::D768
        );
        assert_eq!(
            MatryoshkaEmbedder::select_dimension(UseCase::HighAccuracy),
            Dimension::D1536
        );
        assert_eq!(
            MatryoshkaEmbedder::select_dimension(UseCase::Storage),
            Dimension::D256
        );
    }

    #[test]
    fn test_embed_for_use_case() {
        let embedder = MatryoshkaEmbedder::with_defaults();
        let full = make_embedding(1536);

        let result = embedder
            .embed_for_use_case(&full, UseCase::HighAccuracy)
            .expect("embed should succeed");

        assert_eq!(result.embedding.len(), 1536);
        assert_eq!(result.dimension, Dimension::D1536);
    }

    #[test]
    fn test_embed_multi_produces_all_dimensions() {
        let embedder = MatryoshkaEmbedder::with_defaults();
        let full = make_embedding(1536);

        let results = embedder
            .embed_multi(&full, Dimension::all_ascending())
            .expect("embed_multi should succeed");

        assert_eq!(results.len(), 4);
        assert_eq!(results[0].embedding.len(), 256);
        assert_eq!(results[1].embedding.len(), 384);
        assert_eq!(results[2].embedding.len(), 768);
        assert_eq!(results[3].embedding.len(), 1536);
    }

    #[test]
    fn test_dimension_values() {
        assert_eq!(Dimension::D256.value(), 256);
        assert_eq!(Dimension::D384.value(), 384);
        assert_eq!(Dimension::D768.value(), 768);
        assert_eq!(Dimension::D1536.value(), 1536);
    }

    #[test]
    fn test_default_embedder() {
        let embedder = MatryoshkaEmbedder::default();
        let full = make_embedding(1536);

        let result = embedder
            .embed_default(&full)
            .expect("embed_default should succeed");

        assert_eq!(result.dimension, Dimension::D768);
        assert_eq!(result.embedding.len(), 768);
    }
}
