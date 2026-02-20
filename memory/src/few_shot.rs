use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum FewShotError {
    #[error("No examples available")]
    NoExamples,
    #[error("Invalid k value: {k} exceeds available examples ({available})")]
    InvalidK { k: usize, available: usize },
    #[error("Embedding dimension mismatch: query has {query_dim}, example has {example_dim}")]
    DimensionMismatch {
        query_dim: usize,
        example_dim: usize,
    },
    #[error("Empty query embedding")]
    EmptyQueryEmbedding,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Example {
    pub input: String,
    pub output: String,
    pub embedding: Vec<f32>,
}

impl Example {
    pub fn new(input: impl Into<String>, output: impl Into<String>, embedding: Vec<f32>) -> Self {
        Self {
            input: input.into(),
            output: output.into(),
            embedding,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredExample {
    pub example: Example,
    pub relevance_score: f32,
    pub diversity_score: f32,
    pub final_score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FewShotConfig {
    pub default_k: usize,
    pub lambda: f32,
    pub input_prefix: String,
    pub output_prefix: String,
    pub separator: String,
}

impl Default for FewShotConfig {
    fn default() -> Self {
        Self {
            default_k: 3,
            lambda: 0.7,
            input_prefix: "Input: ".to_string(),
            output_prefix: "Output: ".to_string(),
            separator: "\n\n".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FewShotSelector {
    examples: Vec<Example>,
    config: FewShotConfig,
}

impl FewShotSelector {
    pub fn new(examples: Vec<Example>, config: FewShotConfig) -> Self {
        Self { examples, config }
    }

    pub fn with_defaults(examples: Vec<Example>) -> Self {
        Self::new(examples, FewShotConfig::default())
    }

    pub fn add_example(&mut self, example: Example) {
        self.examples.push(example);
    }

    pub fn example_count(&self) -> usize {
        self.examples.len()
    }

    pub fn select(
        &self,
        query_embedding: &[f32],
        k: usize,
    ) -> Result<Vec<ScoredExample>, FewShotError> {
        if self.examples.is_empty() {
            return Err(FewShotError::NoExamples);
        }
        if query_embedding.is_empty() {
            return Err(FewShotError::EmptyQueryEmbedding);
        }

        let effective_k = k.min(self.examples.len());

        let mut relevance_scores: Vec<(usize, f32)> = self
            .examples
            .iter()
            .enumerate()
            .map(|(i, ex)| {
                let score = cosine_similarity(query_embedding, &ex.embedding);
                (i, score)
            })
            .collect();

        relevance_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        self.mmr_rerank(query_embedding, &relevance_scores, effective_k)
    }

    pub fn select_default(
        &self,
        query_embedding: &[f32],
    ) -> Result<Vec<ScoredExample>, FewShotError> {
        self.select(query_embedding, self.config.default_k)
    }

    fn mmr_rerank(
        &self,
        _query_embedding: &[f32],
        relevance_scores: &[(usize, f32)],
        k: usize,
    ) -> Result<Vec<ScoredExample>, FewShotError> {
        let lambda = self.config.lambda;
        let mut selected: Vec<ScoredExample> = Vec::with_capacity(k);
        let mut selected_indices: Vec<usize> = Vec::with_capacity(k);
        let mut candidates: Vec<(usize, f32)> = relevance_scores.to_vec();

        for _ in 0..k {
            if candidates.is_empty() {
                break;
            }

            let mut best_mmr = f32::NEG_INFINITY;
            let mut best_candidate_pos = 0;

            for (pos, &(idx, rel_score)) in candidates.iter().enumerate() {
                let max_sim_to_selected = if selected_indices.is_empty() {
                    0.0
                } else {
                    selected_indices
                        .iter()
                        .map(|&sel_idx| {
                            cosine_similarity(
                                &self.examples[idx].embedding,
                                &self.examples[sel_idx].embedding,
                            )
                        })
                        .fold(f32::NEG_INFINITY, f32::max)
                };

                let mmr = lambda * rel_score - (1.0 - lambda) * max_sim_to_selected;

                if mmr > best_mmr {
                    best_mmr = mmr;
                    best_candidate_pos = pos;
                }
            }

            let (chosen_idx, chosen_rel) = candidates.remove(best_candidate_pos);

            let diversity_score = if selected_indices.is_empty() {
                1.0
            } else {
                let avg_sim: f32 = selected_indices
                    .iter()
                    .map(|&sel_idx| {
                        cosine_similarity(
                            &self.examples[chosen_idx].embedding,
                            &self.examples[sel_idx].embedding,
                        )
                    })
                    .sum::<f32>()
                    / selected_indices.len() as f32;
                1.0 - avg_sim
            };

            selected.push(ScoredExample {
                example: self.examples[chosen_idx].clone(),
                relevance_score: chosen_rel,
                diversity_score,
                final_score: best_mmr,
            });
            selected_indices.push(chosen_idx);
        }

        Ok(selected)
    }

    pub fn format_prompt(
        &self,
        examples: &[ScoredExample],
        system_prefix: &str,
        query: &str,
    ) -> String {
        let mut parts = Vec::with_capacity(examples.len() + 2);

        if !system_prefix.is_empty() {
            parts.push(system_prefix.to_string());
        }

        for scored in examples {
            parts.push(format!(
                "{}{}\n{}{}",
                self.config.input_prefix,
                scored.example.input,
                self.config.output_prefix,
                scored.example.output,
            ));
        }

        parts.push(format!("{}{}", self.config.input_prefix, query));

        parts.join(&self.config.separator)
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let len = a.len().min(b.len());
    if len == 0 {
        return 0.0;
    }

    let mut dot = 0.0_f32;
    let mut norm_a = 0.0_f32;
    let mut norm_b = 0.0_f32;

    for i in 0..len {
        dot += a[i] * b[i];
        norm_a += a[i] * a[i];
        norm_b += b[i] * b[i];
    }

    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom < f32::EPSILON {
        return 0.0;
    }

    dot / denom
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_example(input: &str, output: &str, direction: &[f32]) -> Example {
        Example::new(input.to_string(), output.to_string(), direction.to_vec())
    }

    fn sample_examples() -> Vec<Example> {
        vec![
            make_example("What is Rust?", "A systems language", &[1.0, 0.0, 0.0]),
            make_example("What is Python?", "A scripting language", &[0.0, 1.0, 0.0]),
            make_example("What is Go?", "A compiled language", &[0.5, 0.5, 0.0]),
            make_example("What is math?", "Study of numbers", &[0.0, 0.0, 1.0]),
        ]
    }

    #[test]
    fn test_select_returns_k_examples() {
        let selector = FewShotSelector::with_defaults(sample_examples());
        let query = [0.9, 0.1, 0.0];

        let results = selector.select(&query, 2).expect("select should succeed");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_select_most_relevant_first() {
        let selector = FewShotSelector::with_defaults(sample_examples());
        let query = [1.0, 0.0, 0.0];

        let results = selector.select(&query, 1).expect("select should succeed");
        assert_eq!(results[0].example.input, "What is Rust?");
    }

    #[test]
    fn test_mmr_promotes_diversity() {
        let config = FewShotConfig {
            lambda: 0.3,
            ..Default::default()
        };
        let selector = FewShotSelector::new(sample_examples(), config);
        let query = [0.8, 0.2, 0.0];

        let results = selector.select(&query, 3).expect("select should succeed");

        let inputs: Vec<&str> = results.iter().map(|r| r.example.input.as_str()).collect();
        assert!(
            inputs.contains(&"What is math?"),
            "low lambda should promote diverse examples: got {:?}",
            inputs
        );
    }

    #[test]
    fn test_select_empty_examples_errors() {
        let selector = FewShotSelector::with_defaults(vec![]);
        let result = selector.select(&[1.0, 0.0], 3);
        assert!(matches!(result, Err(FewShotError::NoExamples)));
    }

    #[test]
    fn test_select_empty_query_errors() {
        let selector = FewShotSelector::with_defaults(sample_examples());
        let result = selector.select(&[], 3);
        assert!(matches!(result, Err(FewShotError::EmptyQueryEmbedding)));
    }

    #[test]
    fn test_format_prompt_assembles_correctly() {
        let selector = FewShotSelector::with_defaults(sample_examples());
        let query = [1.0, 0.0, 0.0];

        let selected = selector.select(&query, 2).expect("select should succeed");
        let prompt = selector.format_prompt(
            &selected,
            "Answer questions about programming.",
            "What is C++?",
        );

        assert!(prompt.contains("Answer questions about programming."));
        assert!(prompt.contains("Input: What is C++?"));
        assert!(prompt.contains("Output: "));
    }

    #[test]
    fn test_cosine_similarity_identical_vectors() {
        let a = [1.0, 2.0, 3.0];
        let sim = cosine_similarity(&a, &a);
        assert!((sim - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_cosine_similarity_orthogonal_vectors() {
        let a = [1.0, 0.0];
        let b = [0.0, 1.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 1e-5);
    }

    #[test]
    fn test_add_example_increases_count() {
        let mut selector = FewShotSelector::with_defaults(sample_examples());
        let count_before = selector.example_count();
        selector.add_example(make_example("new", "example", &[0.1, 0.1, 0.1]));
        assert_eq!(selector.example_count(), count_before + 1);
    }

    #[test]
    fn test_select_k_larger_than_examples() {
        let selector = FewShotSelector::with_defaults(sample_examples());
        let query = [1.0, 0.0, 0.0];

        let results = selector.select(&query, 100).expect("select should succeed");
        assert_eq!(results.len(), 4, "should return all available examples");
    }
}
