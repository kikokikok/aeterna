use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, instrument};

use crate::graph_duckdb::Community;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunitySummary {
    pub community_id: String,
    pub level: u32,
    pub title: String,
    pub summary: String,
    pub member_node_ids: Vec<String>,
    pub child_summary_ids: Vec<String>,
    pub findings: Vec<Finding>,
    pub modularity: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub description: String,
    pub relevance_score: f64,
    pub source_node_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HierarchicalIndex {
    pub levels: HashMap<u32, Vec<CommunitySummary>>,
    pub total_communities: usize,
    pub max_level: u32,
}

impl HierarchicalIndex {
    pub fn new() -> Self {
        Self {
            levels: HashMap::new(),
            total_communities: 0,
            max_level: 0,
        }
    }

    pub fn add_summary(&mut self, summary: CommunitySummary) {
        let level = summary.level;
        self.levels.entry(level).or_default().push(summary);
        self.total_communities += 1;
        if level > self.max_level {
            self.max_level = level;
        }
    }

    pub fn get_level(&self, level: u32) -> &[CommunitySummary] {
        self.levels.get(&level).map(|v| v.as_slice()).unwrap_or(&[])
    }

    pub fn search(&self, query: &str, max_results: usize) -> Vec<&CommunitySummary> {
        let query_lower = query.to_lowercase();
        let mut results: Vec<(&CommunitySummary, usize)> = Vec::new();

        for summaries in self.levels.values() {
            for summary in summaries {
                let mut score = 0usize;
                if summary.title.to_lowercase().contains(&query_lower) {
                    score += 3;
                }
                if summary.summary.to_lowercase().contains(&query_lower) {
                    score += 1;
                }
                for finding in &summary.findings {
                    if finding.description.to_lowercase().contains(&query_lower) {
                        score += 2;
                    }
                }
                if score > 0 {
                    results.push((summary, score));
                }
            }
        }

        results.sort_by(|a, b| b.1.cmp(&a.1));
        results
            .into_iter()
            .take(max_results)
            .map(|(s, _)| s)
            .collect()
    }
}

impl Default for HierarchicalIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
pub trait SummarizationProvider: Send + Sync {
    async fn summarize_community(
        &self,
        node_descriptions: &[String],
        edge_descriptions: &[String],
    ) -> Result<(String, String, Vec<Finding>), GraphRagError>;

    async fn summarize_subcommunities(
        &self,
        child_summaries: &[&CommunitySummary],
    ) -> Result<(String, String, Vec<Finding>), GraphRagError>;
}

#[derive(Debug, thiserror::Error)]
pub enum GraphRagError {
    #[error("Summarization failed: {0}")]
    SummarizationFailed(String),
    #[error("Community not found: {0}")]
    CommunityNotFound(String),
    #[error("Provider error: {0}")]
    ProviderError(String),
}

pub struct GraphRagEngine<S: SummarizationProvider> {
    provider: Arc<S>,
    max_hierarchy_depth: u32,
    min_subcommunity_size: usize,
}

impl<S: SummarizationProvider> GraphRagEngine<S> {
    pub fn new(provider: Arc<S>, max_hierarchy_depth: u32, min_subcommunity_size: usize) -> Self {
        Self {
            provider,
            max_hierarchy_depth,
            min_subcommunity_size,
        }
    }

    #[instrument(skip(self, communities, node_descriptions))]
    pub async fn build_hierarchical_index(
        &self,
        communities: &[Community],
        node_descriptions: &HashMap<String, String>,
        edge_descriptions: &HashMap<(String, String), String>,
    ) -> Result<HierarchicalIndex, GraphRagError> {
        let mut index = HierarchicalIndex::new();

        let level_0_summaries = self
            .summarize_leaf_communities(communities, node_descriptions, edge_descriptions)
            .await?;

        for summary in &level_0_summaries {
            index.add_summary(summary.clone());
        }

        if self.max_hierarchy_depth > 0 {
            self.build_upper_levels(&mut index, &level_0_summaries, 1)
                .await?;
        }

        debug!(
            "Built hierarchical index: {} communities across {} levels",
            index.total_communities,
            index.max_level + 1
        );
        Ok(index)
    }

    async fn summarize_leaf_communities(
        &self,
        communities: &[Community],
        node_descriptions: &HashMap<String, String>,
        edge_descriptions: &HashMap<(String, String), String>,
    ) -> Result<Vec<CommunitySummary>, GraphRagError> {
        let mut summaries = Vec::with_capacity(communities.len());

        for community in communities {
            let node_descs: Vec<String> = community
                .member_node_ids
                .iter()
                .filter_map(|nid| node_descriptions.get(nid).cloned())
                .collect();

            let edge_descs: Vec<String> = community
                .member_node_ids
                .iter()
                .flat_map(|src| {
                    community.member_node_ids.iter().filter_map(move |tgt| {
                        edge_descriptions.get(&(src.clone(), tgt.clone())).cloned()
                    })
                })
                .collect();

            let (title, summary_text, findings) = self
                .provider
                .summarize_community(&node_descs, &edge_descs)
                .await?;

            summaries.push(CommunitySummary {
                community_id: community.id.clone(),
                level: 0,
                title,
                summary: summary_text,
                member_node_ids: community.member_node_ids.clone(),
                child_summary_ids: vec![],
                findings,
                modularity: community.modularity,
            });
        }

        Ok(summaries)
    }

    async fn build_upper_levels(
        &self,
        index: &mut HierarchicalIndex,
        child_summaries: &[CommunitySummary],
        current_level: u32,
    ) -> Result<(), GraphRagError> {
        if current_level > self.max_hierarchy_depth || child_summaries.len() <= 1 {
            return Ok(());
        }

        let groups = self.group_summaries_for_aggregation(child_summaries);

        let mut level_summaries = Vec::new();

        for group in &groups {
            if group.len() < self.min_subcommunity_size {
                continue;
            }

            let child_refs: Vec<&CommunitySummary> = group.iter().copied().collect();
            let (title, summary_text, findings) =
                self.provider.summarize_subcommunities(&child_refs).await?;

            let all_member_ids: Vec<String> = group
                .iter()
                .flat_map(|s| s.member_node_ids.clone())
                .collect();

            let child_ids: Vec<String> = group.iter().map(|s| s.community_id.clone()).collect();

            let avg_modularity =
                group.iter().map(|s| s.modularity).sum::<f64>() / group.len() as f64;

            let parent_summary = CommunitySummary {
                community_id: uuid::Uuid::new_v4().to_string(),
                level: current_level,
                title,
                summary: summary_text,
                member_node_ids: all_member_ids,
                child_summary_ids: child_ids,
                findings,
                modularity: avg_modularity,
            };

            level_summaries.push(parent_summary);
        }

        for summary in &level_summaries {
            index.add_summary(summary.clone());
        }

        if level_summaries.len() > 1 {
            Box::pin(self.build_upper_levels(index, &level_summaries, current_level + 1)).await?;
        }

        Ok(())
    }

    fn group_summaries_for_aggregation<'a>(
        &self,
        summaries: &'a [CommunitySummary],
    ) -> Vec<Vec<&'a CommunitySummary>> {
        let target_group_size = 3.max(summaries.len() / 3).min(summaries.len());
        let mut groups: Vec<Vec<&CommunitySummary>> = Vec::new();
        let mut current_group: Vec<&CommunitySummary> = Vec::new();

        for summary in summaries {
            current_group.push(summary);
            if current_group.len() >= target_group_size {
                groups.push(current_group);
                current_group = Vec::new();
            }
        }

        if !current_group.is_empty() {
            if groups.is_empty() {
                groups.push(current_group);
            } else {
                groups.last_mut().unwrap().extend(current_group);
            }
        }

        groups
    }
}

pub struct ExtractiveProvider;

impl ExtractiveProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ExtractiveProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SummarizationProvider for ExtractiveProvider {
    async fn summarize_community(
        &self,
        node_descriptions: &[String],
        _edge_descriptions: &[String],
    ) -> Result<(String, String, Vec<Finding>), GraphRagError> {
        let title = if let Some(first) = node_descriptions.first() {
            let truncated: String = first.chars().take(80).collect();
            format!("Community: {}", truncated)
        } else {
            "Empty Community".to_string()
        };

        let summary = node_descriptions.join("; ");

        let findings: Vec<Finding> = node_descriptions
            .iter()
            .enumerate()
            .map(|(i, desc)| Finding {
                description: desc.clone(),
                relevance_score: 1.0 / (i as f64 + 1.0),
                source_node_ids: vec![],
            })
            .collect();

        Ok((title, summary, findings))
    }

    async fn summarize_subcommunities(
        &self,
        child_summaries: &[&CommunitySummary],
    ) -> Result<(String, String, Vec<Finding>), GraphRagError> {
        let title = format!("Aggregate of {} subcommunities", child_summaries.len());

        let summary = child_summaries
            .iter()
            .map(|s| s.summary.as_str())
            .collect::<Vec<_>>()
            .join(" | ");

        let findings: Vec<Finding> = child_summaries
            .iter()
            .flat_map(|s| s.findings.clone())
            .collect();

        Ok((title, summary, findings))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hierarchical_index_new() {
        let index = HierarchicalIndex::new();
        assert_eq!(index.total_communities, 0);
        assert_eq!(index.max_level, 0);
        assert!(index.levels.is_empty());
    }

    #[test]
    fn test_hierarchical_index_add_and_get() {
        let mut index = HierarchicalIndex::new();
        let summary = CommunitySummary {
            community_id: "c1".to_string(),
            level: 0,
            title: "Test Community".to_string(),
            summary: "A test community about payments".to_string(),
            member_node_ids: vec!["n1".to_string(), "n2".to_string()],
            child_summary_ids: vec![],
            findings: vec![Finding {
                description: "Payment processing pattern".to_string(),
                relevance_score: 0.9,
                source_node_ids: vec!["n1".to_string()],
            }],
            modularity: 0.45,
        };

        index.add_summary(summary);
        assert_eq!(index.total_communities, 1);

        let level_0 = index.get_level(0);
        assert_eq!(level_0.len(), 1);
        assert_eq!(level_0[0].title, "Test Community");
        assert_eq!(level_0[0].modularity, 0.45);

        let level_1 = index.get_level(1);
        assert!(level_1.is_empty());
    }

    #[test]
    fn test_hierarchical_index_search() {
        let mut index = HierarchicalIndex::new();

        index.add_summary(CommunitySummary {
            community_id: "c1".to_string(),
            level: 0,
            title: "Payment Processing".to_string(),
            summary: "Handles all payment flows".to_string(),
            member_node_ids: vec!["n1".to_string()],
            child_summary_ids: vec![],
            findings: vec![],
            modularity: 0.5,
        });

        index.add_summary(CommunitySummary {
            community_id: "c2".to_string(),
            level: 0,
            title: "User Authentication".to_string(),
            summary: "Manages auth and login".to_string(),
            member_node_ids: vec!["n2".to_string()],
            child_summary_ids: vec![],
            findings: vec![Finding {
                description: "Payment gateway integration".to_string(),
                relevance_score: 0.7,
                source_node_ids: vec![],
            }],
            modularity: 0.3,
        });

        let results = index.search("payment", 10);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].community_id, "c1");
    }

    #[tokio::test]
    async fn test_extractive_provider_community() {
        let provider = ExtractiveProvider::new();
        let node_descs = vec![
            "User entity representing Alice".to_string(),
            "Project entity for payments-service".to_string(),
        ];
        let edge_descs = vec!["Alice works on payments-service".to_string()];

        let (title, summary, findings) = provider
            .summarize_community(&node_descs, &edge_descs)
            .await
            .unwrap();

        assert!(title.contains("Community:"));
        assert!(summary.contains("Alice"));
        assert_eq!(findings.len(), 2);
        assert!(findings[0].relevance_score > findings[1].relevance_score);
    }

    #[tokio::test]
    async fn test_extractive_provider_subcommunities() {
        let provider = ExtractiveProvider::new();
        let s1 = CommunitySummary {
            community_id: "c1".to_string(),
            level: 0,
            title: "SubA".to_string(),
            summary: "Summary A".to_string(),
            member_node_ids: vec![],
            child_summary_ids: vec![],
            findings: vec![Finding {
                description: "Finding 1".to_string(),
                relevance_score: 0.8,
                source_node_ids: vec![],
            }],
            modularity: 0.4,
        };
        let s2 = CommunitySummary {
            community_id: "c2".to_string(),
            level: 0,
            title: "SubB".to_string(),
            summary: "Summary B".to_string(),
            member_node_ids: vec![],
            child_summary_ids: vec![],
            findings: vec![Finding {
                description: "Finding 2".to_string(),
                relevance_score: 0.6,
                source_node_ids: vec![],
            }],
            modularity: 0.3,
        };

        let refs = vec![&s1, &s2];
        let (title, summary, findings) = provider.summarize_subcommunities(&refs).await.unwrap();

        assert!(title.contains("2 subcommunities"));
        assert!(summary.contains("Summary A"));
        assert!(summary.contains("Summary B"));
        assert_eq!(findings.len(), 2);
    }

    #[tokio::test]
    async fn test_graph_rag_engine_build_index() {
        let provider = Arc::new(ExtractiveProvider::new());
        let engine = GraphRagEngine::new(provider, 2, 1);

        let communities = vec![
            Community {
                id: "c1".to_string(),
                member_node_ids: vec!["n1".to_string(), "n2".to_string()],
                density: 1.0,
                level: 0,
                modularity: 0.5,
                parent_community_id: None,
            },
            Community {
                id: "c2".to_string(),
                member_node_ids: vec!["n3".to_string(), "n4".to_string()],
                density: 0.8,
                level: 0,
                modularity: 0.3,
                parent_community_id: None,
            },
        ];

        let mut node_descs = HashMap::new();
        node_descs.insert("n1".to_string(), "Alice entity".to_string());
        node_descs.insert("n2".to_string(), "Bob entity".to_string());
        node_descs.insert("n3".to_string(), "Payment service".to_string());
        node_descs.insert("n4".to_string(), "Auth service".to_string());

        let edge_descs: HashMap<(String, String), String> = HashMap::new();

        let index = engine
            .build_hierarchical_index(&communities, &node_descs, &edge_descs)
            .await
            .unwrap();

        assert!(index.total_communities >= 2);
        assert_eq!(index.get_level(0).len(), 2);
    }
}
