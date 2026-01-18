use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use mk_core::types::{CodeChange, Resolution};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{info, instrument, warn};

#[derive(Debug, Clone)]
pub struct ResolutionTrackerConfig {
    pub default_success_rate: f32,
    pub min_success_rate: f32,
    pub max_success_rate: f32,
    pub promotion_threshold_applications: u32,
    pub promotion_threshold_success_rate: f32
}

impl Default for ResolutionTrackerConfig {
    fn default() -> Self {
        Self {
            default_success_rate: 1.0,
            min_success_rate: 0.0,
            max_success_rate: 1.0,
            promotion_threshold_applications: 5,
            promotion_threshold_success_rate: 0.8
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResolutionOutcome {
    Success,
    Failure
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolutionMetrics {
    pub resolution_id: String,
    pub error_signature_id: String,
    pub success_count: u32,
    pub failure_count: u32,
    pub last_success_at: Option<i64>,
    pub last_failure_at: Option<i64>,
    pub failure_contexts: Vec<FailureContext>,
    pub created_at: i64
}

impl ResolutionMetrics {
    pub fn new(resolution_id: impl Into<String>, error_signature_id: impl Into<String>) -> Self {
        let now = current_timestamp();
        Self {
            resolution_id: resolution_id.into(),
            error_signature_id: error_signature_id.into(),
            success_count: 0,
            failure_count: 0,
            last_success_at: None,
            last_failure_at: None,
            failure_contexts: Vec::new(),
            created_at: now
        }
    }

    pub fn application_count(&self) -> u32 {
        self.success_count + self.failure_count
    }

    pub fn success_rate(&self) -> f32 {
        let total = self.application_count();
        if total == 0 {
            return 1.0;
        }
        self.success_count as f32 / total as f32
    }

    pub fn record_success(&mut self) {
        self.success_count = self.success_count.saturating_add(1);
        self.last_success_at = Some(current_timestamp());
    }

    pub fn record_failure(&mut self, context: Option<FailureContext>) {
        self.failure_count = self.failure_count.saturating_add(1);
        self.last_failure_at = Some(current_timestamp());
        if let Some(ctx) = context {
            if self.failure_contexts.len() < 10 {
                self.failure_contexts.push(ctx);
            }
        }
    }

    pub fn should_promote(&self, config: &ResolutionTrackerConfig) -> bool {
        self.application_count() >= config.promotion_threshold_applications
            && self.success_rate() >= config.promotion_threshold_success_rate
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureContext {
    pub timestamp: i64,
    pub error_message: Option<String>,
    pub stack_trace: Option<String>,
    pub session_id: Option<String>
}

impl FailureContext {
    pub fn new() -> Self {
        Self {
            timestamp: current_timestamp(),
            error_message: None,
            stack_trace: None,
            session_id: None
        }
    }

    pub fn with_error(mut self, msg: impl Into<String>) -> Self {
        self.error_message = Some(msg.into());
        self
    }

    pub fn with_stack_trace(mut self, trace: impl Into<String>) -> Self {
        self.stack_trace = Some(trace.into());
        self
    }

    pub fn with_session(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }
}

impl Default for FailureContext {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplicationRecord {
    pub resolution_id: String,
    pub outcome: ResolutionOutcome,
    pub timestamp: i64,
    pub context: Option<ApplicationContext>
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplicationContext {
    pub session_id: Option<String>,
    pub error_id: Option<String>,
    pub notes: Option<String>
}

#[derive(Debug, Error)]
pub enum ResolutionStorageError {
    #[error("Resolution not found: {0}")]
    NotFound(String),
    #[error("Storage error: {0}")]
    Storage(String),
    #[error("Serialization error: {0}")]
    Serialization(String)
}

#[async_trait]
pub trait ResolutionStorage: Send + Sync {
    async fn get_metrics(
        &self,
        resolution_id: &str
    ) -> Result<Option<ResolutionMetrics>, ResolutionStorageError>;

    async fn save_metrics(&self, metrics: &ResolutionMetrics)
    -> Result<(), ResolutionStorageError>;

    async fn get_metrics_by_error(
        &self,
        error_signature_id: &str
    ) -> Result<Vec<ResolutionMetrics>, ResolutionStorageError>;

    async fn record_application(
        &self,
        record: &ApplicationRecord
    ) -> Result<(), ResolutionStorageError>;

    async fn get_applications(
        &self,
        resolution_id: &str,
        limit: usize
    ) -> Result<Vec<ApplicationRecord>, ResolutionStorageError>;
}

pub struct InMemoryResolutionStorage {
    metrics: std::sync::RwLock<HashMap<String, ResolutionMetrics>>,
    applications: std::sync::RwLock<Vec<ApplicationRecord>>
}

impl InMemoryResolutionStorage {
    pub fn new() -> Self {
        Self {
            metrics: std::sync::RwLock::new(HashMap::new()),
            applications: std::sync::RwLock::new(Vec::new())
        }
    }
}

impl Default for InMemoryResolutionStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ResolutionStorage for InMemoryResolutionStorage {
    async fn get_metrics(
        &self,
        resolution_id: &str
    ) -> Result<Option<ResolutionMetrics>, ResolutionStorageError> {
        let metrics = self
            .metrics
            .read()
            .map_err(|e| ResolutionStorageError::Storage(format!("Lock poisoned: {e}")))?;
        Ok(metrics.get(resolution_id).cloned())
    }

    async fn save_metrics(
        &self,
        metrics: &ResolutionMetrics
    ) -> Result<(), ResolutionStorageError> {
        let mut store = self
            .metrics
            .write()
            .map_err(|e| ResolutionStorageError::Storage(format!("Lock poisoned: {e}")))?;
        store.insert(metrics.resolution_id.clone(), metrics.clone());
        Ok(())
    }

    async fn get_metrics_by_error(
        &self,
        error_signature_id: &str
    ) -> Result<Vec<ResolutionMetrics>, ResolutionStorageError> {
        let metrics = self
            .metrics
            .read()
            .map_err(|e| ResolutionStorageError::Storage(format!("Lock poisoned: {e}")))?;
        Ok(metrics
            .values()
            .filter(|m| m.error_signature_id == error_signature_id)
            .cloned()
            .collect())
    }

    async fn record_application(
        &self,
        record: &ApplicationRecord
    ) -> Result<(), ResolutionStorageError> {
        let mut apps = self
            .applications
            .write()
            .map_err(|e| ResolutionStorageError::Storage(format!("Lock poisoned: {e}")))?;
        apps.push(record.clone());
        Ok(())
    }

    async fn get_applications(
        &self,
        resolution_id: &str,
        limit: usize
    ) -> Result<Vec<ApplicationRecord>, ResolutionStorageError> {
        let apps = self
            .applications
            .read()
            .map_err(|e| ResolutionStorageError::Storage(format!("Lock poisoned: {e}")))?;
        Ok(apps
            .iter()
            .filter(|a| a.resolution_id == resolution_id)
            .take(limit)
            .cloned()
            .collect())
    }
}

pub struct ResolutionTracker<S: ResolutionStorage> {
    cfg: ResolutionTrackerConfig,
    storage: S,
    local_stats: std::sync::RwLock<HashMap<String, (u32, u32)>>
}

impl<S: ResolutionStorage> ResolutionTracker<S> {
    pub fn new(cfg: ResolutionTrackerConfig, storage: S) -> Self {
        Self {
            cfg,
            storage,
            local_stats: std::sync::RwLock::new(HashMap::new())
        }
    }

    pub fn config(&self) -> &ResolutionTrackerConfig {
        &self.cfg
    }

    #[instrument(skip(self), fields(resolution_id = %resolution_id))]
    pub async fn record_application(
        &self,
        resolution_id: &str,
        outcome: ResolutionOutcome,
        context: Option<ApplicationContext>
    ) -> Result<ResolutionMetrics, ResolutionStorageError> {
        let mut metrics = self
            .storage
            .get_metrics(resolution_id)
            .await?
            .ok_or_else(|| ResolutionStorageError::NotFound(resolution_id.to_string()))?;

        match outcome {
            ResolutionOutcome::Success => {
                metrics.record_success();
                info!(
                    resolution_id = %resolution_id,
                    success_rate = metrics.success_rate(),
                    application_count = metrics.application_count(),
                    "Resolution application succeeded"
                );
            }
            ResolutionOutcome::Failure => {
                let failure_ctx = context.as_ref().map(|c| {
                    FailureContext::new()
                        .with_session(c.session_id.clone().unwrap_or_default())
                        .with_error(c.notes.clone().unwrap_or_default())
                });
                metrics.record_failure(failure_ctx);
                warn!(
                    resolution_id = %resolution_id,
                    success_rate = metrics.success_rate(),
                    application_count = metrics.application_count(),
                    "Resolution application failed"
                );
            }
        }

        self.storage.save_metrics(&metrics).await?;

        let record = ApplicationRecord {
            resolution_id: resolution_id.to_string(),
            outcome,
            timestamp: current_timestamp(),
            context
        };
        self.storage.record_application(&record).await?;

        if let Ok(mut stats) = self.local_stats.write() {
            let entry = stats.entry(resolution_id.to_string()).or_insert((0, 0));
            match outcome {
                ResolutionOutcome::Success => entry.0 += 1,
                ResolutionOutcome::Failure => entry.1 += 1
            }
        }

        Ok(metrics)
    }

    pub fn record_outcome(&self, resolution_id: &str, outcome: ResolutionOutcome) {
        if let Ok(mut stats) = self.local_stats.write() {
            let entry = stats.entry(resolution_id.to_string()).or_insert((0, 0));
            match outcome {
                ResolutionOutcome::Success => entry.0 += 1,
                ResolutionOutcome::Failure => entry.1 += 1
            }
        }
    }

    pub fn compute_success_rate(&self, resolution_id: &str) -> f32 {
        let stats = match self.local_stats.read() {
            Ok(s) => s,
            Err(_) => return self.cfg.default_success_rate
        };

        let Some((successes, failures)) = stats.get(resolution_id) else {
            return self.cfg.default_success_rate;
        };

        let total = *successes as f32 + *failures as f32;
        if total == 0.0 {
            return self.cfg.default_success_rate;
        }

        let rate = *successes as f32 / total;
        rate.clamp(self.cfg.min_success_rate, self.cfg.max_success_rate)
    }

    pub fn apply_stats(&self, mut resolution: Resolution) -> Resolution {
        resolution.success_rate = self.compute_success_rate(&resolution.id);
        resolution.application_count = self
            .local_stats
            .read()
            .ok()
            .and_then(|s| s.get(&resolution.id).map(|(s, f)| s + f))
            .unwrap_or(resolution.application_count);
        resolution
    }

    #[instrument(skip(self, changes, resolution_id, error_signature_id, description))]
    pub async fn link_resolution(
        &self,
        resolution_id: impl Into<String>,
        error_signature_id: impl Into<String>,
        description: impl Into<String>,
        changes: Vec<CodeChange>
    ) -> Result<Resolution, ResolutionStorageError> {
        let resolution_id = resolution_id.into();
        let error_signature_id = error_signature_id.into();
        let now = current_timestamp();

        let mut metrics = ResolutionMetrics::new(&resolution_id, &error_signature_id);
        metrics.record_success();

        self.storage.save_metrics(&metrics).await?;

        let record = ApplicationRecord {
            resolution_id: resolution_id.clone(),
            outcome: ResolutionOutcome::Success,
            timestamp: now,
            context: None
        };
        self.storage.record_application(&record).await?;

        if let Ok(mut stats) = self.local_stats.write() {
            stats.insert(resolution_id.clone(), (1, 0));
        }

        info!(
            resolution_id = %resolution_id,
            error_signature_id = %error_signature_id,
            "Linked resolution to error"
        );

        Ok(Resolution {
            id: resolution_id,
            error_signature_id: error_signature_id.into(),
            description: description.into(),
            changes,
            success_rate: 1.0,
            application_count: 1,
            last_success_at: now
        })
    }

    pub fn create_resolution(
        &self,
        id: impl Into<String>,
        error_signature_id: impl Into<String>,
        description: impl Into<String>,
        changes: Vec<CodeChange>,
        last_success_at: i64
    ) -> Resolution {
        Resolution {
            id: id.into(),
            error_signature_id: error_signature_id.into(),
            description: description.into(),
            changes,
            success_rate: self.cfg.default_success_rate,
            application_count: 0,
            last_success_at
        }
    }

    pub async fn get_metrics(
        &self,
        resolution_id: &str
    ) -> Result<Option<ResolutionMetrics>, ResolutionStorageError> {
        self.storage.get_metrics(resolution_id).await
    }

    pub async fn get_resolutions_for_error(
        &self,
        error_signature_id: &str
    ) -> Result<Vec<ResolutionMetrics>, ResolutionStorageError> {
        self.storage.get_metrics_by_error(error_signature_id).await
    }

    pub async fn check_promotion_candidates(
        &self,
        error_signature_id: &str
    ) -> Result<Vec<ResolutionMetrics>, ResolutionStorageError> {
        let metrics = self
            .storage
            .get_metrics_by_error(error_signature_id)
            .await?;
        Ok(metrics
            .into_iter()
            .filter(|m| m.should_promote(&self.cfg))
            .collect())
    }

    pub fn extract_changes_from_diff(&self, diff: &str) -> Vec<CodeChange> {
        let mut changes = Vec::new();
        let mut current_path: Option<String> = None;
        let mut current_diff = String::new();

        for line in diff.lines() {
            if let Some(path) = parse_diff_header(line) {
                if let Some(existing_path) = current_path.take() {
                    if !current_diff.trim().is_empty() {
                        changes.push(CodeChange {
                            file_path: existing_path,
                            diff: current_diff.trim_end().to_string(),
                            description: None
                        });
                    }
                }
                current_path = Some(path);
                current_diff.clear();
                continue;
            }

            if current_path.is_some() {
                current_diff.push_str(line);
                current_diff.push('\n');
            }
        }

        if let Some(path) = current_path {
            if !current_diff.trim().is_empty() {
                changes.push(CodeChange {
                    file_path: path,
                    diff: current_diff.trim_end().to_string(),
                    description: None
                });
            }
        }

        changes
    }
}

fn current_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn parse_diff_header(line: &str) -> Option<String> {
    if !line.starts_with("diff --git ") {
        return None;
    }

    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 4 {
        return None;
    }

    let path = parts[3];
    Some(path.trim_start_matches("b/").to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_tracker() -> ResolutionTracker<InMemoryResolutionStorage> {
        ResolutionTracker::new(
            ResolutionTrackerConfig::default(),
            InMemoryResolutionStorage::new()
        )
    }

    #[test]
    fn test_resolution_metrics_new() {
        let metrics = ResolutionMetrics::new("r1", "e1");

        assert_eq!(metrics.resolution_id, "r1");
        assert_eq!(metrics.error_signature_id, "e1");
        assert_eq!(metrics.success_count, 0);
        assert_eq!(metrics.failure_count, 0);
        assert_eq!(metrics.application_count(), 0);
        assert_eq!(metrics.success_rate(), 1.0);
    }

    #[test]
    fn test_resolution_metrics_record_success() {
        let mut metrics = ResolutionMetrics::new("r1", "e1");

        metrics.record_success();

        assert_eq!(metrics.success_count, 1);
        assert!(metrics.last_success_at.is_some());
        assert_eq!(metrics.success_rate(), 1.0);
    }

    #[test]
    fn test_resolution_metrics_record_failure() {
        let mut metrics = ResolutionMetrics::new("r1", "e1");

        metrics.record_failure(Some(FailureContext::new().with_error("test error")));

        assert_eq!(metrics.failure_count, 1);
        assert!(metrics.last_failure_at.is_some());
        assert_eq!(metrics.success_rate(), 0.0);
        assert_eq!(metrics.failure_contexts.len(), 1);
    }

    #[test]
    fn test_resolution_metrics_success_rate_calculation() {
        let mut metrics = ResolutionMetrics::new("r1", "e1");

        metrics.record_success();
        metrics.record_success();
        metrics.record_failure(None);

        assert!((metrics.success_rate() - (2.0 / 3.0)).abs() < 0.001);
        assert_eq!(metrics.application_count(), 3);
    }

    #[test]
    fn test_resolution_metrics_should_promote() {
        let config = ResolutionTrackerConfig {
            promotion_threshold_applications: 5,
            promotion_threshold_success_rate: 0.8,
            ..Default::default()
        };
        let mut metrics = ResolutionMetrics::new("r1", "e1");

        assert!(!metrics.should_promote(&config));

        for _ in 0..5 {
            metrics.record_success();
        }

        assert!(metrics.should_promote(&config));

        // Add 2 failures to drop below 0.8: 5/(5+2) = 0.714
        metrics.record_failure(None);
        metrics.record_failure(None);
        assert!(!metrics.should_promote(&config));
    }

    #[test]
    fn test_failure_context_builder() {
        let ctx = FailureContext::new()
            .with_error("test error")
            .with_stack_trace("at line 1")
            .with_session("sess-123");

        assert_eq!(ctx.error_message, Some("test error".to_string()));
        assert_eq!(ctx.stack_trace, Some("at line 1".to_string()));
        assert_eq!(ctx.session_id, Some("sess-123".to_string()));
    }

    #[test]
    fn test_failure_contexts_limited_to_10() {
        let mut metrics = ResolutionMetrics::new("r1", "e1");

        for i in 0..15 {
            metrics.record_failure(Some(FailureContext::new().with_error(format!("error {i}"))));
        }

        assert_eq!(metrics.failure_contexts.len(), 10);
        assert_eq!(metrics.failure_count, 15);
    }

    #[tokio::test]
    async fn test_in_memory_storage_save_and_get() {
        let storage = InMemoryResolutionStorage::new();
        let metrics = ResolutionMetrics::new("r1", "e1");

        storage.save_metrics(&metrics).await.unwrap();
        let retrieved = storage.get_metrics("r1").await.unwrap();

        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().resolution_id, "r1");
    }

    #[tokio::test]
    async fn test_in_memory_storage_get_by_error() {
        let storage = InMemoryResolutionStorage::new();
        storage
            .save_metrics(&ResolutionMetrics::new("r1", "e1"))
            .await
            .unwrap();
        storage
            .save_metrics(&ResolutionMetrics::new("r2", "e1"))
            .await
            .unwrap();
        storage
            .save_metrics(&ResolutionMetrics::new("r3", "e2"))
            .await
            .unwrap();

        let results = storage.get_metrics_by_error("e1").await.unwrap();

        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_tracker_link_resolution() {
        let tracker = create_tracker();

        let resolution = tracker
            .link_resolution("r1", "e1", "Fix the bug", vec![])
            .await
            .unwrap();

        assert_eq!(resolution.id, "r1");
        assert_eq!(resolution.error_signature_id, "e1");
        assert_eq!(resolution.success_rate, 1.0);
        assert_eq!(resolution.application_count, 1);

        let metrics = tracker.get_metrics("r1").await.unwrap().unwrap();
        assert_eq!(metrics.success_count, 1);
    }

    #[tokio::test]
    async fn test_tracker_record_application_success() {
        let tracker = create_tracker();
        tracker
            .link_resolution("r1", "e1", "desc", vec![])
            .await
            .unwrap();

        let metrics = tracker
            .record_application("r1", ResolutionOutcome::Success, None)
            .await
            .unwrap();

        assert_eq!(metrics.success_count, 2);
        assert_eq!(metrics.success_rate(), 1.0);
    }

    #[tokio::test]
    async fn test_tracker_record_application_failure() {
        let tracker = create_tracker();
        tracker
            .link_resolution("r1", "e1", "desc", vec![])
            .await
            .unwrap();

        let context = ApplicationContext {
            session_id: Some("sess-1".to_string()),
            error_id: None,
            notes: Some("Did not work".to_string())
        };

        let metrics = tracker
            .record_application("r1", ResolutionOutcome::Failure, Some(context))
            .await
            .unwrap();

        assert_eq!(metrics.success_count, 1);
        assert_eq!(metrics.failure_count, 1);
        assert!((metrics.success_rate() - 0.5).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_tracker_check_promotion_candidates() {
        let tracker = create_tracker();
        tracker
            .link_resolution("r1", "e1", "desc", vec![])
            .await
            .unwrap();

        for _ in 0..4 {
            tracker
                .record_application("r1", ResolutionOutcome::Success, None)
                .await
                .unwrap();
        }

        let candidates = tracker.check_promotion_candidates("e1").await.unwrap();

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].resolution_id, "r1");
    }

    #[test]
    fn test_default_success_rate() {
        let tracker = create_tracker();
        let rate = tracker.compute_success_rate("missing");
        assert_eq!(rate, 1.0);
    }

    #[test]
    fn test_success_rate_updates() {
        let tracker = create_tracker();

        tracker.record_outcome("r1", ResolutionOutcome::Success);
        tracker.record_outcome("r1", ResolutionOutcome::Failure);
        tracker.record_outcome("r1", ResolutionOutcome::Success);

        let rate = tracker.compute_success_rate("r1");
        assert!((rate - (2.0 / 3.0)).abs() < 0.001);
    }

    #[test]
    fn test_apply_stats_updates_fields() {
        let tracker = create_tracker();
        tracker.record_outcome("r1", ResolutionOutcome::Success);
        tracker.record_outcome("r1", ResolutionOutcome::Failure);

        let res = Resolution {
            id: "r1".to_string(),
            error_signature_id: "e".to_string(),
            description: "d".to_string(),
            changes: vec![],
            success_rate: 0.0,
            application_count: 0,
            last_success_at: 0
        };

        let updated = tracker.apply_stats(res);
        assert_eq!(updated.application_count, 2);
        assert!(updated.success_rate > 0.0);
    }

    #[test]
    fn test_create_resolution() {
        let tracker = create_tracker();
        let res = tracker.create_resolution("r", "e", "desc", vec![], 123);

        assert_eq!(res.id, "r");
        assert_eq!(res.error_signature_id, "e");
        assert_eq!(res.description, "desc");
        assert_eq!(res.last_success_at, 123);
    }

    #[test]
    fn test_extract_changes_from_diff() {
        let tracker = create_tracker();
        let diff = "diff --git a/src/a.rs b/src/a.rs\n+fn foo() {}\n\n\n";
        let changes = tracker.extract_changes_from_diff(diff);

        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].file_path, "src/a.rs");
        assert!(changes[0].diff.contains("+fn foo"));
    }

    #[test]
    fn test_extract_changes_multiple_files() {
        let tracker = create_tracker();
        let diff = r#"diff --git a/src/a.rs b/src/a.rs
+fn foo() {}
diff --git a/src/b.rs b/src/b.rs
+fn bar() {}
"#;
        let changes = tracker.extract_changes_from_diff(diff);

        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].file_path, "src/a.rs");
        assert_eq!(changes[1].file_path, "src/b.rs");
    }
}
