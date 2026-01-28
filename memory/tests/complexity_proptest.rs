use memory::rlm::ComplexityRouter;
use mk_core::types::SearchQuery;
use proptest::prelude::*;

fn make_router() -> ComplexityRouter {
    ComplexityRouter::new(config::RlmConfig {
        enabled: true,
        max_steps: 5,
        complexity_threshold: 0.3
    })
}

proptest! {
    #[test]
    fn complexity_score_bounded_zero_to_one(query in "\\PC{1,500}") {
        let router = make_router();
        let search_query = SearchQuery {
            text: query,
            ..Default::default()
        };
        let score = router.compute_complexity(&search_query);

        prop_assert!(score >= 0.0, "Score {} should be >= 0.0", score);
        prop_assert!(score <= 1.0, "Score {} should be <= 1.0", score);
    }

    #[test]
    fn empty_query_has_zero_complexity(query in "") {
        let router = make_router();
        let search_query = SearchQuery {
            text: query,
            ..Default::default()
        };
        let score = router.compute_complexity(&search_query);

        prop_assert!(score < 0.1, "Empty query should have low complexity: {}", score);
    }

    #[test]
    fn longer_queries_not_less_complex(
        short in "[a-z ]{5,20}",
        extra in "[a-z ]{50,100}"
    ) {
        let router = make_router();

        let short_query = SearchQuery {
            text: short.clone(),
            ..Default::default()
        };
        let long_query = SearchQuery {
            text: format!("{} {}", short, extra),
            ..Default::default()
        };

        let short_score = router.compute_complexity(&short_query);
        let long_score = router.compute_complexity(&long_query);

        prop_assert!(
            long_score >= short_score - 0.01,
            "Longer query ({}) score {} should be >= shorter query ({}) score {} (allowing 0.01 tolerance)",
            long_query.text.len(), long_score, short_query.text.len(), short_score
        );
    }

    #[test]
    fn keyword_increases_complexity(base in "[a-z]{5,15}") {
        let router = make_router();

        let keywords = ["compare", "summarize", "analyze", "trends", "evolution"];

        for keyword in keywords {
            let without_keyword = SearchQuery {
                text: base.clone(),
                ..Default::default()
            };
            let with_keyword = SearchQuery {
                text: format!("{} {} something", base, keyword),
                ..Default::default()
            };

            let score_without = router.compute_complexity(&without_keyword);
            let score_with = router.compute_complexity(&with_keyword);

            prop_assert!(
                score_with >= score_without,
                "Adding '{}' should not decrease complexity: {} vs {}",
                keyword, score_with, score_without
            );
        }
    }

    #[test]
    fn routing_decision_consistent_with_threshold(query in "\\PC{1,200}") {
        let router = make_router();
        let search_query = SearchQuery {
            text: query,
            ..Default::default()
        };

        let score = router.compute_complexity(&search_query);
        let should_route = router.should_route_to_rlm(&search_query);

        let expected = score >= 0.3;
        prop_assert_eq!(
            should_route, expected,
            "Routing decision should match threshold: score={}, should_route={}, expected={}",
            score, should_route, expected
        );
    }

    #[test]
    fn deterministic_scoring(query in "[a-zA-Z0-9 ]{10,100}") {
        let router = make_router();
        let search_query = SearchQuery {
            text: query,
            ..Default::default()
        };

        let score1 = router.compute_complexity(&search_query);
        let score2 = router.compute_complexity(&search_query);

        prop_assert_eq!(
            score1, score2,
            "Same query should always produce same score"
        );
    }

    #[test]
    fn temporal_constraints_increase_score(base in "[a-z]{5,15}") {
        let router = make_router();

        let temporal_phrases = ["last week", "since yesterday", "over the last month"];

        for phrase in temporal_phrases {
            let without = SearchQuery {
                text: base.clone(),
                ..Default::default()
            };
            let with = SearchQuery {
                text: format!("{} {}", base, phrase),
                ..Default::default()
            };

            let score_without = router.compute_complexity(&without);
            let score_with = router.compute_complexity(&with);

            prop_assert!(
                score_with >= score_without,
                "Temporal phrase '{}' should not decrease score: {} vs {}",
                phrase, score_with, score_without
            );
        }
    }

    #[test]
    fn aggregate_operators_increase_score(base in "[a-z]{5,15}") {
        let router = make_router();

        let operators = ["all", "every", "total", "average", "count"];

        for op in operators {
            let without = SearchQuery {
                text: base.clone(),
                ..Default::default()
            };
            let with = SearchQuery {
                text: format!("{} {} items", base, op),
                ..Default::default()
            };

            let score_without = router.compute_complexity(&without);
            let score_with = router.compute_complexity(&with);

            prop_assert!(
                score_with >= score_without,
                "Aggregate operator '{}' should not decrease score: {} vs {}",
                op, score_with, score_without
            );
        }
    }
}

#[test]
fn complex_query_exceeds_threshold() {
    let router = make_router();

    let complex_queries = vec![
        "compare all authentication patterns across every team and summarize trends since last \
         quarter",
        "analyze the evolution of database decisions and compare their total impact on \
         performance over time",
        "aggregate every security finding then compare and summarize them with last month's \
         average results",
    ];

    for query_text in complex_queries {
        let query = SearchQuery {
            text: query_text.to_string(),
            ..Default::default()
        };

        let score = router.compute_complexity(&query);
        assert!(
            score >= 0.3,
            "Complex query '{}' should exceed threshold: {}",
            query_text,
            score
        );
        assert!(
            router.should_route_to_rlm(&query),
            "Complex query should route to RLM"
        );
    }
}

#[test]
fn simple_query_below_threshold() {
    let router = make_router();

    let simple_queries = vec!["show config", "list users", "get endpoint", "find file"];

    for query_text in simple_queries {
        let query = SearchQuery {
            text: query_text.to_string(),
            ..Default::default()
        };

        let score = router.compute_complexity(&query);
        assert!(
            score < 0.3,
            "Simple query '{}' should be below threshold: {}",
            query_text,
            score
        );
        assert!(
            !router.should_route_to_rlm(&query),
            "Simple query should NOT route to RLM"
        );
    }
}
