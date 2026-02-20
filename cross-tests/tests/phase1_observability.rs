use chrono::{Duration, Utc};
use mk_core::types::{TenantId, UserId};
use observability::{
    AnomalyDetector, AnomalyDetectorConfig, AnomalyType, CostConfig, CostTracker, ResourceType,
};

fn test_ctx() -> mk_core::types::TenantContext {
    mk_core::types::TenantContext::new(
        TenantId::new("integ-tenant".to_string()).unwrap(),
        UserId::new("integ-user".to_string()).unwrap(),
    )
}

#[test]
fn cost_tracker_records_multiple_resource_types_and_summarises() {
    let tracker = CostTracker::new(CostConfig::default());
    let ctx = test_ctx();

    tracker.record_embedding_generation(&ctx, 5000, "text-embedding-3-small");
    tracker.record_llm_completion(&ctx, 2000, "gpt-4o");
    tracker.record_storage(&ctx, 1_073_741_824); // 1 GB

    let summary = tracker.get_tenant_summary(
        "integ-tenant",
        Utc::now() - Duration::hours(1),
        Utc::now() + Duration::seconds(1),
    );

    assert!(summary.total_cost > 0.0, "total cost must be positive");
    assert!(
        summary
            .by_resource_type
            .contains_key(&ResourceType::EmbeddingGeneration),
        "summary must break down embedding costs"
    );
    assert!(
        summary
            .by_resource_type
            .contains_key(&ResourceType::LlmCompletion),
        "summary must break down LLM costs"
    );
    assert!(
        summary
            .by_resource_type
            .contains_key(&ResourceType::VectorStorage),
        "summary must break down storage costs"
    );
    assert_eq!(summary.currency, "USD");
}

#[test]
fn cost_tracker_budget_enforcement_lifecycle() {
    let tracker = CostTracker::new(CostConfig::default());
    let ctx = test_ctx();

    // No budget set — never over budget
    assert!(!tracker.is_over_budget("integ-tenant"));

    // Set a tight budget
    tracker.set_budget("integ-tenant", 0.50);
    assert!(!tracker.is_over_budget("integ-tenant"));

    // Exceed it with a large LLM call (~$1.50)
    tracker.record_llm_completion(&ctx, 50_000, "gpt-4");
    assert!(
        tracker.is_over_budget("integ-tenant"),
        "must detect over-budget after large spend"
    );

    // Warning level should be at or above 1.0
    let level = tracker.get_budget_warning_level("integ-tenant");
    assert!(
        (level - 1.0).abs() < f64::EPSILON || level > 0.99,
        "warning level should be capped near 1.0, got {level}"
    );
}

#[test]
fn cost_tracker_summary_respects_time_window() {
    let tracker = CostTracker::new(CostConfig::default());
    let ctx = test_ctx();

    tracker.record_embedding_generation(&ctx, 1000, "ada-002");

    // Query a time window entirely in the past — should see no costs
    let past_summary = tracker.get_tenant_summary(
        "integ-tenant",
        Utc::now() - Duration::days(10),
        Utc::now() - Duration::days(5),
    );
    assert_eq!(
        past_summary.total_cost, 0.0,
        "should not see costs outside time window"
    );
}

#[test]
fn anomaly_detector_detects_spike_after_baseline() {
    let detector = AnomalyDetector::new(AnomalyDetectorConfig {
        window_size: 50,
        stddev_threshold: 2.0,
        min_data_points: 10,
    });

    // Establish baseline with stable values around 100
    for i in 0..20 {
        let result = detector.record_and_detect("latency_ms", 100.0 + (i as f64 % 5.0));
        // Should not be an anomaly during baseline build-up
        if i >= 10 {
            assert!(
                !result.is_anomaly,
                "stable value should not trigger anomaly"
            );
        }
    }

    // Inject a spike
    let spike = detector.record_and_detect("latency_ms", 500.0);
    assert!(
        spike.is_anomaly,
        "500ms should be a spike vs ~100ms baseline"
    );
    let anomaly = spike.anomaly.expect("anomaly detail must be present");
    assert_eq!(anomaly.anomaly_type, AnomalyType::Spike);
}

#[test]
fn anomaly_detector_detects_drop_after_baseline() {
    let detector = AnomalyDetector::new(AnomalyDetectorConfig::default());

    for i in 0..20 {
        detector.record_and_detect("throughput", 1000.0 + (i as f64 * 2.0));
    }

    let drop = detector.record_and_detect("throughput", 100.0);
    assert!(drop.is_anomaly, "sharp drop should be flagged");
    assert_eq!(
        drop.anomaly.expect("must have anomaly detail").anomaly_type,
        AnomalyType::Drop
    );
}

#[test]
fn anomaly_detector_baseline_stats_are_reasonable() {
    let detector = AnomalyDetector::new(AnomalyDetectorConfig::default());

    for i in 0..30 {
        detector.record_and_detect("cpu_pct", 50.0 + (i as f64 % 10.0));
    }

    let baseline = detector
        .get_baseline("cpu_pct")
        .expect("baseline should exist after 30 points");

    assert!(baseline.mean > 50.0 && baseline.mean < 60.0);
    assert!(baseline.stddev > 0.0 && baseline.stddev < 10.0);
    assert_eq!(baseline.data_points, 30);
}

#[test]
fn anomaly_detector_requires_minimum_data_points() {
    let config = AnomalyDetectorConfig {
        min_data_points: 15,
        ..Default::default()
    };
    let detector = AnomalyDetector::new(config);

    // Only 5 points — anomaly detection should not fire even with wild value
    for _ in 0..5 {
        detector.record_and_detect("mem_mb", 100.0);
    }
    let result = detector.record_and_detect("mem_mb", 9999.0);
    assert!(
        !result.is_anomaly,
        "should not detect anomaly with insufficient baseline data"
    );
}

#[test]
fn cost_and_anomaly_cross_integration() {
    let tracker = CostTracker::new(CostConfig::default());
    let detector = AnomalyDetector::new(AnomalyDetectorConfig::default());
    let ctx = test_ctx();

    // Simulate steady cost flow and feed cost amounts into anomaly detector
    for _ in 0..15 {
        tracker.record_embedding_generation(&ctx, 1000, "ada-002");
        let summary = tracker.get_tenant_summary(
            "integ-tenant",
            Utc::now() - Duration::hours(1),
            Utc::now() + Duration::seconds(1),
        );
        detector.record_and_detect("cost_usd", summary.total_cost);
    }

    // Sudden cost spike
    tracker.record_llm_completion(&ctx, 100_000, "gpt-4");
    let summary = tracker.get_tenant_summary(
        "integ-tenant",
        Utc::now() - Duration::hours(1),
        Utc::now() + Duration::seconds(1),
    );
    let result = detector.record_and_detect("cost_usd", summary.total_cost);

    assert!(
        result.is_anomaly,
        "sudden cost spike should trigger anomaly detection"
    );
}
