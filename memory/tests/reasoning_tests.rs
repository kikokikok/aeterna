use async_trait::async_trait;
use chrono::Utc;
use memory::embedding::mock::MockEmbeddingService;
use memory::manager::MemoryManager;
use memory::providers::MockProvider;
use memory::reasoning::ReflectiveReasoner;
use mk_core::types::{MemoryEntry, MemoryLayer, ReasoningStrategy, ReasoningTrace, TenantContext};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

struct ConfigurableMockReasoner {
    strategy: ReasoningStrategy,
    refined_query: Option<String>,
    delay_ms: u64,
    should_fail: bool,
    call_count: Arc<AtomicU32>
}

impl ConfigurableMockReasoner {
    fn new(strategy: ReasoningStrategy) -> Self {
        Self {
            strategy,
            refined_query: None,
            delay_ms: 0,
            should_fail: false,
            call_count: Arc::new(AtomicU32::new(0))
        }
    }

    fn with_refined_query(mut self, query: &str) -> Self {
        self.refined_query = Some(query.to_string());
        self
    }

    fn with_delay(mut self, ms: u64) -> Self {
        self.delay_ms = ms;
        self
    }

    fn with_failure(mut self) -> Self {
        self.should_fail = true;
        self
    }
}

#[async_trait]
impl ReflectiveReasoner for ConfigurableMockReasoner {
    async fn reason(
        &self,
        query: &str,
        context_summary: Option<&str>
    ) -> anyhow::Result<ReasoningTrace> {
        self.call_count.fetch_add(1, Ordering::SeqCst);

        if self.delay_ms > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(self.delay_ms)).await;
        }

        if self.should_fail {
            return Err(anyhow::anyhow!("Simulated reasoning failure"));
        }

        let thought_process = if let Some(ctx) = context_summary {
            format!(
                "Analyzed query '{}' with context '{}'. Strategy: {:?}",
                query, ctx, self.strategy
            )
        } else {
            format!("Analyzed query '{}'. Strategy: {:?}", query, self.strategy)
        };

        Ok(ReasoningTrace {
            strategy: self.strategy.clone(),
            thought_process,
            refined_query: self
                .refined_query
                .clone()
                .or_else(|| Some(format!("{} (refined)", query))),
            start_time: Utc::now(),
            end_time: Utc::now(),
            timed_out: false,
            duration_ms: 0,
            metadata: {
                let mut m = HashMap::new();
                m.insert(
                    "original_query".to_string(),
                    serde_json::json!(query.to_string())
                );
                m
            }
        })
    }
}

struct ExhaustiveReasoner;

#[async_trait]
impl ReflectiveReasoner for ExhaustiveReasoner {
    async fn reason(
        &self,
        query: &str,
        _context_summary: Option<&str>
    ) -> anyhow::Result<ReasoningTrace> {
        Ok(ReasoningTrace {
            strategy: ReasoningStrategy::Exhaustive,
            thought_process: format!(
                "Query '{}' requires exhaustive search across all layers",
                query
            ),
            refined_query: Some(format!("{} comprehensive analysis", query)),
            start_time: Utc::now(),
            end_time: Utc::now(),
            timed_out: false,
            duration_ms: 0,
            metadata: HashMap::new()
        })
    }
}

struct SemanticOnlyReasoner;

#[async_trait]
impl ReflectiveReasoner for SemanticOnlyReasoner {
    async fn reason(
        &self,
        query: &str,
        _context_summary: Option<&str>
    ) -> anyhow::Result<ReasoningTrace> {
        Ok(ReasoningTrace {
            strategy: ReasoningStrategy::SemanticOnly,
            thought_process: "Simple semantic search is sufficient".to_string(),
            refined_query: Some(query.to_string()),
            start_time: Utc::now(),
            end_time: Utc::now(),
            timed_out: false,
            duration_ms: 0,
            metadata: HashMap::new()
        })
    }
}

fn test_ctx() -> TenantContext {
    TenantContext::default()
}

fn create_reasoning_config(enabled: bool) -> config::ReasoningConfig {
    config::ReasoningConfig {
        enabled,
        timeout_ms: 5000,
        bypass_simple_queries: true,
        simple_query_max_words: 3,
        exhaustive_limit_multiplier: 2.0,
        targeted_limit_multiplier: 1.5,
        p95_latency_threshold_ms: 2500,
        cache_ttl_seconds: 3600,
        cache_enabled: true,
        cache_max_entries: 10000,
        circuit_breaker_enabled: true,
        circuit_breaker_failure_threshold_percent: 5.0,
        circuit_breaker_window_secs: 300,
        circuit_breaker_min_requests: 10,
        circuit_breaker_recovery_secs: 60,
        circuit_breaker_half_open_requests: 3,
        max_hop_depth: 3,
        hop_relevance_threshold: 0.3,
        max_query_budget: 50
    }
}

fn create_memory_config(reasoning: config::ReasoningConfig) -> config::MemoryConfig {
    config::MemoryConfig {
        promotion_threshold: 0.8,
        decay_interval_secs: 86400,
        decay_rate: 0.05,
        optimization_trigger_count: 100,
        layer_summary_configs: HashMap::new(),
        reasoning,
        rlm: config::RlmConfig::default()
    }
}

async fn setup_manager_with_reasoner(
    reasoner: Arc<dyn ReflectiveReasoner>,
    reasoning_enabled: bool
) -> MemoryManager {
    let reasoning_config = create_reasoning_config(reasoning_enabled);
    let memory_config = create_memory_config(reasoning_config);

    let manager = MemoryManager::new()
        .with_embedding_service(Arc::new(MockEmbeddingService::new(1536)))
        .with_reasoner(reasoner)
        .with_config(memory_config);

    register_all_layer_providers(&manager).await;

    manager
}

async fn register_all_layer_providers(manager: &MemoryManager) {
    let user_provider = Arc::new(MockProvider::new());
    let session_provider = Arc::new(MockProvider::new());
    let project_provider = Arc::new(MockProvider::new());

    manager
        .register_provider(MemoryLayer::User, user_provider)
        .await;
    manager
        .register_provider(MemoryLayer::Session, session_provider)
        .await;
    manager
        .register_provider(MemoryLayer::Project, project_provider)
        .await;
}

async fn add_test_memories(manager: &MemoryManager, ctx: TenantContext) {
    let memories = vec![
        (
            "mem_1",
            "Rust programming best practices for async code",
            MemoryLayer::User
        ),
        (
            "mem_2",
            "Database schema design patterns for PostgreSQL",
            MemoryLayer::User
        ),
        (
            "mem_3",
            "Memory optimization techniques in Aeterna",
            MemoryLayer::Session
        ),
        (
            "mem_4",
            "API design guidelines for REST services",
            MemoryLayer::Project
        ),
        (
            "mem_5",
            "Error handling patterns with anyhow and thiserror",
            MemoryLayer::User
        ),
    ];

    for (id, content, layer) in memories {
        let entry = MemoryEntry {
            id: id.to_string(),
            content: content.to_string(),
            embedding: None,
            layer,
            summaries: HashMap::new(),
            context_vector: None,
            importance_score: Some(0.8),
            metadata: {
                let mut m = HashMap::new();
                m.insert("score".to_string(), serde_json::json!(0.85));
                m
            },
            created_at: Utc::now().timestamp(),
            updated_at: Utc::now().timestamp()
        };

        manager
            .add_to_layer(ctx.clone(), layer, entry)
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn test_reflective_retrieval_with_targeted_strategy() {
    let reasoner = Arc::new(
        ConfigurableMockReasoner::new(ReasoningStrategy::Targeted)
            .with_refined_query("Rust async programming patterns")
    );
    let call_count = reasoner.call_count.clone();

    let manager = setup_manager_with_reasoner(reasoner, true).await;
    let ctx = test_ctx();

    add_test_memories(&manager, ctx.clone()).await;

    let (results, trace) = manager
        .search_text_with_reasoning(
            ctx,
            "How to write efficient async Rust code with proper error handling",
            10,
            0.0,
            HashMap::new(),
            Some("Working on a high-performance service")
        )
        .await
        .unwrap();

    assert!(trace.is_some(), "Reasoning trace should be present");
    assert_eq!(
        call_count.load(Ordering::SeqCst),
        1,
        "Reasoner should be called once"
    );

    let trace = trace.unwrap();
    assert_eq!(trace.strategy, ReasoningStrategy::Targeted);
    assert_eq!(
        trace.refined_query.as_deref(),
        Some("Rust async programming patterns")
    );
    assert!(trace.thought_process.contains("Analyzed query"));

    assert!(!results.is_empty(), "Should return search results");
}

#[tokio::test]
async fn test_reflective_retrieval_with_exhaustive_strategy() {
    let reasoner = Arc::new(ExhaustiveReasoner);

    let mut reasoning_config = create_reasoning_config(true);
    reasoning_config.exhaustive_limit_multiplier = 3.0;
    let memory_config = create_memory_config(reasoning_config);

    let manager = MemoryManager::new()
        .with_embedding_service(Arc::new(MockEmbeddingService::new(1536)))
        .with_reasoner(reasoner)
        .with_config(memory_config);

    register_all_layer_providers(&manager).await;

    let ctx = test_ctx();
    add_test_memories(&manager, ctx.clone()).await;

    let (results, trace) = manager
        .search_text_with_reasoning(
            ctx,
            "comprehensive analysis of memory system architecture design",
            5,
            0.0,
            HashMap::new(),
            None
        )
        .await
        .unwrap();

    assert!(trace.is_some());
    let trace = trace.unwrap();
    assert_eq!(trace.strategy, ReasoningStrategy::Exhaustive);
    assert!(!results.is_empty());
}

#[tokio::test]
async fn test_reflective_retrieval_with_semantic_only_strategy() {
    let reasoner = Arc::new(SemanticOnlyReasoner);
    let manager = setup_manager_with_reasoner(reasoner, true).await;
    let ctx = test_ctx();

    add_test_memories(&manager, ctx.clone()).await;

    let (results, trace) = manager
        .search_text_with_reasoning(
            ctx,
            "database design patterns for efficient storage",
            10,
            0.0,
            HashMap::new(),
            None
        )
        .await
        .unwrap();

    assert!(trace.is_some());
    let trace = trace.unwrap();
    assert_eq!(trace.strategy, ReasoningStrategy::SemanticOnly);
    assert!(!results.is_empty());
}

#[tokio::test]
async fn test_reflective_retrieval_bypasses_simple_queries() {
    let reasoner = Arc::new(ConfigurableMockReasoner::new(ReasoningStrategy::Targeted));
    let call_count = reasoner.call_count.clone();

    let manager = setup_manager_with_reasoner(reasoner, true).await;
    let ctx = test_ctx();

    add_test_memories(&manager, ctx.clone()).await;

    let (results, trace) = manager
        .search_text_with_reasoning(ctx, "Rust async", 10, 0.0, HashMap::new(), None)
        .await
        .unwrap();

    assert!(trace.is_none(), "Simple queries should bypass reasoning");
    assert_eq!(
        call_count.load(Ordering::SeqCst),
        0,
        "Reasoner should not be called for simple queries"
    );
    assert!(!results.is_empty());
}

#[tokio::test]
async fn test_reflective_retrieval_disabled_returns_no_trace() {
    let reasoner = Arc::new(ConfigurableMockReasoner::new(ReasoningStrategy::Targeted));
    let call_count = reasoner.call_count.clone();

    let manager = setup_manager_with_reasoner(reasoner, false).await;
    let ctx = test_ctx();

    add_test_memories(&manager, ctx.clone()).await;

    let (results, trace) = manager
        .search_text_with_reasoning(
            ctx,
            "complex query that would normally trigger reasoning analysis",
            10,
            0.0,
            HashMap::new(),
            None
        )
        .await
        .unwrap();

    assert!(trace.is_none(), "Disabled reasoning should return no trace");
    assert_eq!(
        call_count.load(Ordering::SeqCst),
        0,
        "Reasoner should not be called when disabled"
    );
    assert!(!results.is_empty());
}

#[tokio::test]
async fn test_reflective_retrieval_timeout_fallback() {
    let reasoner =
        Arc::new(ConfigurableMockReasoner::new(ReasoningStrategy::Targeted).with_delay(200));

    let mut reasoning_config = create_reasoning_config(true);
    reasoning_config.timeout_ms = 50;
    let memory_config = create_memory_config(reasoning_config);

    let manager = MemoryManager::new()
        .with_embedding_service(Arc::new(MockEmbeddingService::new(1536)))
        .with_reasoner(reasoner)
        .with_config(memory_config);

    register_all_layer_providers(&manager).await;

    let ctx = test_ctx();
    add_test_memories(&manager, ctx.clone()).await;

    let (results, trace) = manager
        .search_text_with_reasoning(
            ctx,
            "complex query that triggers reasoning but times out",
            10,
            0.0,
            HashMap::new(),
            None
        )
        .await
        .unwrap();

    let trace = trace.expect("Timeout should return a trace with timed_out=true");
    assert!(trace.timed_out, "Trace should indicate timeout occurred");
    assert_eq!(
        trace.strategy,
        ReasoningStrategy::SemanticOnly,
        "Timeout should fall back to SemanticOnly strategy"
    );
    assert!(
        trace.refined_query.is_none(),
        "Timeout trace should have no refined query"
    );
    assert!(
        !results.is_empty(),
        "Search should still succeed after timeout"
    );
}

#[tokio::test]
async fn test_reflective_retrieval_failure_fallback() {
    let reasoner =
        Arc::new(ConfigurableMockReasoner::new(ReasoningStrategy::Targeted).with_failure());

    let manager = setup_manager_with_reasoner(reasoner, true).await;
    let ctx = test_ctx();

    add_test_memories(&manager, ctx.clone()).await;

    let (results, trace) = manager
        .search_text_with_reasoning(
            ctx,
            "complex query that causes reasoning failure",
            10,
            0.0,
            HashMap::new(),
            None
        )
        .await
        .unwrap();

    assert!(
        trace.is_none(),
        "Failed reasoning should fallback with no trace"
    );
    assert!(
        !results.is_empty(),
        "Search should succeed despite reasoning failure"
    );
}

#[tokio::test]
async fn test_reflective_retrieval_with_context_summary() {
    let reasoner = Arc::new(ConfigurableMockReasoner::new(ReasoningStrategy::Targeted));
    let manager = setup_manager_with_reasoner(reasoner, true).await;
    let ctx = test_ctx();

    add_test_memories(&manager, ctx.clone()).await;

    let context = "Currently debugging a performance issue in the memory manager";

    let (_, trace) = manager
        .search_text_with_reasoning(
            ctx,
            "memory optimization and performance tuning techniques",
            10,
            0.0,
            HashMap::new(),
            Some(context)
        )
        .await
        .unwrap();

    assert!(trace.is_some());
    let trace = trace.unwrap();

    assert!(
        trace.thought_process.contains(context),
        "Context should be included in reasoning"
    );
}

#[tokio::test]
async fn test_reflective_retrieval_preserves_metadata() {
    let reasoner = Arc::new(ConfigurableMockReasoner::new(ReasoningStrategy::Targeted));
    let manager = setup_manager_with_reasoner(reasoner, true).await;
    let ctx = test_ctx();

    add_test_memories(&manager, ctx.clone()).await;

    let (_, trace) = manager
        .search_text_with_reasoning(
            ctx,
            "query for testing metadata preservation in reasoning trace",
            10,
            0.0,
            HashMap::new(),
            None
        )
        .await
        .unwrap();

    assert!(trace.is_some());
    let trace = trace.unwrap();

    assert!(
        trace.metadata.contains_key("original_query"),
        "Metadata should contain original query"
    );
}

#[tokio::test]
async fn test_reflective_retrieval_multiple_searches() {
    let reasoner = Arc::new(ConfigurableMockReasoner::new(ReasoningStrategy::Targeted));
    let call_count = reasoner.call_count.clone();

    let manager = setup_manager_with_reasoner(reasoner, true).await;
    let ctx = test_ctx();

    add_test_memories(&manager, ctx.clone()).await;

    for i in 0..5 {
        let (results, trace) = manager
            .search_text_with_reasoning(
                ctx.clone(),
                &format!("complex search query number {} for testing", i),
                10,
                0.0,
                HashMap::new(),
                None
            )
            .await
            .unwrap();

        assert!(trace.is_some());
        assert!(!results.is_empty());
    }

    assert_eq!(
        call_count.load(Ordering::SeqCst),
        5,
        "Reasoner should be called for each search"
    );
}

#[tokio::test]
async fn test_reflective_retrieval_with_filters() {
    let reasoner = Arc::new(ConfigurableMockReasoner::new(ReasoningStrategy::Targeted));
    let manager = setup_manager_with_reasoner(reasoner, true).await;
    let ctx = test_ctx();

    add_test_memories(&manager, ctx.clone()).await;

    let mut filters = HashMap::new();
    filters.insert("score".to_string(), serde_json::json!(0.85));

    let (results, trace) = manager
        .search_text_with_reasoning(
            ctx,
            "search with filters for specific memory score",
            10,
            0.0,
            filters,
            None
        )
        .await
        .unwrap();

    assert!(trace.is_some());
    assert!(!results.is_empty());
}

#[tokio::test]
async fn test_reflective_retrieval_limit_adjustment_targeted() {
    let reasoner = Arc::new(ConfigurableMockReasoner::new(ReasoningStrategy::Targeted));

    let mut reasoning_config = create_reasoning_config(true);
    reasoning_config.targeted_limit_multiplier = 1.5;
    let memory_config = create_memory_config(reasoning_config);

    let manager = MemoryManager::new()
        .with_embedding_service(Arc::new(MockEmbeddingService::new(1536)))
        .with_reasoner(reasoner)
        .with_config(memory_config);

    register_all_layer_providers(&manager).await;

    let ctx = test_ctx();
    add_test_memories(&manager, ctx.clone()).await;

    let (results, trace) = manager
        .search_text_with_reasoning(
            ctx,
            "complex query to test targeted limit adjustment behavior",
            10,
            0.0,
            HashMap::new(),
            None
        )
        .await
        .unwrap();

    assert!(trace.is_some());
    let trace = trace.unwrap();
    assert_eq!(trace.strategy, ReasoningStrategy::Targeted);
    assert!(!results.is_empty());
}

#[tokio::test]
async fn test_reflective_retrieval_without_reasoner_configured() {
    let reasoning_config = create_reasoning_config(true);
    let memory_config = create_memory_config(reasoning_config);

    let manager = MemoryManager::new()
        .with_embedding_service(Arc::new(MockEmbeddingService::new(1536)))
        .with_config(memory_config);

    register_all_layer_providers(&manager).await;

    let ctx = test_ctx();
    add_test_memories(&manager, ctx.clone()).await;

    let (results, trace) = manager
        .search_text_with_reasoning(
            ctx,
            "query without configured reasoner should still work",
            10,
            0.0,
            HashMap::new(),
            None
        )
        .await
        .unwrap();

    assert!(trace.is_none(), "No reasoner means no trace");
    assert!(!results.is_empty(), "Search should still work");
}

#[tokio::test]
async fn test_reflective_retrieval_empty_results() {
    let reasoner = Arc::new(ConfigurableMockReasoner::new(ReasoningStrategy::Targeted));
    let manager = setup_manager_with_reasoner(reasoner, true).await;
    let ctx = test_ctx();

    let (results, trace) = manager
        .search_text_with_reasoning(
            ctx,
            "query that should return no results from empty store",
            10,
            0.5,
            HashMap::new(),
            None
        )
        .await
        .unwrap();

    assert!(trace.is_some());
    assert!(results.is_empty());
}

#[tokio::test]
async fn test_reflective_retrieval_timing_recorded() {
    let reasoner =
        Arc::new(ConfigurableMockReasoner::new(ReasoningStrategy::Targeted).with_delay(10));
    let manager = setup_manager_with_reasoner(reasoner, true).await;
    let ctx = test_ctx();

    add_test_memories(&manager, ctx.clone()).await;

    let (_, trace) = manager
        .search_text_with_reasoning(
            ctx,
            "query to test timing is recorded in reasoning trace",
            10,
            0.0,
            HashMap::new(),
            None
        )
        .await
        .unwrap();

    assert!(trace.is_some());
    let trace = trace.unwrap();

    assert!(trace.start_time <= trace.end_time);
}

// =====================================================
// Section 4.2 - Reasoning Cache Integration Tests
// =====================================================

#[tokio::test]
async fn test_reasoning_cache_hit_skips_llm_call() {
    let reasoner = Arc::new(ConfigurableMockReasoner::new(ReasoningStrategy::Targeted));
    let call_count = reasoner.call_count.clone();

    let manager = setup_manager_with_reasoner(reasoner, true).await;
    let ctx = test_ctx();

    add_test_memories(&manager, ctx.clone()).await;

    // First call - should invoke reasoner (cache miss)
    let query = "complex query for testing cache behavior with reasoning";
    let (results1, trace1) = manager
        .search_text_with_reasoning(ctx.clone(), query, 10, 0.0, HashMap::new(), None)
        .await
        .unwrap();

    assert!(trace1.is_some(), "First call should have trace");
    assert_eq!(
        call_count.load(Ordering::SeqCst),
        1,
        "First call should invoke reasoner"
    );
    assert!(!results1.is_empty());

    // Second call with same query - should use cache (cache hit)
    let (results2, trace2) = manager
        .search_text_with_reasoning(ctx.clone(), query, 10, 0.0, HashMap::new(), None)
        .await
        .unwrap();

    assert!(trace2.is_some(), "Cached call should have trace");
    assert_eq!(
        call_count.load(Ordering::SeqCst),
        1,
        "Second call should use cache, not invoke reasoner again"
    );
    assert!(!results2.is_empty());

    // Verify traces are consistent
    let t1 = trace1.unwrap();
    let t2 = trace2.unwrap();
    assert_eq!(t1.strategy, t2.strategy);
    assert_eq!(t1.refined_query, t2.refined_query);
}

#[tokio::test]
async fn test_reasoning_cache_different_queries_miss() {
    let reasoner = Arc::new(ConfigurableMockReasoner::new(ReasoningStrategy::Targeted));
    let call_count = reasoner.call_count.clone();

    let manager = setup_manager_with_reasoner(reasoner, true).await;
    let ctx = test_ctx();

    add_test_memories(&manager, ctx.clone()).await;

    // First query
    let (_, trace1) = manager
        .search_text_with_reasoning(
            ctx.clone(),
            "first unique query for cache testing scenario",
            10,
            0.0,
            HashMap::new(),
            None
        )
        .await
        .unwrap();

    assert!(trace1.is_some());
    assert_eq!(call_count.load(Ordering::SeqCst), 1);

    // Different query - should be cache miss
    let (_, trace2) = manager
        .search_text_with_reasoning(
            ctx.clone(),
            "second completely different query for testing",
            10,
            0.0,
            HashMap::new(),
            None
        )
        .await
        .unwrap();

    assert!(trace2.is_some());
    assert_eq!(
        call_count.load(Ordering::SeqCst),
        2,
        "Different queries should each invoke reasoner"
    );
}

#[tokio::test]
async fn test_reasoning_cache_normalized_queries_hit() {
    let reasoner = Arc::new(ConfigurableMockReasoner::new(ReasoningStrategy::Targeted));
    let call_count = reasoner.call_count.clone();

    let manager = setup_manager_with_reasoner(reasoner, true).await;
    let ctx = test_ctx();

    add_test_memories(&manager, ctx.clone()).await;

    // First call with a normal query
    let (_, trace1) = manager
        .search_text_with_reasoning(
            ctx.clone(),
            "query for cache normalization test",
            10,
            0.0,
            HashMap::new(),
            None
        )
        .await
        .unwrap();

    assert!(trace1.is_some());
    assert_eq!(call_count.load(Ordering::SeqCst), 1);

    // The same query with different casing and whitespace - should hit cache
    let (_, trace2) = manager
        .search_text_with_reasoning(
            ctx.clone(),
            "  QUERY   FOR   CACHE   NORMALIZATION   TEST  ",
            10,
            0.0,
            HashMap::new(),
            None
        )
        .await
        .unwrap();

    assert!(trace2.is_some());
    assert_eq!(
        call_count.load(Ordering::SeqCst),
        1,
        "Normalized queries should hit cache"
    );
}

#[tokio::test]
async fn test_reasoning_cache_disabled_always_calls_llm() {
    let reasoner = Arc::new(ConfigurableMockReasoner::new(ReasoningStrategy::Targeted));
    let call_count = reasoner.call_count.clone();

    // Create manager with cache disabled
    let mut reasoning_config = create_reasoning_config(true);
    reasoning_config.cache_enabled = false;
    let memory_config = create_memory_config(reasoning_config);

    let manager = MemoryManager::new()
        .with_embedding_service(Arc::new(MockEmbeddingService::new(1536)))
        .with_reasoner(reasoner)
        .with_config(memory_config);

    register_all_layer_providers(&manager).await;

    let ctx = test_ctx();
    add_test_memories(&manager, ctx.clone()).await;

    let query = "query for testing disabled cache behavior";

    // First call
    let (_, trace1) = manager
        .search_text_with_reasoning(ctx.clone(), query, 10, 0.0, HashMap::new(), None)
        .await
        .unwrap();

    assert!(trace1.is_some());
    assert_eq!(call_count.load(Ordering::SeqCst), 1);

    // Second call with same query - should still invoke reasoner (cache disabled)
    let (_, trace2) = manager
        .search_text_with_reasoning(ctx.clone(), query, 10, 0.0, HashMap::new(), None)
        .await
        .unwrap();

    assert!(trace2.is_some());
    assert_eq!(
        call_count.load(Ordering::SeqCst),
        2,
        "With cache disabled, every call should invoke reasoner"
    );
}

#[tokio::test]
async fn test_reasoning_cache_different_tenants_separate_cache() {
    let reasoner = Arc::new(ConfigurableMockReasoner::new(ReasoningStrategy::Targeted));
    let call_count = reasoner.call_count.clone();

    let manager = setup_manager_with_reasoner(reasoner, true).await;

    // Create two different tenant contexts
    let ctx1 = TenantContext::new(
        mk_core::types::TenantId::new("tenant-1".to_string()).unwrap(),
        mk_core::types::UserId::new("user-1".to_string()).unwrap()
    );
    let ctx2 = TenantContext::new(
        mk_core::types::TenantId::new("tenant-2".to_string()).unwrap(),
        mk_core::types::UserId::new("user-2".to_string()).unwrap()
    );

    add_test_memories(&manager, ctx1.clone()).await;
    add_test_memories(&manager, ctx2.clone()).await;

    let query = "identical query for testing tenant isolation in cache";

    // First tenant's query
    let (_, trace1) = manager
        .search_text_with_reasoning(ctx1.clone(), query, 10, 0.0, HashMap::new(), None)
        .await
        .unwrap();

    assert!(trace1.is_some());
    assert_eq!(call_count.load(Ordering::SeqCst), 1);

    // Same query, different tenant - should be cache miss
    let (_, trace2) = manager
        .search_text_with_reasoning(ctx2.clone(), query, 10, 0.0, HashMap::new(), None)
        .await
        .unwrap();

    assert!(trace2.is_some());
    assert_eq!(
        call_count.load(Ordering::SeqCst),
        2,
        "Different tenants should have separate cache entries"
    );

    // First tenant again - should hit cache
    let (_, trace3) = manager
        .search_text_with_reasoning(ctx1.clone(), query, 10, 0.0, HashMap::new(), None)
        .await
        .unwrap();

    assert!(trace3.is_some());
    assert_eq!(
        call_count.load(Ordering::SeqCst),
        2,
        "Same tenant should hit cache"
    );
}

#[tokio::test]
async fn test_reasoning_cache_simple_query_bypass_not_cached() {
    let reasoner = Arc::new(ConfigurableMockReasoner::new(ReasoningStrategy::Targeted));
    let call_count = reasoner.call_count.clone();

    let manager = setup_manager_with_reasoner(reasoner, true).await;
    let ctx = test_ctx();

    add_test_memories(&manager, ctx.clone()).await;

    // Simple query (bypasses reasoning)
    let simple_query = "simple test";

    let (_, trace1) = manager
        .search_text_with_reasoning(ctx.clone(), simple_query, 10, 0.0, HashMap::new(), None)
        .await
        .unwrap();

    assert!(trace1.is_none(), "Simple query should bypass reasoning");
    assert_eq!(call_count.load(Ordering::SeqCst), 0);

    // Same simple query again
    let (_, trace2) = manager
        .search_text_with_reasoning(ctx.clone(), simple_query, 10, 0.0, HashMap::new(), None)
        .await
        .unwrap();

    assert!(
        trace2.is_none(),
        "Simple query should still bypass reasoning"
    );
    assert_eq!(
        call_count.load(Ordering::SeqCst),
        0,
        "Simple queries should never invoke reasoner"
    );
}
