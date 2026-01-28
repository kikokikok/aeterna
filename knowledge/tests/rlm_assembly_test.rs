use std::sync::Arc;

use async_trait::async_trait;
use knowledge::context_architect::{AssemblerConfig, ContextAssembler, SummarySource};
use mk_core::traits::{RlmAssemblyResult, RlmAssemblyService};
use mk_core::types::{MemoryLayer, TenantContext, TenantId, UserId};
use std::str::FromStr;

struct MockRlmHandler {
    complexity_threshold: f32,
    should_fail: bool
}

impl MockRlmHandler {
    fn new(complexity_threshold: f32) -> Self {
        Self {
            complexity_threshold,
            should_fail: false
        }
    }

    fn failing() -> Self {
        Self {
            complexity_threshold: 0.0,
            should_fail: true
        }
    }
}

#[async_trait]
impl RlmAssemblyService for MockRlmHandler {
    fn should_use_rlm(&self, query_text: &str) -> bool {
        self.compute_complexity(query_text) >= self.complexity_threshold
    }

    fn compute_complexity(&self, query_text: &str) -> f32 {
        let mut score: f32 = 0.0;

        if query_text.contains("compare") || query_text.contains("summarize") {
            score += 0.3;
        }
        if query_text.contains("across") || query_text.contains("all teams") {
            score += 0.3;
        }
        if query_text.contains("trends") || query_text.contains("evolution") {
            score += 0.2;
        }

        score.min(1.0)
    }

    async fn execute_assembly(
        &self,
        query_text: &str,
        _tenant: &TenantContext
    ) -> Result<RlmAssemblyResult, anyhow::Error> {
        if self.should_fail {
            return Err(anyhow::anyhow!("Mock RLM failure"));
        }

        Ok(RlmAssemblyResult {
            content: format!("RLM synthesized result for: {}", query_text),
            involved_memory_ids: vec!["mem-1".to_string(), "mem-2".to_string()],
            rlm_synthesized: true,
            steps: 3,
            total_reward: 0.8
        })
    }
}

fn create_tenant() -> TenantContext {
    TenantContext::new(
        TenantId::from_str("test-tenant").unwrap(),
        UserId::from_str("test-user").unwrap()
    )
}

fn create_sample_sources() -> Vec<SummarySource> {
    use mk_core::types::{LayerSummary, SummaryDepth};
    use std::collections::HashMap;

    let mut summaries = HashMap::new();
    summaries.insert(
        SummaryDepth::Sentence,
        LayerSummary {
            depth: SummaryDepth::Sentence,
            content: "Standard search result".to_string(),
            token_count: 20,
            generated_at: 0,
            source_hash: "hash".to_string(),
            content_hash: None,
            personalized: false,
            personalization_context: None
        }
    );

    vec![SummarySource {
        entry_id: "entry-1".to_string(),
        layer: MemoryLayer::Project,
        summaries,
        context_vector: None,
        full_content: None,
        full_content_tokens: None,
        current_source_content: None
    }]
}

#[tokio::test]
async fn test_assembler_without_rlm_handler() {
    let assembler = ContextAssembler::new(AssemblerConfig::default());

    assert!(!assembler.has_rlm_handler());
    assert!(!assembler.should_use_rlm("compare all patterns across teams"));
    assert_eq!(assembler.compute_query_complexity("any query"), 0.0);
}

#[tokio::test]
async fn test_assembler_with_rlm_handler() {
    let handler = Arc::new(MockRlmHandler::new(0.3));
    let assembler =
        ContextAssembler::new(AssemblerConfig::default()).with_rlm_handler(handler.clone());

    assert!(assembler.has_rlm_handler());
}

#[tokio::test]
async fn test_complexity_routing_simple_query() {
    let handler = Arc::new(MockRlmHandler::new(0.3));
    let assembler = ContextAssembler::new(AssemblerConfig::default()).with_rlm_handler(handler);

    let simple_query = "what is the login endpoint";
    assert!(!assembler.should_use_rlm(simple_query));
    assert!(assembler.compute_query_complexity(simple_query) < 0.3);
}

#[tokio::test]
async fn test_complexity_routing_complex_query() {
    let handler = Arc::new(MockRlmHandler::new(0.3));
    let assembler = ContextAssembler::new(AssemblerConfig::default()).with_rlm_handler(handler);

    let complex_query = "compare and summarize all patterns across teams";
    assert!(assembler.should_use_rlm(complex_query));
    assert!(assembler.compute_query_complexity(complex_query) >= 0.3);
}

#[tokio::test]
async fn test_assemble_with_rlm_routes_complex_query() {
    let handler = Arc::new(MockRlmHandler::new(0.3));
    let assembler = ContextAssembler::new(AssemblerConfig::default()).with_rlm_handler(handler);

    let tenant = create_tenant();
    let sources = create_sample_sources();
    let complex_query = "compare and summarize patterns across all teams";

    let result = assembler
        .assemble_with_rlm(complex_query, None, &sources, None, &tenant)
        .await;

    assert!(result.view.content.contains("RLM synthesized"));
    assert!(result.view.metadata.view_type == "rlm_synthesized");
    assert!(!result.entries.is_empty());
}

#[tokio::test]
async fn test_assemble_with_rlm_routes_simple_query_to_standard() {
    let handler = Arc::new(MockRlmHandler::new(0.3));
    let assembler = ContextAssembler::new(AssemblerConfig::default()).with_rlm_handler(handler);

    let tenant = create_tenant();
    let sources = create_sample_sources();
    let simple_query = "show me the config";

    let result = assembler
        .assemble_with_rlm(simple_query, None, &sources, None, &tenant)
        .await;

    assert!(!result.view.content.contains("RLM synthesized"));
    assert!(result.view.metadata.view_type != "rlm_synthesized");
}

#[tokio::test]
async fn test_assemble_with_rlm_fallback_on_failure() {
    let handler = Arc::new(MockRlmHandler::failing());
    let assembler = ContextAssembler::new(AssemblerConfig::default()).with_rlm_handler(handler);

    let tenant = create_tenant();
    let sources = create_sample_sources();
    let complex_query = "compare patterns";

    let result = assembler
        .assemble_with_rlm(complex_query, None, &sources, None, &tenant)
        .await;

    assert!(!result.view.content.contains("RLM synthesized"));
    assert!(
        result
            .entries
            .iter()
            .any(|e| e.content.contains("Standard"))
    );
}

#[tokio::test]
async fn test_assemble_with_rlm_no_handler_uses_standard() {
    let assembler = ContextAssembler::new(AssemblerConfig::default());

    let tenant = create_tenant();
    let sources = create_sample_sources();
    let complex_query = "compare and summarize patterns across all teams";

    let result = assembler
        .assemble_with_rlm(complex_query, None, &sources, None, &tenant)
        .await;

    assert!(!result.view.content.contains("RLM synthesized"));
}

#[tokio::test]
async fn test_rlm_result_includes_metadata() {
    let handler = Arc::new(MockRlmHandler::new(0.0));
    let assembler = ContextAssembler::new(AssemblerConfig::default()).with_rlm_handler(handler);

    let tenant = create_tenant();
    let sources = create_sample_sources();

    let result = assembler
        .assemble_with_rlm("any query", None, &sources, None, &tenant)
        .await;

    assert!(result.view.metadata.view_type == "rlm_synthesized");
    assert!(!result.entries.is_empty());
    assert!(result.entries[0].entry_id.starts_with("rlm-"));
    assert_eq!(result.entries[0].relevance_score, 0.95);
}

#[tokio::test]
async fn test_rlm_respects_token_budget() {
    let handler = Arc::new(MockRlmHandler::new(0.0));
    let assembler = ContextAssembler::new(AssemblerConfig::default()).with_rlm_handler(handler);

    let tenant = create_tenant();
    let sources = create_sample_sources();
    let custom_budget = 500u32;

    let result = assembler
        .assemble_with_rlm("query", None, &sources, Some(custom_budget), &tenant)
        .await;

    assert_eq!(result.token_budget, custom_budget);
}

#[tokio::test]
async fn test_multiple_complexity_levels() {
    let handler = Arc::new(MockRlmHandler::new(0.3));
    let assembler = ContextAssembler::new(AssemblerConfig::default()).with_rlm_handler(handler);

    let queries_and_expected = vec![
        ("simple query", false),
        ("compare items", true),
        ("show trends", false),
        ("summarize all teams", true),
        ("compare and summarize trends across all teams", true),
    ];

    for (query, expected_rlm) in queries_and_expected {
        let should_use = assembler.should_use_rlm(query);
        assert_eq!(
            should_use, expected_rlm,
            "Query '{}' should_use_rlm={}, expected {}",
            query, should_use, expected_rlm
        );
    }
}
