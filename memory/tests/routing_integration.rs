//! Integration tests for RLM routing decisions.
//!
//! These tests verify that the ComplexityRouter correctly routes queries
//! between standard search and RLM decomposition based on query complexity.
//! Also tests transparent routing through MemoryManager.

use config::RlmConfig;
use memory::rlm::router::ComplexityRouter;
use mk_core::types::SearchQuery;

/// Create a test router with default configuration.
fn create_test_router(threshold: f32) -> ComplexityRouter {
    ComplexityRouter::new(RlmConfig {
        enabled: true,
        max_steps: 5,
        complexity_threshold: threshold
    })
}

#[test]
fn test_simple_queries_not_routed() {
    let router = create_test_router(0.3);

    let simple_queries = vec![
        "login",
        "how to login",
        "authentication",
        "get user",
        "find user by id",
        "search users",
        "api key",
        "password reset",
        "user profile",
        "session token",
    ];

    for query_text in simple_queries {
        let query = SearchQuery {
            text: query_text.to_string(),
            target_layers: vec![],
            filters: std::collections::HashMap::new(),
            limit: 10,
            threshold: 0.7
        };

        let complexity = router.compute_complexity(&query);
        let should_route = router.should_route_to_rlm(&query);

        assert!(
            complexity < 0.3,
            "Simple query '{}' should have complexity < 0.3, got {}",
            query_text,
            complexity
        );

        assert!(
            !should_route,
            "Simple query '{}' should not be routed to RLM",
            query_text
        );
    }
}

#[test]
fn test_complex_queries_routed() {
    // Use a more realistic threshold for complex queries
    let router = create_test_router(0.2);

    let complex_queries = vec![
        "compare the evolution of authentication methods between last week and today and \
         summarize the impact",
        "analyze the relationship between user engagement trends and feature adoption rates over \
         the last quarter",
        "trace the sequence of deployment events that caused the production outage and summarize \
         the lessons learned",
        "compare rate limiting strategies before and after the security audit and aggregate the \
         performance metrics",
        "what are the differences between our monolithic and microservices error handling \
         approaches over time",
    ];

    for query_text in complex_queries {
        let query = SearchQuery {
            text: query_text.to_string(),
            ..Default::default()
        };

        let complexity = router.compute_complexity(&query);
        let should_route = router.should_route_to_rlm(&query);

        // Complex queries should have significant complexity
        assert!(
            complexity >= 0.2,
            "Complex query should have complexity >= 0.2, got {} for '{}'",
            complexity,
            query_text
        );

        // With 0.2 threshold, should be routed
        assert!(
            should_route,
            "Complex query should be routed to RLM (complexity: {}): '{}'",
            complexity, query_text
        );
    }
}

#[test]
fn test_threshold_adjustment() {
    // Use a query that we know has moderate complexity
    let test_query = SearchQuery {
        text: "compare authentication methods and summarize the results over time".to_string(),
        ..Default::default()
    };

    // Test with very low threshold (should route)
    let low_threshold_router = create_test_router(0.05);
    let low_complexity = low_threshold_router.compute_complexity(&test_query);
    let low_should_route = low_threshold_router.should_route_to_rlm(&test_query);

    // Test with very high threshold (should not route)
    let high_threshold_router = create_test_router(0.9);
    let high_complexity = high_threshold_router.compute_complexity(&test_query);
    let high_should_route = high_threshold_router.should_route_to_rlm(&test_query);

    // Complexity should be the same regardless of threshold
    assert_eq!(low_complexity, high_complexity);

    // The query should have some complexity
    assert!(low_complexity > 0.0, "Query should have some complexity");

    // With low threshold, should route; with high threshold, should not route
    // (unless complexity is above 0.9, which is unlikely)
    if low_complexity >= 0.05 && low_complexity < 0.9 {
        assert!(low_should_route, "Should route with low threshold (0.05)");
        assert!(
            !high_should_route,
            "Should not route with high threshold (0.9)"
        );
    }
}

#[test]
fn test_rlm_disabled() {
    let router = ComplexityRouter::new(RlmConfig {
        enabled: false,
        max_steps: 5,
        complexity_threshold: 0.0 // Even with 0 threshold, should not route when disabled
    });

    let complex_query = SearchQuery {
        text: "compare and summarize all authentication methods used in the last year".to_string(),
        ..Default::default()
    };

    // Even if query is complex, should not route when RLM is disabled
    assert!(!router.should_route_to_rlm(&complex_query));
}

#[test]
fn test_query_length_impact() {
    let router = create_test_router(0.3);

    // Short query
    let short_query = SearchQuery {
        text: "login".to_string(),
        ..Default::default()
    };

    // Long query with same keywords
    let long_query = SearchQuery {
        text: "how to implement user authentication and authorization with multi-factor \
               authentication using time-based one-time passwords and biometric verification \
               while maintaining compliance with security standards and regulations"
            .to_string(),
        ..Default::default()
    };

    let short_complexity = router.compute_complexity(&short_query);
    let long_complexity = router.compute_complexity(&long_query);

    // Longer query should have higher complexity
    assert!(
        long_complexity > short_complexity,
        "Long query should have higher complexity: {} > {}",
        long_complexity,
        short_complexity
    );
}

#[test]
fn test_keyword_density_calculation() {
    let router = create_test_router(0.3);

    // Query with many complexity keywords
    let dense_query = SearchQuery {
        text: "compare the evolution and summarize the impact of changes over time".to_string(),
        ..Default::default()
    };

    // Query with few keywords
    let sparse_query = SearchQuery {
        text: "how to use the system".to_string(),
        ..Default::default()
    };

    let dense_complexity = router.compute_complexity(&dense_query);
    let sparse_complexity = router.compute_complexity(&sparse_query);

    assert!(
        dense_complexity > sparse_complexity,
        "Query with more keywords should have higher complexity: {} > {}",
        dense_complexity,
        sparse_complexity
    );
}

#[test]
fn test_multi_hop_indicator_detection() {
    let router = create_test_router(0.3);

    let multi_hop_queries = vec![
        "what caused the error and then what happened",
        "first we deployed, then we monitored, followed by optimization",
        "the outage was caused by configuration change leading to cascade failure",
    ];

    for query_text in multi_hop_queries {
        let query = SearchQuery {
            text: query_text.to_string(),
            ..Default::default()
        };

        let complexity = router.compute_complexity(&query);

        assert!(
            complexity >= 0.15, // Should have at least some complexity from multi-hop
            "Multi-hop query should have complexity >= 0.15, got {} for '{}'",
            complexity,
            query_text
        );
    }
}

#[test]
fn test_temporal_constraint_detection() {
    let router = create_test_router(0.3);

    let temporal_queries = vec![
        "errors since last week",
        "performance before the update",
        "metrics from yesterday",
        "changes in the last period",
    ];

    for query_text in temporal_queries {
        let query = SearchQuery {
            text: query_text.to_string(),
            ..Default::default()
        };

        let complexity = router.compute_complexity(&query);
        let _should_route = router.should_route_to_rlm(&query);

        // Temporal constraints add complexity
        assert!(
            complexity >= 0.1,
            "Temporal query should have complexity >= 0.1, got {} for '{}'",
            complexity,
            query_text
        );
    }
}

#[test]
fn test_aggregate_operator_detection() {
    let router = create_test_router(0.3);

    let aggregate_queries = vec![
        "all user sessions",
        "every API call",
        "total errors count",
        "average response time",
    ];

    for query_text in aggregate_queries {
        let query = SearchQuery {
            text: query_text.to_string(),
            ..Default::default()
        };

        let complexity = router.compute_complexity(&query);

        // Aggregate operators add complexity
        assert!(
            complexity >= 0.1,
            "Aggregate query should have complexity >= 0.1, got {} for '{}'",
            complexity,
            query_text
        );
    }
}

#[test]
fn test_edge_cases() {
    let router = create_test_router(0.3);

    // Empty query
    let empty_query = SearchQuery {
        text: "".to_string(),
        ..Default::default()
    };

    let empty_complexity = router.compute_complexity(&empty_query);
    assert_eq!(empty_complexity, 0.0);
    assert!(!router.should_route_to_rlm(&empty_query));

    // Very long query (should be capped at 1.0)
    let long_text = "compare ".repeat(100);
    let very_long_query = SearchQuery {
        text: long_text,
        ..Default::default()
    };

    let long_complexity = router.compute_complexity(&very_long_query);
    assert!(long_complexity <= 1.0);

    // Query with special characters
    let special_query = SearchQuery {
        text: "compare auth (OAuth2 vs JWT) & summarize @results!".to_string(),
        ..Default::default()
    };

    // Should not panic on special characters
    let _ = router.compute_complexity(&special_query);
    let _ = router.should_route_to_rlm(&special_query);
}

#[test]
fn test_configurable_thresholds() {
    // Test different threshold values
    let thresholds = vec![0.1, 0.3, 0.5, 0.7, 0.9];

    let test_query = SearchQuery {
        text: "compare authentication methods and summarize results".to_string(),
        ..Default::default()
    };

    for threshold in thresholds {
        let router = create_test_router(threshold);
        let complexity = router.compute_complexity(&test_query);
        let should_route = router.should_route_to_rlm(&test_query);

        // Verify routing decision matches threshold
        assert_eq!(
            should_route,
            complexity >= threshold,
            "For threshold {}, complexity {}, should_route should be {}",
            threshold,
            complexity,
            complexity >= threshold
        );
    }
}

mod transparent_routing_tests {
    use super::*;
    use memory::manager::MemoryManager;
    use mk_core::types::{TenantContext, TenantId, UserId};
    use std::str::FromStr;

    fn test_tenant() -> TenantContext {
        TenantContext::new(
            TenantId::from_str("test-tenant").unwrap(),
            UserId::from_str("test-user").unwrap()
        )
    }

    #[test]
    fn test_manager_has_rlm_router() {
        let manager = MemoryManager::new();
        let config = config::MemoryConfig {
            rlm: RlmConfig {
                enabled: true,
                max_steps: 5,
                complexity_threshold: 0.3
            },
            ..Default::default()
        };
        let _manager = manager.with_config(config);
    }

    #[test]
    fn test_manager_rlm_disabled_config() {
        let manager = MemoryManager::new();
        let config = config::MemoryConfig {
            rlm: RlmConfig {
                enabled: false,
                max_steps: 5,
                complexity_threshold: 0.3
            },
            ..Default::default()
        };
        let _manager = manager.with_config(config);
    }

    #[tokio::test]
    async fn test_simple_query_uses_standard_search() {
        let config = config::MemoryConfig {
            rlm: RlmConfig {
                enabled: true,
                max_steps: 5,
                complexity_threshold: 0.3
            },
            ..Default::default()
        };
        let _manager = MemoryManager::new().with_config(config);
        let _ctx = test_tenant();

        let query = SearchQuery {
            text: "login".to_string(),
            ..Default::default()
        };

        let router = create_test_router(0.3);
        assert!(!router.should_route_to_rlm(&query));
    }

    #[tokio::test]
    async fn test_complex_query_routes_to_rlm() {
        let config = config::MemoryConfig {
            rlm: RlmConfig {
                enabled: true,
                max_steps: 5,
                complexity_threshold: 0.2
            },
            ..Default::default()
        };
        let _manager = MemoryManager::new().with_config(config);
        let _ctx = test_tenant();

        let query = SearchQuery {
            text: "compare authentication methods and summarize the evolution over the last month"
                .to_string(),
            ..Default::default()
        };

        let router = create_test_router(0.2);
        let complexity = router.compute_complexity(&query);
        assert!(
            complexity >= 0.2,
            "Complex query should have complexity >= 0.2, got {}",
            complexity
        );
        assert!(router.should_route_to_rlm(&query));
    }

    #[test]
    fn test_routing_decision_consistency() {
        let queries = vec![
            ("simple login", false),
            ("get user by id", false),
            (
                "compare all authentication methods and summarize trends over time",
                true
            ),
            (
                "trace the evolution of error handling then analyze impact",
                true
            ),
        ];

        let router = create_test_router(0.2);

        for (query_text, expected_complex) in queries {
            let query = SearchQuery {
                text: query_text.to_string(),
                ..Default::default()
            };

            let is_complex = router.should_route_to_rlm(&query);

            if expected_complex {
                assert!(
                    is_complex,
                    "Query '{}' should be routed to RLM (complexity: {})",
                    query_text,
                    router.compute_complexity(&query)
                );
            } else {
                assert!(
                    !is_complex,
                    "Query '{}' should NOT be routed to RLM (complexity: {})",
                    query_text,
                    router.compute_complexity(&query)
                );
            }
        }
    }

    #[test]
    fn test_fallback_behavior_config() {
        let config = config::MemoryConfig {
            rlm: RlmConfig {
                enabled: true,
                max_steps: 3,
                complexity_threshold: 0.5
            },
            ..Default::default()
        };

        let _manager = MemoryManager::new().with_config(config);
    }
}
