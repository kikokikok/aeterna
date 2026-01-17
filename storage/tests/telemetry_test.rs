use metrics_util::CompositeKey;
use metrics_util::debugging::{DebuggingRecorder, Snapshotter};
use std::sync::Once;
use storage::graph_duckdb::{AlertSeverity, ContentionAlertConfig, GraphMetrics};

static INIT: Once = Once::new();

fn setup_recorder() {
    INIT.call_once(|| {
        let recorder = DebuggingRecorder::per_thread();
        metrics::set_boxed_recorder(Box::new(recorder)).ok();
    });
}

type SnapshotVec = Vec<(
    CompositeKey,
    Option<metrics::Unit>,
    Option<metrics::SharedString>,
    metrics_util::debugging::DebugValue
)>;

fn get_snapshot_vec() -> SnapshotVec {
    Snapshotter::current_thread_snapshot()
        .map(|s| s.into_vec())
        .unwrap_or_default()
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
    setup_recorder();
    let metrics = GraphMetrics::new();

    metrics.record_query(0.5, 10);

    let vec = get_snapshot_vec();

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
    setup_recorder();
    let metrics = GraphMetrics::new();

    metrics.record_cache_hit();
    metrics.record_cache_hit();
    metrics.record_cache_miss();

    let vec = get_snapshot_vec();

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
    setup_recorder();
    let metrics = GraphMetrics::new();

    metrics.record_lock_attempt("tenant-1");
    metrics.record_lock_acquired("tenant-1", 100, 2);
    metrics.record_lock_released("tenant-1", 500);

    let vec = get_snapshot_vec();

    assert!(
        has_metric_name(&vec, "graph_write_lock_attempts_total"),
        "Should record lock attempts. Found metrics: {:?}",
        vec.iter()
            .map(|(k, _, _, _)| k.key().name())
            .collect::<Vec<_>>()
    );
    assert!(
        has_metric_name(&vec, "graph_write_lock_acquired_total"),
        "Should record lock acquisitions"
    );
    assert!(
        has_metric_name(&vec, "graph_write_lock_released_total"),
        "Should record lock releases"
    );
}

#[test]
fn test_graph_metrics_lock_timeout() {
    setup_recorder();
    let metrics = GraphMetrics::new();

    metrics.record_lock_attempt("tenant-1");
    metrics.record_lock_timeout("tenant-1", 5000, 5);

    let vec = get_snapshot_vec();

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
    setup_recorder();
    let metrics = GraphMetrics::new();

    metrics.record_lock_acquired("tenant-1", 1500, 3);

    let vec = get_snapshot_vec();

    assert!(
        has_metric_name(&vec, "graph_write_lock_wait_seconds"),
        "Should record wait time histogram. Found metrics: {:?}",
        vec.iter()
            .map(|(k, _, _, _)| k.key().name())
            .collect::<Vec<_>>()
    );
    assert!(
        has_metric_name(&vec, "graph_write_lock_retries"),
        "Should record retry count histogram"
    );
}

#[test]
fn test_graph_metrics_hold_time_histogram() {
    setup_recorder();
    let metrics = GraphMetrics::new();

    metrics.record_lock_released("tenant-1", 2500);

    let vec = get_snapshot_vec();

    assert!(
        has_metric_name(&vec, "graph_write_lock_hold_seconds"),
        "Should record hold time histogram. Found metrics: {:?}",
        vec.iter()
            .map(|(k, _, _, _)| k.key().name())
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_graph_metrics_alert_emission_warn() {
    setup_recorder();
    let alert_config = ContentionAlertConfig {
        queue_depth_warn: 5,
        queue_depth_critical: 10,
        wait_time_warn_ms: 500,
        wait_time_critical_ms: 2000,
        timeout_rate_warn_percent: 5.0,
        timeout_rate_critical_percent: 15.0
    };
    let metrics = GraphMetrics::with_alert_config(alert_config);

    // GIVEN wait_time_warn_ms=500, wait_time_critical_ms=2000
    // WHEN wait time is 800ms (exceeds warn but not critical)
    metrics.record_lock_acquired("tenant-1", 800, 2);

    let vec = get_snapshot_vec();

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
    setup_recorder();
    let alert_config = ContentionAlertConfig {
        queue_depth_warn: 5,
        queue_depth_critical: 10,
        wait_time_warn_ms: 500,
        wait_time_critical_ms: 2000,
        timeout_rate_warn_percent: 5.0,
        timeout_rate_critical_percent: 15.0
    };
    let metrics = GraphMetrics::with_alert_config(alert_config);

    // GIVEN wait_time_critical_ms=2000
    // WHEN wait time is 2500ms (exceeds critical)
    metrics.record_lock_acquired("tenant-1", 2500, 5);

    let vec = get_snapshot_vec();

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
    setup_recorder();
    let alert_config = ContentionAlertConfig {
        queue_depth_warn: 5,
        queue_depth_critical: 10,
        wait_time_warn_ms: 1000,
        wait_time_critical_ms: 3000,
        timeout_rate_warn_percent: 5.0,
        timeout_rate_critical_percent: 15.0
    };
    let metrics = GraphMetrics::with_alert_config(alert_config);

    // GIVEN wait_time_warn_ms=1000
    // WHEN wait time is 500ms (below warn threshold)
    metrics.record_lock_acquired("tenant-1", 500, 1);

    let vec = get_snapshot_vec();

    let has_alert = has_metric_name(&vec, "graph_contention_alerts_total");
    assert!(
        !has_alert,
        "Should NOT emit alert when wait time is below warn threshold"
    );
}

#[test]
fn test_graph_metrics_default_no_alerts() {
    setup_recorder();
    let metrics = GraphMetrics::new();

    // GIVEN GraphMetrics::new() (no alert config)
    // WHEN recording high wait time
    metrics.record_lock_acquired("tenant-1", 10000, 10);

    let vec = get_snapshot_vec();

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
