use metrics_util::CompositeKey;
use metrics_util::debugging::DebuggingRecorder;
use storage::graph_duckdb::{AlertSeverity, ContentionAlertConfig, GraphMetrics};

type SnapshotVec = Vec<(
    CompositeKey,
    Option<metrics::Unit>,
    Option<metrics::SharedString>,
    metrics_util::debugging::DebugValue
)>;

/// Run test closure with a scoped recorder and return the snapshot
fn with_test_recorder<F, R>(f: F) -> (R, SnapshotVec)
where
    F: FnOnce() -> R
{
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    let result = metrics::with_local_recorder(&recorder, f);
    let snapshot = snapshotter.snapshot().into_vec();

    (result, snapshot)
}

fn has_metric_name(snapshot: &SnapshotVec, name: &str) -> bool {
    snapshot.iter().any(|(k, _, _, _)| k.key().name() == name)
}

fn has_metric_with_labels(snapshot: &SnapshotVec, name: &str, labels: &[(&str, &str)]) -> bool {
    snapshot.iter().any(|(k, _, _, _)| {
        if k.key().name() != name {
            return false;
        }
        let key_labels: Vec<_> = k.key().labels().collect();
        labels.iter().all(|(label_name, label_value)| {
            key_labels
                .iter()
                .any(|l| l.key() == *label_name && l.value() == *label_value)
        })
    })
}

#[test]
fn test_graph_metrics_record_query() {
    let (_, vec) = with_test_recorder(|| {
        let metrics = GraphMetrics::new();
        metrics.record_query(0.5, 10);
    });

    assert!(
        has_metric_name(&vec, "graph_query_duration_seconds"),
        "Should record query duration histogram. Found metrics: {:?}",
        vec.iter()
            .map(|(k, _, _, _)| k.key().name())
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_graph_metrics_record_cache_operations() {
    let (_, vec) = with_test_recorder(|| {
        let metrics = GraphMetrics::new();
        metrics.record_cache_hit();
        metrics.record_cache_hit();
        metrics.record_cache_miss();
    });

    assert!(
        has_metric_name(&vec, "graph_cache_hits_total"),
        "Should record cache hit counter. Found metrics: {:?}",
        vec.iter()
            .map(|(k, _, _, _)| k.key().name())
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_graph_metrics_lock_lifecycle() {
    let (_, vec) = with_test_recorder(|| {
        let metrics = GraphMetrics::new();
        metrics.record_lock_attempt("tenant-1");
        metrics.record_lock_acquired("tenant-1", 100, 2);
        metrics.record_lock_released("tenant-1", 500);
    });

    assert!(
        has_metric_name(&vec, "graph_write_lock_attempts_total"),
        "Should record lock attempts. Found metrics: {:?}",
        vec.iter()
            .map(|(k, _, _, _)| k.key().name())
            .collect::<Vec<_>>()
    );
    assert!(
        has_metric_name(&vec, "graph_write_lock_acquired_total"),
        "Should record lock acquisitions. Found metrics: {:?}",
        vec.iter()
            .map(|(k, _, _, _)| k.key().name())
            .collect::<Vec<_>>()
    );
    assert!(
        has_metric_name(&vec, "graph_write_lock_released_total"),
        "Should record lock releases. Found metrics: {:?}",
        vec.iter()
            .map(|(k, _, _, _)| k.key().name())
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_graph_metrics_lock_timeout() {
    let (_, vec) = with_test_recorder(|| {
        let metrics = GraphMetrics::new();
        metrics.record_lock_attempt("tenant-1");
        metrics.record_lock_timeout("tenant-1", 5000, 5);
    });

    assert!(
        has_metric_name(&vec, "graph_write_lock_timeouts_total"),
        "Should record lock timeouts. Found metrics: {:?}",
        vec.iter()
            .map(|(k, _, _, _)| k.key().name())
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_graph_metrics_wait_time_histograms() {
    let (_, vec) = with_test_recorder(|| {
        let metrics = GraphMetrics::new();
        metrics.record_lock_acquired("tenant-1", 1500, 3);
    });

    assert!(
        has_metric_name(&vec, "graph_write_lock_wait_seconds"),
        "Should record wait time histogram. Found metrics: {:?}",
        vec.iter()
            .map(|(k, _, _, _)| k.key().name())
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_graph_metrics_alert_emission_warn() {
    let alert_config = ContentionAlertConfig {
        queue_depth_warn: 5,
        queue_depth_critical: 10,
        wait_time_warn_ms: 500,
        wait_time_critical_ms: 2000,
        timeout_rate_warn_percent: 5.0,
        timeout_rate_critical_percent: 15.0
    };

    // GIVEN wait_time_warn_ms=500, wait_time_critical_ms=2000
    // WHEN wait time is 800ms (exceeds warn but not critical)
    let (_, vec) = with_test_recorder(|| {
        let metrics = GraphMetrics::with_alert_config(alert_config);
        metrics.record_lock_acquired("tenant-1", 800, 2);
    });

    assert!(
        has_metric_with_labels(
            &vec,
            "graph_contention_alerts_total",
            &[("severity", "warn")]
        ),
        "Should emit warn alert when wait time exceeds warn threshold. Found metrics: {:?}",
        vec.iter()
            .map(|(k, _, _, _)| format!(
                "{} {:?}",
                k.key().name(),
                k.key().labels().collect::<Vec<_>>()
            ))
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_graph_metrics_alert_emission_critical() {
    let alert_config = ContentionAlertConfig {
        queue_depth_warn: 5,
        queue_depth_critical: 10,
        wait_time_warn_ms: 500,
        wait_time_critical_ms: 2000,
        timeout_rate_warn_percent: 5.0,
        timeout_rate_critical_percent: 15.0
    };

    // GIVEN wait_time_critical_ms=2000
    // WHEN wait time is 2500ms (exceeds critical)
    let (_, vec) = with_test_recorder(|| {
        let metrics = GraphMetrics::with_alert_config(alert_config);
        metrics.record_lock_acquired("tenant-1", 2500, 5);
    });

    assert!(
        has_metric_with_labels(
            &vec,
            "graph_contention_alerts_total",
            &[("severity", "critical")]
        ),
        "Should emit critical alert when wait time exceeds critical threshold. Found metrics: {:?}",
        vec.iter()
            .map(|(k, _, _, _)| format!(
                "{} {:?}",
                k.key().name(),
                k.key().labels().collect::<Vec<_>>()
            ))
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_graph_metrics_no_alert_below_threshold() {
    let alert_config = ContentionAlertConfig {
        queue_depth_warn: 5,
        queue_depth_critical: 10,
        wait_time_warn_ms: 1000,
        wait_time_critical_ms: 3000,
        timeout_rate_warn_percent: 5.0,
        timeout_rate_critical_percent: 15.0
    };

    // GIVEN wait_time_warn_ms=1000
    // WHEN wait time is 500ms (below warn threshold)
    let (_, vec) = with_test_recorder(|| {
        let metrics = GraphMetrics::with_alert_config(alert_config);
        metrics.record_lock_acquired("tenant-1", 500, 1);
    });

    let has_alert = has_metric_name(&vec, "graph_contention_alerts_total");
    assert!(
        !has_alert,
        "Should NOT emit alert when wait time is below warn threshold"
    );
}

#[test]
fn test_graph_metrics_default_no_alerts() {
    // GIVEN GraphMetrics::new() (no alert config)
    // WHEN recording high wait time
    let (_, vec) = with_test_recorder(|| {
        let metrics = GraphMetrics::new();
        metrics.record_lock_acquired("tenant-1", 10000, 10);
    });

    let has_alert = has_metric_name(&vec, "graph_contention_alerts_total");
    assert!(
        !has_alert,
        "GraphMetrics::new() should not emit alerts (no config)"
    );
}

#[test]
fn test_alert_severity_enum() {
    assert_eq!(AlertSeverity::Warn, AlertSeverity::Warn);
    assert_eq!(AlertSeverity::Critical, AlertSeverity::Critical);
    assert_ne!(AlertSeverity::Warn, AlertSeverity::Critical);
}
