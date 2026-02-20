//! Complexity-based query routing for RLM.

use mk_core::types::SearchQuery;
use regex::Regex;
use serde::{Deserialize, Serialize};

/// Extracted complexity signals from a query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexitySignals {
    pub query_length: usize,
    pub keyword_density: f32,
    pub multi_hop_indicators: usize,
    pub temporal_constraints: bool,
    pub aggregate_operators: bool,
}

/// Routes queries to RLM or standard search based on complexity score.
pub struct ComplexityRouter {
    config: config::RlmConfig,
    keywords: Vec<Regex>,
}

impl ComplexityRouter {
    pub fn new(config: config::RlmConfig) -> Self {
        let keyword_patterns = vec![
            r"(?i)\bcompare\b",
            r"(?i)\bdifference\b",
            r"(?i)\btrends?\b",
            r"(?i)\bevolution\b",
            r"(?i)\bhistory\b",
            r"(?i)\bsummarize\b",
            r"(?i)\baggregate\b",
            r"(?i)\bimpact\b",
            r"(?i)\brelationship\b",
            r"(?i)\bsequence\b",
            r"(?i)\banalyze\b",
            r"(?i)\btrace\b",
        ];

        let keywords = keyword_patterns
            .into_iter()
            .map(|p| Regex::new(p).unwrap())
            .collect();

        Self { config, keywords }
    }

    pub fn compute_complexity(&self, query: &SearchQuery) -> f32 {
        let text = &query.text;
        let signals = self.extract_signals(text);

        let mut score = 0.0;

        let normalized_length_score = (signals.query_length as f32 / 200.0).min(1.0) * 0.2;
        score += normalized_length_score;

        let density_score = signals.keyword_density * 0.4;
        score += density_score;

        let multi_hop_score = (signals.multi_hop_indicators as f32 / 3.0).min(1.0) * 0.2;
        score += multi_hop_score;

        if signals.temporal_constraints {
            score += 0.1;
        }
        if signals.aggregate_operators {
            score += 0.1;
        }

        score.min(1.0)
    }

    pub fn should_route_to_rlm(&self, query: &SearchQuery) -> bool {
        if !self.config.enabled {
            return false;
        }
        self.compute_complexity(query) >= self.config.complexity_threshold
    }

    fn extract_signals(&self, text: &str) -> ComplexitySignals {
        let query_length = text.len();

        let mut keyword_count = 0;
        for re in &self.keywords {
            if re.is_match(text) {
                keyword_count += 1;
            }
        }

        let multi_hop_indicators = self.count_multi_hop_indicators(text);
        let temporal_constraints = self.has_temporal_constraints(text);
        let aggregate_operators = self.has_aggregate_operators(text);

        ComplexitySignals {
            query_length,
            keyword_density: keyword_count as f32 / self.keywords.len() as f32,
            multi_hop_indicators,
            temporal_constraints,
            aggregate_operators,
        }
    }

    fn count_multi_hop_indicators(&self, text: &str) -> usize {
        let patterns = [
            r"(?i)\bthen\b",
            r"(?i)\bafter\b",
            r"(?i)\bfollowed by\b",
            r"(?i)\bcaused\b",
            r"(?i)\bleading to\b",
        ];

        patterns
            .iter()
            .filter(|p| Regex::new(p).unwrap().is_match(text))
            .count()
    }

    fn has_temporal_constraints(&self, text: &str) -> bool {
        let patterns = [
            r"(?i)\blast (?:week|month|quarter|year)\b",
            r"(?i)\byesterday\b",
            r"(?i)\bsince\b",
            r"(?i)\bbefore\b",
            r"(?i)\bperiod\b",
            r"(?i)\bover the last\b",
            r"(?i)\bin the last\b",
            r"(?i)\bover time\b",
        ];

        patterns
            .iter()
            .any(|p| Regex::new(p).unwrap().is_match(text))
    }

    fn has_aggregate_operators(&self, text: &str) -> bool {
        let patterns = [
            r"(?i)\ball\b",
            r"(?i)\bevery\b",
            r"(?i)\btotal\b",
            r"(?i)\baverage\b",
            r"(?i)\bcount\b",
        ];

        patterns
            .iter()
            .any(|p| Regex::new(p).unwrap().is_match(text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mk_core::types::SearchQuery;

    #[test]
    fn test_complexity_scoring() {
        let router = ComplexityRouter::new(config::RlmConfig {
            enabled: true,
            max_steps: 5,
            complexity_threshold: 0.3,
        });

        let q1 = SearchQuery {
            text: "how to login".to_string(),
            ..Default::default()
        };
        assert!(router.compute_complexity(&q1) < 0.3);

        let q2 = SearchQuery {
            text: "compare the evolution of auth patterns between last week and today and \
                   summarize the impact"
                .to_string(),
            ..Default::default()
        };
        let score = router.compute_complexity(&q2);
        assert!(score >= 0.3);
        assert!(router.should_route_to_rlm(&q2));
    }

    #[test]
    fn test_simple_query_routes_to_standard() {
        let router = ComplexityRouter::new(config::RlmConfig {
            enabled: true,
            max_steps: 5,
            complexity_threshold: 0.3,
        });

        let simple_queries = vec![
            "what is the login endpoint",
            "show me the config file",
            "list all users",
            "get database connection",
        ];

        for query_text in simple_queries {
            let query = SearchQuery {
                text: query_text.to_string(),
                ..Default::default()
            };
            assert!(
                !router.should_route_to_rlm(&query),
                "Simple query '{}' should route to standard search",
                query_text
            );
        }
    }

    #[test]
    fn test_complex_query_routes_to_rlm() {
        let router = ComplexityRouter::new(config::RlmConfig {
            enabled: true,
            max_steps: 5,
            complexity_threshold: 0.3,
        });

        let complex_queries = vec![
            "compare all database patterns across teams and summarize trends over the last month",
            "trace the evolution of authentication from last year and analyze its impact on \
             security",
            "aggregate all error patterns then compare them with last week's performance metrics",
        ];

        for query_text in complex_queries {
            let query = SearchQuery {
                text: query_text.to_string(),
                ..Default::default()
            };
            assert!(
                router.should_route_to_rlm(&query),
                "Complex query '{}' should route to RLM (score: {})",
                query_text,
                router.compute_complexity(&query)
            );
        }
    }

    #[test]
    fn test_disabled_rlm_routes_all_to_standard() {
        let router = ComplexityRouter::new(config::RlmConfig {
            enabled: false,
            max_steps: 5,
            complexity_threshold: 0.3,
        });

        let complex_query = SearchQuery {
            text: "compare and analyze all trends since last quarter and summarize the evolution"
                .to_string(),
            ..Default::default()
        };

        assert!(
            !router.should_route_to_rlm(&complex_query),
            "Disabled RLM should route all queries to standard search"
        );
    }

    #[test]
    fn test_query_at_threshold_boundary() {
        let router = ComplexityRouter::new(config::RlmConfig {
            enabled: true,
            max_steps: 5,
            complexity_threshold: 0.3,
        });

        let boundary_query = SearchQuery {
            text: "compare two patterns".to_string(),
            ..Default::default()
        };

        let score = router.compute_complexity(&boundary_query);
        let routes_to_rlm = router.should_route_to_rlm(&boundary_query);

        assert_eq!(
            routes_to_rlm,
            score >= 0.3,
            "Routing decision should match threshold comparison"
        );
    }

    #[test]
    fn test_multi_hop_indicators_increase_score() {
        let router = ComplexityRouter::new(config::RlmConfig {
            enabled: true,
            max_steps: 5,
            complexity_threshold: 0.3,
        });

        let without_multi_hop = SearchQuery {
            text: "show me the config".to_string(),
            ..Default::default()
        };

        let with_multi_hop = SearchQuery {
            text: "show me the config then check what caused the error leading to the crash"
                .to_string(),
            ..Default::default()
        };

        let score_without = router.compute_complexity(&without_multi_hop);
        let score_with = router.compute_complexity(&with_multi_hop);

        assert!(
            score_with > score_without,
            "Multi-hop indicators should increase complexity score"
        );
    }

    #[test]
    fn test_temporal_constraints_increase_score() {
        let router = ComplexityRouter::new(config::RlmConfig {
            enabled: true,
            max_steps: 5,
            complexity_threshold: 0.3,
        });

        let without_temporal = SearchQuery {
            text: "show me the deployment logs".to_string(),
            ..Default::default()
        };

        let with_temporal = SearchQuery {
            text: "show me the deployment logs from last week".to_string(),
            ..Default::default()
        };

        let score_without = router.compute_complexity(&without_temporal);
        let score_with = router.compute_complexity(&with_temporal);

        assert!(
            score_with > score_without,
            "Temporal constraints should increase complexity score"
        );
    }

    #[test]
    fn test_aggregate_operators_increase_score() {
        let router = ComplexityRouter::new(config::RlmConfig {
            enabled: true,
            max_steps: 5,
            complexity_threshold: 0.3,
        });

        let without_aggregate = SearchQuery {
            text: "show me the users".to_string(),
            ..Default::default()
        };

        let with_aggregate = SearchQuery {
            text: "show me the total count of all users".to_string(),
            ..Default::default()
        };

        let score_without = router.compute_complexity(&without_aggregate);
        let score_with = router.compute_complexity(&with_aggregate);

        assert!(
            score_with > score_without,
            "Aggregate operators should increase complexity score"
        );
    }
}
