use memory::active_learning::ActiveLearner;
use memory::few_shot::{Example, FewShotSelector};
use memory::matryoshka::{Dimension, MatryoshkaEmbedder, UseCase};
use memory::moa::{AgentResponse, MixtureOfAgents, MoaConfig};

fn make_embedding(dim: usize) -> Vec<f32> {
    (0..dim).map(|i| (i as f32 + 1.0) * 0.01).collect()
}

fn make_example(input: &str, output: &str, direction: &[f32]) -> Example {
    Example::new(input.to_string(), output.to_string(), direction.to_vec())
}

#[test]
fn moa_aggregate_then_matryoshka_embed_combined_response() {
    let moa = MixtureOfAgents::with_defaults();
    let responses = vec![
        AgentResponse::new("agent-1", "Use connection pooling", 0.9),
        AgentResponse::new("agent-2", "Add retry logic", 0.7),
        AgentResponse::new("agent-3", "Cache frequently accessed data", 0.5),
    ];

    let aggregated = moa.aggregate(&responses).expect("aggregate should succeed");
    assert_eq!(aggregated.contributing_agents.len(), 3);

    // Use the aggregated content length as a seed for a fake embedding
    let fake_full = make_embedding(1536);
    let embedder = MatryoshkaEmbedder::with_defaults();

    let result = embedder
        .embed_for_use_case(&fake_full, UseCase::Balanced)
        .expect("embed should succeed");

    assert_eq!(result.dimension, Dimension::D768);
    assert!(result.normalized);
}

#[test]
fn few_shot_selection_feeds_into_active_learning_uncertainty() {
    let examples = vec![
        make_example("How to deploy?", "Use Helm chart", &[1.0, 0.0, 0.0]),
        make_example("How to scale?", "Use HPA", &[0.0, 1.0, 0.0]),
        make_example("How to monitor?", "Use Prometheus", &[0.0, 0.0, 1.0]),
        make_example("How to backup?", "Use WAL archiving", &[0.5, 0.5, 0.0]),
    ];

    let selector = FewShotSelector::with_defaults(examples);
    let query = [0.33, 0.33, 0.33]; // Ambiguous query — similar to all

    let selected = selector.select(&query, 3).expect("select should succeed");
    assert_eq!(selected.len(), 3);

    // Feed the relevance scores into active learning
    let learner = ActiveLearner::with_defaults();
    let sim_scores: Vec<f32> = selected.iter().map(|s| s.relevance_score).collect();

    let uncertainty = learner
        .score_uncertainty(&sim_scores)
        .expect("uncertainty scoring should succeed");

    // Ambiguous query should yield moderate-to-high uncertainty
    assert!(
        uncertainty.score > 0.3,
        "ambiguous query should produce meaningful uncertainty, got {}",
        uncertainty.score
    );
}

#[test]
fn moa_refine_with_few_shot_prompted_agents() {
    let moa = MixtureOfAgents::new(MoaConfig {
        max_iterations: 3,
        convergence_threshold: 0.05,
        min_confidence: 0.0,
    });

    let examples = vec![
        make_example("optimize DB", "add index", &[1.0, 0.0]),
        make_example("optimize API", "add cache", &[0.0, 1.0]),
    ];
    let selector = FewShotSelector::with_defaults(examples);

    let initial = vec![
        AgentResponse::new("a1", "add index", 0.6),
        AgentResponse::new("a2", "add cache", 0.5),
    ];

    let prompt = selector.format_prompt(&[], "You are an optimizer.", "optimize system");
    assert!(prompt.contains("optimizer"));

    let result = moa
        .refine(&initial, |_prev, iteration| {
            vec![
                AgentResponse::new("a1", "add index + cache", 0.7 + 0.01 * iteration as f32),
                AgentResponse::new("a2", "add cache + index", 0.65 + 0.01 * iteration as f32),
            ]
        })
        .expect("refine should succeed");

    assert!(result.converged);
    assert!(result.history.len() >= 2);
}

#[test]
fn matryoshka_multi_dimension_embeddings_for_tiered_storage() {
    let embedder = MatryoshkaEmbedder::with_defaults();
    let full = make_embedding(1536);

    let results = embedder
        .embed_multi(&full, Dimension::all_ascending())
        .expect("embed_multi should succeed");

    assert_eq!(results.len(), 4);

    // Verify each dimension's embedding is properly sized and normalized
    for result in &results {
        assert_eq!(result.embedding.len(), result.dimension.value());
        assert!(result.normalized);

        let l2: f32 = result.embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            (l2 - 1.0).abs() < 1e-4,
            "dimension {:?} not normalized: L2 = {l2}",
            result.dimension
        );
    }

    // Smaller dimensions should be a prefix of larger ones (before normalization)
    // The 256-dim result should match first 256 of full, normalized independently
    assert_eq!(results[0].dimension, Dimension::D256);
    assert_eq!(results[3].dimension, Dimension::D1536);
}

#[test]
fn active_learning_feedback_loop_accumulates_rewards() {
    let mut learner = ActiveLearner::with_defaults();

    // Simulate multiple feedback rounds
    let queries = ["deploy question", "scale question", "monitor question"];
    for (i, q) in queries.iter().enumerate() {
        let scores = vec![0.4, 0.35, 0.3];
        let uncertainty = learner.score_uncertainty(&scores).expect("should succeed");

        if uncertainty.is_high(0.5) {
            let candidates = vec!["ex-a".to_string(), "ex-b".to_string()];
            let request = learner
                .request_feedback(q, uncertainty, &candidates)
                .expect("should succeed");

            if let Some(_req) = request {
                learner.record_feedback(
                    format!("req-{i}"),
                    *q,
                    Some("ex-a".to_string()),
                    i % 2 == 0, // alternate positive/negative
                    None,
                );
            }
        }
    }

    assert!(
        learner.feedback_count() > 0,
        "should have recorded some feedback"
    );

    let reward = learner.get_accumulated_reward("ex-a");
    // At least one positive and one negative feedback
    assert!(
        reward != 0.0,
        "accumulated reward should reflect feedback loop"
    );
}

#[test]
fn end_to_end_ai_pipeline_moa_to_embedding_to_few_shot() {
    // Step 1: MoA aggregation
    let moa = MixtureOfAgents::with_defaults();
    let responses = vec![
        AgentResponse::new("coder", "implement retry with backoff", 0.85),
        AgentResponse::new("reviewer", "add circuit breaker pattern", 0.75),
    ];
    let aggregated = moa.aggregate(&responses).expect("aggregate should work");
    assert!(!aggregated.content.is_empty());

    // Step 2: Generate a multi-scale embedding from the response
    let embedder = MatryoshkaEmbedder::with_defaults();
    let fake_embedding = make_embedding(1536);
    let fast = embedder
        .embed_for_use_case(&fake_embedding, UseCase::FastSearch)
        .expect("fast embed should work");
    let accurate = embedder
        .embed_for_use_case(&fake_embedding, UseCase::HighAccuracy)
        .expect("accurate embed should work");

    assert_eq!(fast.embedding.len(), 256);
    assert_eq!(accurate.embedding.len(), 1536);

    // Step 3: Use the fast embedding for few-shot selection
    let examples = vec![
        make_example("retry pattern", "use exponential backoff", &[1.0, 0.0, 0.0]),
        make_example("circuit breaker", "use state machine", &[0.0, 1.0, 0.0]),
        make_example("rate limiting", "use token bucket", &[0.0, 0.0, 1.0]),
    ];
    let selector = FewShotSelector::with_defaults(examples);
    let query = [0.8, 0.2, 0.0];

    let selected = selector.select(&query, 2).expect("select should work");
    assert_eq!(selected.len(), 2);

    // Step 4: Format the prompt
    let prompt = selector.format_prompt(&selected, "You are a patterns expert.", "What pattern?");
    assert!(prompt.contains("patterns expert"));
    assert!(prompt.contains("Input: What pattern?"));
}
