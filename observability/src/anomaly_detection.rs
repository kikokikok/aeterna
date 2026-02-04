///! # Anomaly Detection Module
///!
///! Statistical anomaly detection for system metrics.
///! Detects unusual patterns in latency, error rates, and resource usage.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};

const DEFAULT_WINDOW_SIZE: usize = 100;
const DEFAULT_STDDEV_THRESHOLD: f64 = 2.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyDetectorConfig {
    /// Number of data points to keep for baseline calculation
    pub window_size: usize,
    /// Number of standard deviations from mean to trigger anomaly
    pub stddev_threshold: f64,
    /// Minimum data points required before detecting anomalies
    pub min_data_points: usize,
}

impl Default for AnomalyDetectorConfig {
    fn default() -> Self {
        Self {
            window_size: DEFAULT_WINDOW_SIZE,
            stddev_threshold: DEFAULT_STDDEV_THRESHOLD,
            min_data_points: 10,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricBaseline {
    pub metric_name: String,
    pub mean: f64,
    pub stddev: f64,
    pub min: f64,
    pub max: f64,
    pub data_points: usize,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Anomaly {
    pub metric_name: String,
    pub timestamp: DateTime<Utc>,
    pub value: f64,
    pub baseline_mean: f64,
    pub baseline_stddev: f64,
    pub deviation: f64,
    pub anomaly_type: AnomalyType,
    pub severity: AnomalySeverity,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AnomalyType {
    Spike,      // Value significantly above normal
    Drop,       // Value significantly below normal
    Sustained,  // Value stays abnormal for multiple periods
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AnomalySeverity {
    Low,     // 2-3 stddev
    Medium,  // 3-4 stddev
    High,    // 4+ stddev
}

pub struct DetectionResult {
    pub is_anomaly: bool,
    pub anomaly: Option<Anomaly>,
}

pub struct AnomalyDetector {
    config: AnomalyDetectorConfig,
    /// Store recent data points for each metric
    data: Arc<RwLock<HashMap<String, VecDeque<f64>>>>,
    /// Store baseline statistics
    baselines: Arc<RwLock<HashMap<String, MetricBaseline>>>,
}

impl AnomalyDetector {
    pub fn new(config: AnomalyDetectorConfig) -> Self {
        Self {
            config,
            data: Arc::new(RwLock::new(HashMap::new())),
            baselines: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add a new data point and check for anomalies
    pub fn record_and_detect(
        &self,
        metric_name: &str,
        value: f64,
    ) -> DetectionResult {
        // Add data point
        {
            let mut data = self.data.write().unwrap();
            let points = data.entry(metric_name.to_string()).or_insert_with(VecDeque::new);
            points.push_back(value);

            // Keep only window_size recent points
            while points.len() > self.config.window_size {
                points.pop_front();
            }
        }

        // Calculate new baseline
        self.update_baseline(metric_name);

        // Detect anomaly
        self.detect_anomaly(metric_name, value)
    }

    fn update_baseline(&self, metric_name: &str) {
        let data = self.data.read().unwrap();
        let points = match data.get(metric_name) {
            Some(p) => p,
            None => return,
        };

        if points.len() < self.config.min_data_points {
            return;
        }

        let values: Vec<f64> = points.iter().copied().collect();
        let mean = Self::calculate_mean(&values);
        let stddev = Self::calculate_stddev(&values, mean);
        let min = values.iter().copied().fold(f64::INFINITY, f64::min);
        let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);

        let baseline = MetricBaseline {
            metric_name: metric_name.to_string(),
            mean,
            stddev,
            min,
            max,
            data_points: values.len(),
            last_updated: Utc::now(),
        };

        self.baselines.write().unwrap().insert(metric_name.to_string(), baseline);
    }

    fn detect_anomaly(&self, metric_name: &str, value: f64) -> DetectionResult {
        let baselines = self.baselines.read().unwrap();
        let baseline = match baselines.get(metric_name) {
            Some(b) => b,
            None => return DetectionResult { is_anomaly: false, anomaly: None },
        };

        if baseline.stddev == 0.0 {
            return DetectionResult { is_anomaly: false, anomaly: None };
        }

        let deviation = ((value - baseline.mean) / baseline.stddev).abs();

        if deviation > self.config.stddev_threshold {
            let anomaly_type = if value > baseline.mean {
                AnomalyType::Spike
            } else {
                AnomalyType::Drop
            };

            let severity = if deviation > 4.0 {
                AnomalySeverity::High
            } else if deviation > 3.0 {
                AnomalySeverity::Medium
            } else {
                AnomalySeverity::Low
            };

            let anomaly = Anomaly {
                metric_name: metric_name.to_string(),
                timestamp: Utc::now(),
                value,
                baseline_mean: baseline.mean,
                baseline_stddev: baseline.stddev,
                deviation,
                anomaly_type,
                severity,
            };

            tracing::warn!(
                "Anomaly detected in {}: value={}, baseline={:.2}, deviation={:.2}σ",
                metric_name,
                value,
                baseline.mean,
                deviation
            );

            DetectionResult {
                is_anomaly: true,
                anomaly: Some(anomaly),
            }
        } else {
            DetectionResult { is_anomaly: false, anomaly: None }
        }
    }

    pub fn get_baseline(&self, metric_name: &str) -> Option<MetricBaseline> {
        self.baselines.read().unwrap().get(metric_name).cloned()
    }

    fn calculate_mean(values: &[f64]) -> f64 {
        if values.is_empty() {
            return 0.0;
        }
        values.iter().sum::<f64>() / values.len() as f64
    }

    fn calculate_stddev(values: &[f64], mean: f64) -> f64 {
        if values.len() < 2 {
            return 0.0;
        }
        let variance: f64 = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>()
            / (values.len() - 1) as f64;
        variance.sqrt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anomaly_detection_spike() {
        let detector = AnomalyDetector::new(AnomalyDetectorConfig::default());

        // Add normal data points
        for i in 0..20 {
            detector.record_and_detect("test_metric", 100.0 + i as f64);
        }

        // Add anomalous spike
        let result = detector.record_and_detect("test_metric", 500.0);
        assert!(result.is_anomaly);
        assert_eq!(result.anomaly.unwrap().anomaly_type, AnomalyType::Spike);
    }

    #[test]
    fn test_anomaly_detection_drop() {
        let detector = AnomalyDetector::new(AnomalyDetectorConfig::default());

        // Add normal data points
        for i in 0..20 {
            detector.record_and_detect("test_metric", 100.0 + i as f64);
        }

        // Add anomalous drop
        let result = detector.record_and_detect("test_metric", 10.0);
        assert!(result.is_anomaly);
        assert_eq!(result.anomaly.unwrap().anomaly_type, AnomalyType::Drop);
    }

    #[test]
    fn test_baseline_calculation() {
        let detector = AnomalyDetector::new(AnomalyDetectorConfig::default());

        for i in 0..20 {
            detector.record_and_detect("test_metric", 100.0 + i as f64);
        }

        let baseline = detector.get_baseline("test_metric");
        assert!(baseline.is_some());
        let baseline = baseline.unwrap();
        
        assert!(baseline.mean > 100.0 && baseline.mean < 120.0);
        assert!(baseline.stddev > 0.0);
    }

    #[test]
    fn test_no_anomaly_for_insufficient_data() {
        let config = AnomalyDetectorConfig {
            min_data_points: 10,
            ..Default::default()
        };
        let detector = AnomalyDetector::new(config);

        // Add only 5 points
        for i in 0..5 {
            let result = detector.record_and_detect("test_metric", 100.0 + i as f64);
            assert!(!result.is_anomaly);
        }
    }
}
