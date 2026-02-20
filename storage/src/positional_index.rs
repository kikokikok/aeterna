use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorPointer {
    pub vector_id: String,
    pub position: u32,
    pub weight: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryPointers {
    pub summary_id: String,
    pub level: u32,
    pub pointers: Vec<VectorPointer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionalIndex {
    summary_to_vectors: HashMap<String, SummaryPointers>,
    vector_to_summaries: HashMap<String, HashSet<String>>,
}

impl PositionalIndex {
    pub fn new() -> Self {
        Self {
            summary_to_vectors: HashMap::new(),
            vector_to_summaries: HashMap::new(),
        }
    }

    pub fn register_summary(&mut self, summary_id: &str, level: u32, member_node_ids: &[String]) {
        let pointers: Vec<VectorPointer> = member_node_ids
            .iter()
            .enumerate()
            .map(|(pos, node_id)| VectorPointer {
                vector_id: node_id.clone(),
                position: pos as u32,
                weight: 1.0,
            })
            .collect();

        for node_id in member_node_ids {
            self.vector_to_summaries
                .entry(node_id.clone())
                .or_default()
                .insert(summary_id.to_string());
        }

        self.summary_to_vectors.insert(
            summary_id.to_string(),
            SummaryPointers {
                summary_id: summary_id.to_string(),
                level,
                pointers,
            },
        );
    }

    pub fn register_weighted(
        &mut self,
        summary_id: &str,
        level: u32,
        weighted_members: &[(String, f64)],
    ) {
        let pointers: Vec<VectorPointer> = weighted_members
            .iter()
            .enumerate()
            .map(|(pos, (node_id, weight))| VectorPointer {
                vector_id: node_id.clone(),
                position: pos as u32,
                weight: *weight,
            })
            .collect();

        for (node_id, _) in weighted_members {
            self.vector_to_summaries
                .entry(node_id.clone())
                .or_default()
                .insert(summary_id.to_string());
        }

        self.summary_to_vectors.insert(
            summary_id.to_string(),
            SummaryPointers {
                summary_id: summary_id.to_string(),
                level,
                pointers,
            },
        );
    }

    pub fn get_vectors_for_summary(&self, summary_id: &str) -> Option<&SummaryPointers> {
        self.summary_to_vectors.get(summary_id)
    }

    pub fn get_summaries_for_vector(&self, vector_id: &str) -> Vec<&str> {
        self.vector_to_summaries
            .get(vector_id)
            .map(|ids| ids.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }

    pub fn drill_down(&self, summary_id: &str, top_k: usize) -> Vec<&VectorPointer> {
        let Some(entry) = self.summary_to_vectors.get(summary_id) else {
            return Vec::new();
        };

        let mut sorted: Vec<&VectorPointer> = entry.pointers.iter().collect();
        sorted.sort_by(|a, b| {
            b.weight
                .partial_cmp(&a.weight)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        sorted.truncate(top_k);
        sorted
    }

    pub fn update_weight(&mut self, summary_id: &str, vector_id: &str, new_weight: f64) -> bool {
        if let Some(entry) = self.summary_to_vectors.get_mut(summary_id) {
            for pointer in &mut entry.pointers {
                if pointer.vector_id == vector_id {
                    pointer.weight = new_weight;
                    return true;
                }
            }
        }
        false
    }

    pub fn remove_summary(&mut self, summary_id: &str) -> bool {
        if let Some(entry) = self.summary_to_vectors.remove(summary_id) {
            for pointer in &entry.pointers {
                if let Some(set) = self.vector_to_summaries.get_mut(&pointer.vector_id) {
                    set.remove(summary_id);
                    if set.is_empty() {
                        self.vector_to_summaries.remove(&pointer.vector_id);
                    }
                }
            }
            true
        } else {
            false
        }
    }

    pub fn remove_vector(&mut self, vector_id: &str) -> Vec<String> {
        let affected_summaries: Vec<String> = self
            .vector_to_summaries
            .remove(vector_id)
            .unwrap_or_default()
            .into_iter()
            .collect();

        for summary_id in &affected_summaries {
            if let Some(entry) = self.summary_to_vectors.get_mut(summary_id) {
                entry.pointers.retain(|p| p.vector_id != vector_id);
            }
        }

        affected_summaries
    }

    pub fn summary_count(&self) -> usize {
        self.summary_to_vectors.len()
    }

    pub fn vector_count(&self) -> usize {
        self.vector_to_summaries.len()
    }

    pub fn build_from_hierarchical_index(&mut self, index: &crate::graphrag::HierarchicalIndex) {
        for level in 0..=index.max_level {
            for summary in index.get_level(level) {
                self.register_summary(
                    &summary.community_id,
                    summary.level,
                    &summary.member_node_ids,
                );
            }
        }
    }
}

impl Default for PositionalIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_members() -> Vec<String> {
        vec![
            "vec_001".to_string(),
            "vec_002".to_string(),
            "vec_003".to_string(),
        ]
    }

    #[test]
    fn register_and_lookup_summary() {
        let mut idx = PositionalIndex::new();
        let members = sample_members();
        idx.register_summary("community_a", 0, &members);

        let entry = idx.get_vectors_for_summary("community_a").unwrap();
        assert_eq!(entry.pointers.len(), 3);
        assert_eq!(entry.level, 0);
        assert_eq!(entry.pointers[0].vector_id, "vec_001");
        assert_eq!(entry.pointers[0].position, 0);
    }

    #[test]
    fn reverse_lookup_vector_to_summaries() {
        let mut idx = PositionalIndex::new();
        idx.register_summary("community_a", 0, &sample_members());
        idx.register_summary(
            "community_b",
            0,
            &["vec_002".to_string(), "vec_004".to_string()],
        );

        let summaries = idx.get_summaries_for_vector("vec_002");
        assert_eq!(summaries.len(), 2);
        assert!(summaries.contains(&"community_a"));
        assert!(summaries.contains(&"community_b"));

        let single = idx.get_summaries_for_vector("vec_001");
        assert_eq!(single.len(), 1);
        assert!(single.contains(&"community_a"));
    }

    #[test]
    fn drill_down_returns_top_k_by_weight() {
        let mut idx = PositionalIndex::new();
        idx.register_weighted(
            "community_w",
            1,
            &[
                ("vec_a".to_string(), 0.9),
                ("vec_b".to_string(), 0.3),
                ("vec_c".to_string(), 0.7),
                ("vec_d".to_string(), 0.1),
            ],
        );

        let top = idx.drill_down("community_w", 2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].vector_id, "vec_a");
        assert_eq!(top[1].vector_id, "vec_c");
    }

    #[test]
    fn drill_down_missing_summary_returns_empty() {
        let idx = PositionalIndex::new();
        let result = idx.drill_down("nonexistent", 5);
        assert!(result.is_empty());
    }

    #[test]
    fn update_weight() {
        let mut idx = PositionalIndex::new();
        idx.register_summary("s1", 0, &sample_members());

        assert!(idx.update_weight("s1", "vec_002", 5.0));
        let entry = idx.get_vectors_for_summary("s1").unwrap();
        assert!((entry.pointers[1].weight - 5.0).abs() < f64::EPSILON);

        assert!(!idx.update_weight("s1", "nonexistent", 1.0));
        assert!(!idx.update_weight("missing_summary", "vec_001", 1.0));
    }

    #[test]
    fn remove_summary_cleans_both_maps() {
        let mut idx = PositionalIndex::new();
        idx.register_summary("s1", 0, &sample_members());

        assert!(idx.remove_summary("s1"));
        assert!(idx.get_vectors_for_summary("s1").is_none());
        assert!(idx.get_summaries_for_vector("vec_001").is_empty());
        assert_eq!(idx.summary_count(), 0);
        assert_eq!(idx.vector_count(), 0);
    }

    #[test]
    fn remove_vector_cleans_from_summaries() {
        let mut idx = PositionalIndex::new();
        idx.register_summary("s1", 0, &sample_members());

        let affected = idx.remove_vector("vec_002");
        assert_eq!(affected, vec!["s1"]);

        let entry = idx.get_vectors_for_summary("s1").unwrap();
        assert_eq!(entry.pointers.len(), 2);
        assert!(entry.pointers.iter().all(|p| p.vector_id != "vec_002"));
    }

    #[test]
    fn counts() {
        let mut idx = PositionalIndex::new();
        assert_eq!(idx.summary_count(), 0);
        assert_eq!(idx.vector_count(), 0);

        idx.register_summary("s1", 0, &sample_members());
        assert_eq!(idx.summary_count(), 1);
        assert_eq!(idx.vector_count(), 3);

        idx.register_summary("s2", 1, &["vec_003".to_string(), "vec_004".to_string()]);
        assert_eq!(idx.summary_count(), 2);
        assert_eq!(idx.vector_count(), 4);
    }

    #[test]
    fn default_trait() {
        let idx = PositionalIndex::default();
        assert_eq!(idx.summary_count(), 0);
    }
}
