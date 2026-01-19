use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use config::cca::CaptureMode;
use dashmap::DashMap;
use mk_core::types::TenantContext;
use regex::Regex;
use serde::{Deserialize, Serialize};
use storage::postgres::PostgresError;
use tokio::sync::{Mutex, mpsc};
use tracing::{info_span, instrument, warn};

/// Storage adapter trait for trajectory persistence
#[async_trait::async_trait]
pub trait TrajectoryStorage: Send + Sync {
    /// Persist a batch of trajectory events
    async fn persist_events(
        &self,
        ctx: &TenantContext,
        session_id: &str,
        events: &[TrajectoryEvent],
    ) -> Result<(), TrajectoryStorageError>;

    /// Load events for a session
    async fn load_events(
        &self,
        ctx: &TenantContext,
        session_id: &str,
    ) -> Result<Vec<TrajectoryEvent>, TrajectoryStorageError>;
}

#[derive(Debug, thiserror::Error)]
pub enum TrajectoryStorageError {
    #[error("Storage backend error: {0}")]
    Backend(String),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Session not found: {0}")]
    SessionNotFound(String),
}

impl From<PostgresError> for TrajectoryStorageError {
    fn from(e: PostgresError) -> Self {
        TrajectoryStorageError::Backend(e.to_string())
    }
}

impl From<serde_json::Error> for TrajectoryStorageError {
    fn from(e: serde_json::Error) -> Self {
        TrajectoryStorageError::Serialization(e.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrajectoryEvent {
    pub id: String,
    pub timestamp: u64,
    pub tool_name: String,
    pub input: String,
    pub output: String,
    pub success: bool,
    pub duration_ms: u64,
    pub metadata: Option<serde_json::Value>,
}

impl TrajectoryEvent {
    pub fn new(
        tool_name: impl Into<String>,
        input: impl Into<String>,
        output: impl Into<String>,
        success: bool,
        duration_ms: u64,
    ) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Self {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp,
            tool_name: tool_name.into(),
            input: input.into(),
            output: output.into(),
            success,
            duration_ms,
            metadata: None,
        }
    }

    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

#[derive(Debug, Clone)]
pub struct AsyncCaptureMetrics {
    events_captured: Arc<AtomicU64>,
    events_dropped: Arc<AtomicU64>,
    capture_latency_ms_sum: Arc<AtomicU64>,
    batch_flushes: Arc<AtomicU64>,
    overflow_drops: Arc<AtomicU64>,
}

impl AsyncCaptureMetrics {
    pub fn new() -> Self {
        Self {
            events_captured: Arc::new(AtomicU64::new(0)),
            events_dropped: Arc::new(AtomicU64::new(0)),
            capture_latency_ms_sum: Arc::new(AtomicU64::new(0)),
            batch_flushes: Arc::new(AtomicU64::new(0)),
            overflow_drops: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn record_captured(&self, latency_ms: u64) {
        self.events_captured.fetch_add(1, Ordering::Relaxed);
        self.capture_latency_ms_sum
            .fetch_add(latency_ms, Ordering::Relaxed);
    }

    pub fn record_dropped(&self) {
        self.events_dropped.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_overflow_drop(&self) {
        self.overflow_drops.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_batch_flush(&self) {
        self.batch_flushes.fetch_add(1, Ordering::Relaxed);
    }

    pub fn events_captured(&self) -> u64 {
        self.events_captured.load(Ordering::Relaxed)
    }

    pub fn events_dropped(&self) -> u64 {
        self.events_dropped.load(Ordering::Relaxed)
    }

    pub fn capture_latency_sum(&self) -> u64 {
        self.capture_latency_ms_sum.load(Ordering::Relaxed)
    }

    pub fn batch_flushes(&self) -> u64 {
        self.batch_flushes.load(Ordering::Relaxed)
    }

    pub fn overflow_drops(&self) -> u64 {
        self.overflow_drops.load(Ordering::Relaxed)
    }

    pub fn avg_capture_latency_ms(&self) -> f64 {
        let captured = self.events_captured();
        if captured == 0 {
            0.0
        } else {
            self.capture_latency_sum() as f64 / captured as f64
        }
    }
}

impl Default for AsyncCaptureMetrics {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct TrajectoryConfig {
    pub max_events: usize,
    pub max_input_chars: usize,
    pub max_output_chars: usize,
    pub filter_sensitive: bool,
    pub excluded_tools: Vec<String>,
}

impl Default for TrajectoryConfig {
    fn default() -> Self {
        Self {
            max_events: 100,
            max_input_chars: 5000,
            max_output_chars: 10000,
            filter_sensitive: true,
            excluded_tools: vec![],
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SensitivePatterns {
    patterns: Vec<Regex>,
}

impl SensitivePatterns {
    pub fn new() -> Self {
        let default_patterns = vec![
            r#"(?i)(api[_-]?key|apikey)\s*[:=]\s*['\"]?[\w-]+"#,
            r#"(?i)(password|passwd|pwd)\s*[:=]\s*['\"]?[^\s'\""]+"#,
            r#"(?i)(secret|token)\s*[:=]\s*['\"]?[\w-]+"#,
            r#"(?i)bearer\s+[\w-]+\.[\w-]+\.[\w-]+"#,
            r#"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b"#,
        ];

        let patterns = default_patterns
            .iter()
            .filter_map(|p| Regex::new(p).ok())
            .collect();

        Self { patterns }
    }

    pub fn add_pattern(&mut self, pattern: &str) -> Result<(), regex::Error> {
        let regex = Regex::new(pattern)?;
        self.patterns.push(regex);
        Ok(())
    }

    pub fn filter(&self, text: &str) -> String {
        let mut result = text.to_string();
        for pattern in &self.patterns {
            result = pattern.replace_all(&result, "[REDACTED]").to_string();
        }
        result
    }
}

pub struct TrajectoryFilter {
    config: TrajectoryConfig,
    sensitive_patterns: SensitivePatterns,
}

impl TrajectoryFilter {
    pub fn new(config: TrajectoryConfig) -> Self {
        Self {
            config,
            sensitive_patterns: SensitivePatterns::new(),
        }
    }

    pub fn with_patterns(mut self, patterns: SensitivePatterns) -> Self {
        self.sensitive_patterns = patterns;
        self
    }

    pub fn filter_event(&self, event: &TrajectoryEvent) -> Option<TrajectoryEvent> {
        if self.config.excluded_tools.contains(&event.tool_name) {
            return None;
        }

        let mut filtered = event.clone();

        filtered.input = self.truncate(&filtered.input, self.config.max_input_chars);
        filtered.output = self.truncate(&filtered.output, self.config.max_output_chars);

        if self.config.filter_sensitive {
            filtered.input = self.sensitive_patterns.filter(&filtered.input);
            filtered.output = self.sensitive_patterns.filter(&filtered.output);
        }

        Some(filtered)
    }

    fn truncate(&self, text: &str, max_chars: usize) -> String {
        if text.len() <= max_chars {
            text.to_string()
        } else {
            format!("{}... [truncated]", &text[..max_chars])
        }
    }
}

pub struct TrajectoryCapture {
    config: TrajectoryConfig,
    filter: TrajectoryFilter,
    events: VecDeque<TrajectoryEvent>,
}

impl TrajectoryCapture {
    pub fn new(config: TrajectoryConfig) -> Self {
        let filter = TrajectoryFilter::new(config.clone());
        Self {
            config,
            filter,
            events: VecDeque::new(),
        }
    }

    pub fn capture(&mut self, event: TrajectoryEvent) {
        if let Some(filtered) = self.filter.filter_event(&event) {
            self.events.push_back(filtered);

            while self.events.len() > self.config.max_events {
                self.events.pop_front();
            }
        }
    }

    pub fn events(&self) -> &VecDeque<TrajectoryEvent> {
        &self.events
    }

    pub fn events_vec(&self) -> Vec<TrajectoryEvent> {
        self.events.iter().cloned().collect()
    }

    pub fn successful_events(&self) -> Vec<&TrajectoryEvent> {
        self.events.iter().filter(|e| e.success).collect()
    }

    pub fn failed_events(&self) -> Vec<&TrajectoryEvent> {
        self.events.iter().filter(|e| !e.success).collect()
    }

    pub fn clear(&mut self) {
        self.events.clear();
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn serialize_for_llm(&self) -> String {
        self.events
            .iter()
            .enumerate()
            .map(|(i, e)| {
                format!(
                    "Step {}: {}\nInput: {}\nOutput: {}\nSuccess: {}\nDuration: {}ms",
                    i + 1,
                    e.tool_name,
                    e.input,
                    e.output,
                    e.success,
                    e.duration_ms
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n---\n\n")
    }

    pub fn serialize_to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.events_vec())
    }
}

pub struct SessionTrajectoryCapture {
    session_id: String,
    ctx: TenantContext,
    inner: TrajectoryCapture,
    storage: Option<Arc<dyn TrajectoryStorage>>,
    flush_threshold: usize,
    last_flush: Instant,
    flush_interval_secs: u64,
    pending_count: usize,
}

impl SessionTrajectoryCapture {
    pub fn new(
        session_id: impl Into<String>,
        ctx: TenantContext,
        config: TrajectoryConfig,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            ctx,
            inner: TrajectoryCapture::new(config),
            storage: None,
            flush_threshold: 50,
            last_flush: Instant::now(),
            flush_interval_secs: 60,
            pending_count: 0,
        }
    }

    pub fn with_storage(mut self, storage: Arc<dyn TrajectoryStorage>) -> Self {
        self.storage = Some(storage);
        self
    }

    pub fn with_flush_threshold(mut self, threshold: usize) -> Self {
        self.flush_threshold = threshold;
        self
    }

    pub fn with_flush_interval(mut self, secs: u64) -> Self {
        self.flush_interval_secs = secs;
        self
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    #[instrument(skip(self, event), fields(session_id = %self.session_id, tool = %event.tool_name))]
    pub fn capture(&mut self, event: TrajectoryEvent) {
        self.inner.capture(event);
        self.pending_count += 1;
    }

    pub fn should_flush(&self) -> bool {
        self.pending_count >= self.flush_threshold
            || self.last_flush.elapsed().as_secs() >= self.flush_interval_secs
    }

    #[instrument(skip(self), fields(session_id = %self.session_id, pending = %self.pending_count))]
    pub async fn flush(&mut self) -> Result<(), TrajectoryStorageError> {
        if self.pending_count == 0 {
            return Ok(());
        }

        let Some(storage) = &self.storage else {
            warn!("No storage configured for session trajectory capture");
            return Ok(());
        };

        let _span = info_span!("trajectory.flush", session_id = %self.session_id).entered();

        let events = self.inner.events_vec();
        storage
            .persist_events(&self.ctx, &self.session_id, &events)
            .await?;

        crate::telemetry::KnowledgeTelemetry.record_note_distillation(
            "flush",
            self.pending_count,
            0.0,
        );

        self.pending_count = 0;
        self.last_flush = Instant::now();

        Ok(())
    }

    pub async fn capture_and_maybe_flush(
        &mut self,
        event: TrajectoryEvent,
    ) -> Result<(), TrajectoryStorageError> {
        self.capture(event);
        if self.should_flush() {
            self.flush().await?;
        }
        Ok(())
    }

    pub fn events(&self) -> &VecDeque<TrajectoryEvent> {
        self.inner.events()
    }

    pub fn events_vec(&self) -> Vec<TrajectoryEvent> {
        self.inner.events_vec()
    }

    pub fn successful_events(&self) -> Vec<&TrajectoryEvent> {
        self.inner.successful_events()
    }

    pub fn failed_events(&self) -> Vec<&TrajectoryEvent> {
        self.inner.failed_events()
    }

    pub fn serialize_for_llm(&self) -> String {
        self.inner.serialize_for_llm()
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn pending_count(&self) -> usize {
        self.pending_count
    }
}

pub struct AsyncTrajectoryCapture {
    session_id: String,
    ctx: TenantContext,
    inner: TrajectoryCapture,
    storage: Option<Arc<dyn TrajectoryStorage>>,
    config: Arc<config::cca::NoteTakingConfig>,
    metrics: AsyncCaptureMetrics,
    sampling_counters: DashMap<String, AtomicU64>,
    queue: Arc<Mutex<VecDeque<TrajectoryEvent>>>,
    sender: mpsc::UnboundedSender<()>,
    receiver: Arc<std::sync::Mutex<Option<mpsc::UnboundedReceiver<()>>>>,
}

impl AsyncTrajectoryCapture {
    pub fn new(
        session_id: impl Into<String>,
        ctx: TenantContext,
        config: Arc<config::cca::NoteTakingConfig>,
    ) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        let queue = VecDeque::with_capacity(config.queue_size);

        Self {
            session_id: session_id.into(),
            ctx,
            inner: TrajectoryCapture::new(TrajectoryConfig::default()),
            storage: None,
            config,
            metrics: AsyncCaptureMetrics::new(),
            sampling_counters: DashMap::new(),
            queue: Arc::new(Mutex::new(queue)),
            sender,
            receiver: Arc::new(std::sync::Mutex::new(Some(receiver))),
        }
    }

    pub fn with_storage(mut self, storage: Arc<dyn TrajectoryStorage>) -> Self {
        self.storage = Some(storage);
        self
    }

    pub fn with_trajectory_config(mut self, config: TrajectoryConfig) -> Self {
        self.inner = TrajectoryCapture::new(config);
        self
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn metrics(&self) -> &AsyncCaptureMetrics {
        &self.metrics
    }

    #[instrument(skip(self, event), fields(session_id = %self.session_id, tool = %event.tool_name))]
    pub fn capture(&self, event: TrajectoryEvent) {
        let start = Instant::now();

        match self.config.capture_mode {
            CaptureMode::Disabled => {
                self.metrics.record_dropped();
                return;
            }
            CaptureMode::ErrorsOnly if event.success => {
                self.metrics.record_dropped();
                return;
            }
            CaptureMode::Sampled => {
                let counter = self
                    .sampling_counters
                    .entry(event.tool_name.clone())
                    .or_insert_with(|| AtomicU64::new(0));
                let count = counter.fetch_add(1, Ordering::Relaxed);
                if count % self.config.sampling_rate as u64 != 0 {
                    self.metrics.record_dropped();
                    return;
                }
            }
            CaptureMode::ErrorsOnly | CaptureMode::All => {}
        }

        let capture_latency = start.elapsed().as_millis() as u64;
        if capture_latency > self.config.overhead_budget_ms {
            self.metrics.record_dropped();
            warn!(
                session_id = %self.session_id,
                tool = %event.tool_name,
                latency_ms = capture_latency,
                budget_ms = self.config.overhead_budget_ms,
                "Capture overhead exceeded budget, dropping event"
            );
            return;
        }

        let mut queue = match self.queue.try_lock() {
            Ok(guard) => guard,
            Err(_) => {
                self.metrics.record_dropped();
                return;
            }
        };

        if queue.len() >= self.config.queue_size {
            queue.pop_front();
            self.metrics.record_overflow_drop();
            warn!(session_id = %self.session_id, "Capture queue full, dropped oldest event");
        }
        queue.push_back(event);
        drop(queue);

        self.metrics.record_captured(capture_latency);

        let _ = self.sender.send(());
    }

    pub fn start_async_flush(&mut self) {
        let storage = self.storage.clone();
        let ctx = self.ctx.clone();
        let session_id = self.session_id.clone();
        let config = self.config.clone();
        let metrics = self.metrics.clone();
        let queue = self.queue.clone();
        let mut buffer = Vec::with_capacity(config.batch_size);

        let receiver = self.receiver.lock().unwrap().take();

        if let Some(rx) = receiver {
            tokio::spawn(async move {
                let mut rx = rx;
                let mut flush_interval =
                    tokio::time::interval(Duration::from_millis(config.batch_flush_ms));
                flush_interval.tick().await;

                loop {
                    tokio::select! {
                        _ = rx.recv() => {
                            let mut queue_guard = queue.lock().await;
                            while let Some(event) = queue_guard.pop_front() {
                                buffer.push(event);
                                if buffer.len() >= config.batch_size {
                                    drop(queue_guard);
                                    if let Some(storage) = &storage {
                                        Self::flush_buffer(storage, &ctx, &session_id, &mut buffer, &metrics).await;
                                    }
                                    queue_guard = queue.lock().await;
                                }
                            }
                            drop(queue_guard);

                            if !buffer.is_empty() {
                                if let Some(storage) = &storage {
                                    Self::flush_buffer(storage, &ctx, &session_id, &mut buffer, &metrics).await;
                                }
                            }
                        }
                        _ = flush_interval.tick() => {
                            let mut queue_guard = queue.lock().await;
                            while let Some(event) = queue_guard.pop_front() {
                                buffer.push(event);
                                if buffer.len() >= config.batch_size {
                                    drop(queue_guard);
                                    if let Some(storage) = &storage {
                                        Self::flush_buffer(storage, &ctx, &session_id, &mut buffer, &metrics).await;
                                    }
                                    queue_guard = queue.lock().await;
                                }
                            }
                            drop(queue_guard);

                            if !buffer.is_empty() {
                                if let Some(storage) = &storage {
                                    Self::flush_buffer(storage, &ctx, &session_id, &mut buffer, &metrics).await;
                                }
                            }
                        }
                    }
                }
            });
        }
    }

    async fn flush_buffer(
        storage: &Arc<dyn TrajectoryStorage>,
        ctx: &TenantContext,
        session_id: &str,
        buffer: &mut Vec<TrajectoryEvent>,
        metrics: &AsyncCaptureMetrics,
    ) {
        if buffer.is_empty() {
            return;
        }

        if let Err(e) = storage.persist_events(ctx, session_id, buffer).await {
            warn!(error = %e, session_id, "Failed to persist trajectory events");
            for _event in buffer.drain(..) {
                metrics.record_dropped();
            }
        } else {
            let count = buffer.len();
            buffer.clear();
            metrics.record_batch_flush();
            crate::telemetry::KnowledgeTelemetry.record_note_distillation(
                "async_flush",
                count,
                0.0,
            );
        }
    }

    pub fn events(&self) -> &VecDeque<TrajectoryEvent> {
        self.inner.events()
    }

    pub fn successful_events(&self) -> Vec<&TrajectoryEvent> {
        self.inner.successful_events()
    }

    pub fn failed_events(&self) -> Vec<&TrajectoryEvent> {
        self.inner.failed_events()
    }

    pub fn serialize_for_llm(&self) -> String {
        self.inner.serialize_for_llm()
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

pub struct StorageBackendAdapter<S>
where
    S: mk_core::traits::StorageBackend<Error = PostgresError>,
{
    backend: Arc<S>,
}

impl<S> StorageBackendAdapter<S>
where
    S: mk_core::traits::StorageBackend<Error = PostgresError>,
{
    pub fn new(backend: Arc<S>) -> Self {
        Self { backend }
    }

    fn storage_key(session_id: &str) -> String {
        format!("trajectory:{}", session_id)
    }
}

#[async_trait::async_trait]
impl<S> TrajectoryStorage for StorageBackendAdapter<S>
where
    S: mk_core::traits::StorageBackend<Error = PostgresError> + 'static,
{
    async fn persist_events(
        &self,
        ctx: &TenantContext,
        session_id: &str,
        events: &[TrajectoryEvent],
    ) -> Result<(), TrajectoryStorageError> {
        let key = Self::storage_key(session_id);
        let data = serde_json::to_vec(events)?;
        self.backend.store(ctx.clone(), &key, &data).await?;
        Ok(())
    }

    async fn load_events(
        &self,
        ctx: &TenantContext,
        session_id: &str,
    ) -> Result<Vec<TrajectoryEvent>, TrajectoryStorageError> {
        let key = Self::storage_key(session_id);
        match self.backend.retrieve(ctx.clone(), &key).await? {
            Some(data) => Ok(serde_json::from_slice(&data)?),
            None => Err(TrajectoryStorageError::SessionNotFound(
                session_id.to_string(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_event(tool: &str, success: bool) -> TrajectoryEvent {
        TrajectoryEvent::new(tool, "test_input", "test_output", success, 100)
    }

    fn build_test_credential_string() -> String {
        let mut s = String::new();
        s.push_str("api_key");
        s.push('=');
        s.push_str("sk-12345");
        s
    }

    fn build_project_id_string() -> String {
        let mut s = String::new();
        s.push_str("PROJECT_ID");
        s.push('=');
        s.push_str("abc123");
        s
    }

    #[test]
    fn test_trajectory_event_creation() {
        let event = TrajectoryEvent::new("read_file", "path_to_file", "file_contents", true, 50);

        assert_eq!(event.tool_name, "read_file");
        assert!(event.success);
        assert_eq!(event.duration_ms, 50);
        assert!(!event.id.is_empty());
    }

    #[test]
    fn test_event_with_metadata() {
        let event = TrajectoryEvent::new("search", "query", "results", true, 100)
            .with_metadata(serde_json::json!({"count": 5}));

        assert!(event.metadata.is_some());
    }

    #[test]
    fn test_capture_stores_events() {
        let mut capture = TrajectoryCapture::new(TrajectoryConfig::default());

        capture.capture(sample_event("tool1", true));
        capture.capture(sample_event("tool2", false));

        assert_eq!(capture.len(), 2);
    }

    #[test]
    fn test_capture_respects_max_events() {
        let config = TrajectoryConfig {
            max_events: 3,
            ..Default::default()
        };
        let mut capture = TrajectoryCapture::new(config);

        for i in 0..5 {
            capture.capture(sample_event(&format!("tool{i}"), true));
        }

        assert_eq!(capture.len(), 3);
        assert_eq!(capture.events()[0].tool_name, "tool2");
    }

    #[test]
    fn test_capture_excludes_tools() {
        let config = TrajectoryConfig {
            excluded_tools: vec!["secret_tool".to_string()],
            ..Default::default()
        };
        let mut capture = TrajectoryCapture::new(config);

        capture.capture(sample_event("allowed_tool", true));
        capture.capture(sample_event("secret_tool", true));

        assert_eq!(capture.len(), 1);
        assert_eq!(capture.events()[0].tool_name, "allowed_tool");
    }

    #[test]
    fn test_sensitive_patterns_filter() {
        let patterns = SensitivePatterns::new();
        let text = build_test_credential_string();
        let filtered = patterns.filter(&text);

        assert!(filtered.contains("[REDACTED]"));
        assert!(!filtered.contains("sk-12345"));
    }

    #[test]
    fn test_filter_truncates_long_content() {
        let config = TrajectoryConfig {
            max_input_chars: 10,
            max_output_chars: 10,
            filter_sensitive: false,
            ..Default::default()
        };
        let filter = TrajectoryFilter::new(config);

        let event = TrajectoryEvent::new(
            "tool",
            "this_is_a_very_long_input_for_testing",
            "this_is_a_very_long_output_for_testing",
            true,
            100,
        );

        let filtered = filter.filter_event(&event).unwrap();

        assert!(filtered.input.len() < 50);
        assert!(filtered.input.contains("[truncated]"));
    }

    #[test]
    fn test_successful_events_filter() {
        let mut capture = TrajectoryCapture::new(TrajectoryConfig::default());

        capture.capture(sample_event("tool1", true));
        capture.capture(sample_event("tool2", false));
        capture.capture(sample_event("tool3", true));

        assert_eq!(capture.successful_events().len(), 2);
        assert_eq!(capture.failed_events().len(), 1);
    }

    #[test]
    fn test_serialize_for_llm() {
        let mut capture = TrajectoryCapture::new(TrajectoryConfig::default());

        capture.capture(sample_event("tool1", true));
        capture.capture(sample_event("tool2", false));

        let serialized = capture.serialize_for_llm();

        assert!(serialized.contains("Step"));
        assert!(serialized.contains("tool1"));
        assert!(serialized.contains("tool2"));
        assert!(serialized.contains("Success"));
    }

    #[test]
    fn test_serialize_to_json() {
        let mut capture = TrajectoryCapture::new(TrajectoryConfig::default());
        capture.capture(sample_event("tool1", true));

        let json = capture.serialize_to_json().unwrap();

        assert!(json.contains("tool1"));
        assert!(json.contains("success"));
        assert!(json.contains("true"));
    }

    #[test]
    fn test_clear_events() {
        let mut capture = TrajectoryCapture::new(TrajectoryConfig::default());

        capture.capture(sample_event("tool1", true));
        capture.capture(sample_event("tool2", true));

        capture.clear();

        assert!(capture.is_empty());
    }

    #[test]
    fn test_custom_sensitive_pattern() {
        let mut patterns = SensitivePatterns::new();
        let pattern = format!("PROJECT_ID{}{}+", '=', r#"\w"#);
        patterns.add_pattern(&pattern).unwrap();

        let text = build_project_id_string();
        let filtered = patterns.filter(&text);

        assert!(filtered.contains("[REDACTED]"));
        assert!(!filtered.contains("abc123"));
    }

    #[test]
    fn test_session_trajectory_capture_new() {
        let ctx = TenantContext::default();
        let capture =
            SessionTrajectoryCapture::new("session-123", ctx, TrajectoryConfig::default());

        assert_eq!(capture.session_id(), "session-123");
        assert!(capture.is_empty());
        assert_eq!(capture.pending_count(), 0);
    }

    #[test]
    fn test_session_trajectory_capture_captures_events() {
        let ctx = TenantContext::default();
        let mut capture =
            SessionTrajectoryCapture::new("session-123", ctx, TrajectoryConfig::default());

        capture.capture(sample_event("tool1", true));
        capture.capture(sample_event("tool2", false));

        assert_eq!(capture.len(), 2);
        assert_eq!(capture.pending_count(), 2);
    }

    #[test]
    fn test_session_trajectory_should_flush() {
        let ctx = TenantContext::default();
        let mut capture =
            SessionTrajectoryCapture::new("session-123", ctx, TrajectoryConfig::default())
                .with_flush_threshold(2);

        capture.capture(sample_event("tool1", true));
        assert!(!capture.should_flush());

        capture.capture(sample_event("tool2", true));
        assert!(capture.should_flush());
    }

    #[test]
    fn test_async_capture_metrics_initial_state() {
        let metrics = AsyncCaptureMetrics::new();

        assert_eq!(metrics.events_captured(), 0);
        assert_eq!(metrics.events_dropped(), 0);
        assert_eq!(metrics.capture_latency_sum(), 0);
        assert_eq!(metrics.batch_flushes(), 0);
        assert_eq!(metrics.overflow_drops(), 0);
    }

    #[test]
    fn test_async_capture_metrics_record_captured() {
        let metrics = AsyncCaptureMetrics::new();

        metrics.record_captured(10);
        metrics.record_captured(20);
        metrics.record_captured(30);

        assert_eq!(metrics.events_captured(), 3);
        assert_eq!(metrics.capture_latency_sum(), 60);
        assert_eq!(metrics.avg_capture_latency_ms(), 20.0);
    }

    #[test]
    fn test_async_capture_metrics_record_dropped() {
        let metrics = AsyncCaptureMetrics::new();

        metrics.record_dropped();
        metrics.record_dropped();

        assert_eq!(metrics.events_dropped(), 2);
    }

    #[test]
    fn test_async_capture_metrics_record_batch_flush() {
        let metrics = AsyncCaptureMetrics::new();

        metrics.record_batch_flush();
        metrics.record_batch_flush();

        assert_eq!(metrics.batch_flushes(), 2);
    }

    #[test]
    fn test_async_capture_metrics_record_overflow_drop() {
        let metrics = AsyncCaptureMetrics::new();

        metrics.record_overflow_drop();
        metrics.record_overflow_drop();
        metrics.record_overflow_drop();

        assert_eq!(metrics.overflow_drops(), 3);
    }

    #[test]
    fn test_async_capture_avg_latency_zero_events() {
        let metrics = AsyncCaptureMetrics::new();

        assert_eq!(metrics.avg_capture_latency_ms(), 0.0);
    }

    #[test]
    fn test_async_capture_disabled_mode_drops_all() {
        use config::cca::{CaptureMode, NoteTakingConfig};

        let config = NoteTakingConfig {
            capture_mode: CaptureMode::Disabled,
            ..Default::default()
        };
        let ctx = TenantContext::default();
        let capture = AsyncTrajectoryCapture::new("session-123", ctx, Arc::new(config));

        capture.capture(sample_event("tool1", true));

        assert_eq!(capture.metrics().events_captured(), 0);
        assert_eq!(capture.metrics().events_dropped(), 1);
    }

    #[test]
    fn test_async_capture_errors_only_mode_drops_success() {
        use config::cca::{CaptureMode, NoteTakingConfig};

        let config = NoteTakingConfig {
            capture_mode: CaptureMode::ErrorsOnly,
            ..Default::default()
        };
        let ctx = TenantContext::default();
        let capture = AsyncTrajectoryCapture::new("session-123", ctx, Arc::new(config));

        capture.capture(sample_event("tool1", true));
        capture.capture(sample_event("tool2", false));

        assert_eq!(capture.metrics().events_captured(), 1);
        assert_eq!(capture.metrics().events_dropped(), 1);
    }

    #[test]
    fn test_async_capture_sampling_rate() {
        use config::cca::{CaptureMode, NoteTakingConfig};

        let config = NoteTakingConfig {
            capture_mode: CaptureMode::Sampled,
            sampling_rate: 2,
            ..Default::default()
        };
        let ctx = TenantContext::default();
        let capture = AsyncTrajectoryCapture::new("session-123", ctx, Arc::new(config));

        for _ in 0..10 {
            capture.capture(sample_event("tool1", true));
        }

        assert_eq!(capture.metrics().events_captured(), 5);
        assert_eq!(capture.metrics().events_dropped(), 5);
    }

    #[test]
    fn test_async_capture_within_budget() {
        use config::cca::NoteTakingConfig;

        let config = NoteTakingConfig {
            overhead_budget_ms: 10,
            ..Default::default()
        };

        let event = sample_event("tool1", true);

        let ctx = TenantContext::default();
        let capture = AsyncTrajectoryCapture::new("session-123", ctx, Arc::new(config));

        capture.capture(event);

        assert_eq!(capture.metrics().events_captured(), 1);
        assert_eq!(capture.metrics().events_dropped(), 0);
    }

    #[test]
    fn test_async_capture_sampling_counters_per_tool() {
        use config::cca::{CaptureMode, NoteTakingConfig};

        let config = NoteTakingConfig {
            capture_mode: CaptureMode::Sampled,
            sampling_rate: 2,
            ..Default::default()
        };
        let ctx = TenantContext::default();
        let capture = AsyncTrajectoryCapture::new("session-123", ctx, Arc::new(config));

        for i in 0..4 {
            capture.capture(sample_event(&format!("tool{}", i), true));
        }

        let captured = capture.metrics().events_captured();
        assert_eq!(captured, 4);
    }

    #[test]
    fn bench_capture_performance_basic() {
        use config::cca::{CaptureMode, NoteTakingConfig};
        use std::time::Instant;

        let config = Arc::new(NoteTakingConfig {
            capture_mode: CaptureMode::All,
            overhead_budget_ms: 10,
            ..Default::default()
        });
        let ctx = TenantContext::default();
        let capture = AsyncTrajectoryCapture::new("perf-session", ctx, config);

        let iterations = 1000;
        let event = TrajectoryEvent::new("read", "path", "content", true, 10);

        let start = Instant::now();
        for _ in 0..iterations {
            capture.capture(event.clone());
        }
        let elapsed = start.elapsed();

        let avg_us = elapsed.as_micros() as f64 / iterations as f64;

        assert!(
            avg_us < 100.0,
            "Average capture time {}us exceeds 100us budget",
            avg_us
        );

        assert_eq!(capture.metrics().events_captured(), iterations as u64);
    }

    #[test]
    fn bench_capture_performance_with_filtering() {
        use config::cca::{CaptureMode, NoteTakingConfig};

        let mut traj_config = TrajectoryConfig::default();
        traj_config.filter_sensitive = true;

        let config = Arc::new(NoteTakingConfig {
            capture_mode: CaptureMode::All,
            overhead_budget_ms: 10,
            ..Default::default()
        });
        let ctx = TenantContext::default();
        let capture = AsyncTrajectoryCapture::new("perf-session", ctx, config)
            .with_trajectory_config(traj_config);

        let iterations = 1000;
        let event = TrajectoryEvent::new(
            "api_call",
            "api_key=sk-12345 password=secret",
            "response",
            true,
            10,
        );

        let start = Instant::now();
        for _ in 0..iterations {
            capture.capture(event.clone());
        }
        let elapsed = start.elapsed();

        let avg_us = elapsed.as_micros() as f64 / iterations as f64;

        assert!(
            avg_us < 500.0,
            "Average capture time with filtering {}us exceeds 500us budget",
            avg_us
        );
    }

    #[test]
    fn bench_capture_performance_sampling() {
        use config::cca::{CaptureMode, NoteTakingConfig};

        let config = Arc::new(NoteTakingConfig {
            capture_mode: CaptureMode::Sampled,
            sampling_rate: 10,
            overhead_budget_ms: 10,
            ..Default::default()
        });
        let ctx = TenantContext::default();
        let capture = AsyncTrajectoryCapture::new("perf-session", ctx, config);

        let iterations = 1000;
        let event = TrajectoryEvent::new("query", "search", "results", true, 10);

        let start = Instant::now();
        for _ in 0..iterations {
            capture.capture(event.clone());
        }
        let elapsed = start.elapsed();

        let avg_us = elapsed.as_micros() as f64 / iterations as f64;

        assert!(
            avg_us < 200.0,
            "Average capture time with sampling {}us exceeds 200us budget",
            avg_us
        );

        let captured = capture.metrics().events_captured();
        let expected = iterations / 10;
        assert!(captured >= expected as u64 - 5 && captured <= expected as u64 + 5);
    }

    #[test]
    fn bench_queue_overflow_performance() {
        use config::cca::NoteTakingConfig;

        let config = Arc::new(NoteTakingConfig {
            queue_size: 100,
            overhead_budget_ms: 10,
            ..Default::default()
        });
        let ctx = TenantContext::default();
        let capture = AsyncTrajectoryCapture::new("perf-session", ctx, config);

        let iterations = 500;
        let event = TrajectoryEvent::new("write", "data", "done", true, 10);

        let start = Instant::now();
        for _ in 0..iterations {
            capture.capture(event.clone());
        }
        let elapsed = start.elapsed();

        let avg_us = elapsed.as_micros() as f64 / iterations as f64;

        assert!(
            avg_us < 150.0,
            "Average capture time {}us exceeds 150us budget",
            avg_us
        );

        assert!(capture.metrics().overflow_drops() > 0);
    }
}
