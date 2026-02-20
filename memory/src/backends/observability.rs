use super::{
    BackendCapabilities, BackendError, DeleteResult, HealthStatus, SearchQuery, SearchResult,
    UpsertResult, VectorBackend, VectorRecord,
};
use async_trait::async_trait;
use metrics::{counter, gauge, histogram};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use tracing::{Instrument, Level, span};

pub struct InstrumentedBackend<B: VectorBackend> {
    inner: B,
    circuit_breaker: CircuitBreaker,
}

impl<B: VectorBackend> InstrumentedBackend<B> {
    pub fn new(inner: B) -> Self {
        Self {
            inner,
            circuit_breaker: CircuitBreaker::new(5, 30),
        }
    }

    pub fn with_circuit_breaker(mut self, failure_threshold: u32, reset_timeout_secs: u64) -> Self {
        self.circuit_breaker = CircuitBreaker::new(failure_threshold, reset_timeout_secs);
        self
    }

    fn record_upsert(&self, duration: std::time::Duration, success: bool) {
        let backend = self.inner.backend_name();
        histogram!("vector_backend_operation_duration_seconds", "backend" => backend, "operation" => "upsert")
            .record(duration.as_secs_f64());
        counter!("vector_backend_operations_total", "backend" => backend, "operation" => "upsert")
            .increment(1);
        if !success {
            counter!("vector_backend_errors_total", "backend" => backend, "operation" => "upsert")
                .increment(1);
        }
    }

    fn record_search(&self, duration: std::time::Duration, success: bool) {
        let backend = self.inner.backend_name();
        histogram!("vector_backend_operation_duration_seconds", "backend" => backend, "operation" => "search")
            .record(duration.as_secs_f64());
        counter!("vector_backend_operations_total", "backend" => backend, "operation" => "search")
            .increment(1);
        if !success {
            counter!("vector_backend_errors_total", "backend" => backend, "operation" => "search")
                .increment(1);
        }
    }

    fn record_delete(&self, duration: std::time::Duration, success: bool) {
        let backend = self.inner.backend_name();
        histogram!("vector_backend_operation_duration_seconds", "backend" => backend, "operation" => "delete")
            .record(duration.as_secs_f64());
        counter!("vector_backend_operations_total", "backend" => backend, "operation" => "delete")
            .increment(1);
        if !success {
            counter!("vector_backend_errors_total", "backend" => backend, "operation" => "delete")
                .increment(1);
        }
    }

    fn record_get(&self, duration: std::time::Duration, success: bool) {
        let backend = self.inner.backend_name();
        histogram!("vector_backend_operation_duration_seconds", "backend" => backend, "operation" => "get")
            .record(duration.as_secs_f64());
        counter!("vector_backend_operations_total", "backend" => backend, "operation" => "get")
            .increment(1);
        if !success {
            counter!("vector_backend_errors_total", "backend" => backend, "operation" => "get")
                .increment(1);
        }
    }

    fn record_upsert_batch_size(&self, size: usize) {
        let backend = self.inner.backend_name();
        histogram!("vector_backend_batch_size", "backend" => backend, "operation" => "upsert")
            .record(size as f64);
    }

    fn record_delete_batch_size(&self, size: usize) {
        let backend = self.inner.backend_name();
        histogram!("vector_backend_batch_size", "backend" => backend, "operation" => "delete")
            .record(size as f64);
    }
}

#[async_trait]
impl<B: VectorBackend + Send + Sync> VectorBackend for InstrumentedBackend<B> {
    async fn health_check(&self) -> Result<HealthStatus, BackendError> {
        let backend = self.inner.backend_name();

        let start = Instant::now();
        let result = self
            .inner
            .health_check()
            .instrument(span!(Level::DEBUG, "vector_backend.health_check", backend))
            .await;
        let duration = start.elapsed();

        histogram!("vector_backend_health_check_duration_seconds", "backend" => backend)
            .record(duration.as_secs_f64());

        if let Ok(ref status) = result {
            tracing::debug!(
                backend = backend,
                healthy = status.healthy,
                latency_ms = status.latency_ms,
                "health check completed"
            );
            gauge!("vector_backend_healthy", "backend" => backend).set(if status.healthy {
                1.0
            } else {
                0.0
            });

            if let Some(latency) = status.latency_ms {
                gauge!("vector_backend_latency_ms", "backend" => backend).set(latency as f64);
            }
        } else if let Err(ref e) = result {
            tracing::warn!(backend = backend, error = %e, "health check failed");
        }

        result
    }

    async fn capabilities(&self) -> BackendCapabilities {
        let backend = self.inner.backend_name();
        self.inner
            .capabilities()
            .instrument(span!(Level::DEBUG, "vector_backend.capabilities", backend))
            .await
    }

    async fn upsert(
        &self,
        tenant_id: &str,
        vectors: Vec<VectorRecord>,
    ) -> Result<UpsertResult, BackendError> {
        let backend = self.inner.backend_name();
        let batch_size = vectors.len();

        if !self.circuit_breaker.allow_request() {
            tracing::warn!(backend, tenant_id, "circuit breaker rejected upsert");
            counter!("vector_backend_circuit_breaker_rejected_total", "backend" => backend)
                .increment(1);
            return Err(BackendError::CircuitOpen(backend.into()));
        }

        self.record_upsert_batch_size(batch_size);

        let start = Instant::now();
        let result = self
            .inner
            .upsert(tenant_id, vectors)
            .instrument(span!(
                Level::DEBUG,
                "vector_backend.upsert",
                backend,
                tenant_id,
                batch_size
            ))
            .await;
        let duration = start.elapsed();

        let success = result.is_ok();
        self.record_upsert(duration, success);

        if success {
            self.circuit_breaker.record_success();
            if let Ok(ref r) = result {
                tracing::debug!(
                    backend,
                    tenant_id,
                    upserted = r.upserted_count,
                    failed = r.failed_ids.len(),
                    duration_ms = duration.as_millis(),
                    "upsert completed"
                );
                counter!("vector_backend_vectors_upserted_total", "backend" => backend)
                    .increment(r.upserted_count as u64);
            }
        } else {
            self.circuit_breaker.record_failure();
            if let Err(ref e) = result {
                tracing::error!(backend, tenant_id, error = %e, "upsert failed");
            }
        }

        result
    }

    async fn search(
        &self,
        tenant_id: &str,
        query: SearchQuery,
    ) -> Result<Vec<SearchResult>, BackendError> {
        let backend = self.inner.backend_name();
        let limit = query.limit;

        if !self.circuit_breaker.allow_request() {
            tracing::warn!(backend, tenant_id, "circuit breaker rejected search");
            counter!("vector_backend_circuit_breaker_rejected_total", "backend" => backend)
                .increment(1);
            return Err(BackendError::CircuitOpen(backend.into()));
        }

        let start = Instant::now();
        let result = self
            .inner
            .search(tenant_id, query)
            .instrument(span!(
                Level::DEBUG,
                "vector_backend.search",
                backend,
                tenant_id,
                limit
            ))
            .await;
        let duration = start.elapsed();

        let success = result.is_ok();
        self.record_search(duration, success);

        if success {
            self.circuit_breaker.record_success();
            if let Ok(ref results) = result {
                tracing::debug!(
                    backend,
                    tenant_id,
                    result_count = results.len(),
                    duration_ms = duration.as_millis(),
                    "search completed"
                );
                histogram!("vector_backend_search_results_count", "backend" => backend)
                    .record(results.len() as f64);
            }
        } else {
            self.circuit_breaker.record_failure();
            if let Err(ref e) = result {
                tracing::error!(backend, tenant_id, error = %e, "search failed");
            }
        }

        result
    }

    async fn delete(
        &self,
        tenant_id: &str,
        ids: Vec<String>,
    ) -> Result<DeleteResult, BackendError> {
        let backend = self.inner.backend_name();
        let batch_size = ids.len();

        if !self.circuit_breaker.allow_request() {
            tracing::warn!(backend, tenant_id, "circuit breaker rejected delete");
            counter!("vector_backend_circuit_breaker_rejected_total", "backend" => backend)
                .increment(1);
            return Err(BackendError::CircuitOpen(backend.into()));
        }

        self.record_delete_batch_size(batch_size);

        let start = Instant::now();
        let result = self
            .inner
            .delete(tenant_id, ids)
            .instrument(span!(
                Level::DEBUG,
                "vector_backend.delete",
                backend,
                tenant_id,
                batch_size
            ))
            .await;
        let duration = start.elapsed();

        let success = result.is_ok();
        self.record_delete(duration, success);

        if success {
            self.circuit_breaker.record_success();
            if let Ok(ref r) = result {
                tracing::debug!(
                    backend,
                    tenant_id,
                    deleted = r.deleted_count,
                    duration_ms = duration.as_millis(),
                    "delete completed"
                );
                counter!("vector_backend_vectors_deleted_total", "backend" => backend)
                    .increment(r.deleted_count as u64);
            }
        } else {
            self.circuit_breaker.record_failure();
            if let Err(ref e) = result {
                tracing::error!(backend, tenant_id, error = %e, "delete failed");
            }
        }

        result
    }

    async fn get(&self, tenant_id: &str, id: &str) -> Result<Option<VectorRecord>, BackendError> {
        let backend = self.inner.backend_name();

        if !self.circuit_breaker.allow_request() {
            tracing::warn!(backend, tenant_id, id, "circuit breaker rejected get");
            counter!("vector_backend_circuit_breaker_rejected_total", "backend" => backend)
                .increment(1);
            return Err(BackendError::CircuitOpen(backend.into()));
        }

        let start = Instant::now();
        let result = self
            .inner
            .get(tenant_id, id)
            .instrument(span!(
                Level::DEBUG,
                "vector_backend.get",
                backend,
                tenant_id,
                id
            ))
            .await;
        let duration = start.elapsed();

        let success = result.is_ok();
        self.record_get(duration, success);

        if success {
            self.circuit_breaker.record_success();
            if let Ok(ref opt) = result {
                let hit = opt.is_some();
                tracing::debug!(
                    backend,
                    tenant_id,
                    id,
                    hit,
                    duration_ms = duration.as_millis(),
                    "get completed"
                );
                if hit {
                    counter!("vector_backend_get_hits_total", "backend" => backend, "hit" => "true").increment(1);
                } else {
                    counter!("vector_backend_get_hits_total", "backend" => backend, "hit" => "false").increment(1);
                }
            }
        } else {
            self.circuit_breaker.record_failure();
            if let Err(ref e) = result {
                tracing::error!(backend, tenant_id, id, error = %e, "get failed");
            }
        }

        result
    }

    fn backend_name(&self) -> &'static str {
        self.inner.backend_name()
    }
}

#[derive(Debug)]
pub struct CircuitBreaker {
    failure_count: AtomicU64,
    success_count: AtomicU64,
    last_failure_time: AtomicU64,
    failure_threshold: u32,
    reset_timeout_secs: u64,
    state: std::sync::atomic::AtomicU8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum CircuitState {
    Closed = 0,
    Open = 1,
    HalfOpen = 2,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: u32, reset_timeout_secs: u64) -> Self {
        Self {
            failure_count: AtomicU64::new(0),
            success_count: AtomicU64::new(0),
            last_failure_time: AtomicU64::new(0),
            failure_threshold,
            reset_timeout_secs,
            state: std::sync::atomic::AtomicU8::new(CircuitState::Closed as u8),
        }
    }

    fn state(&self) -> CircuitState {
        match self.state.load(Ordering::SeqCst) {
            0 => CircuitState::Closed,
            1 => CircuitState::Open,
            2 => CircuitState::HalfOpen,
            _ => CircuitState::Closed,
        }
    }

    fn set_state(&self, state: CircuitState) {
        self.state.store(state as u8, Ordering::SeqCst);
    }

    pub fn allow_request(&self) -> bool {
        match self.state() {
            CircuitState::Closed => true,
            CircuitState::Open => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let last_failure = self.last_failure_time.load(Ordering::SeqCst);

                if now - last_failure >= self.reset_timeout_secs {
                    self.set_state(CircuitState::HalfOpen);
                    true
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => true,
        }
    }

    pub fn record_success(&self) {
        self.success_count.fetch_add(1, Ordering::SeqCst);

        match self.state() {
            CircuitState::HalfOpen => {
                self.failure_count.store(0, Ordering::SeqCst);
                self.set_state(CircuitState::Closed);
            }
            CircuitState::Closed => {
                self.failure_count.store(0, Ordering::SeqCst);
            }
            _ => {}
        }
    }

    pub fn record_failure(&self) {
        let failures = self.failure_count.fetch_add(1, Ordering::SeqCst) + 1;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.last_failure_time.store(now, Ordering::SeqCst);

        if failures >= self.failure_threshold as u64 {
            self.set_state(CircuitState::Open);
        }
    }

    pub fn is_open(&self) -> bool {
        self.state() == CircuitState::Open
    }

    pub fn failure_count(&self) -> u64 {
        self.failure_count.load(Ordering::SeqCst)
    }
}

pub fn wrap_with_instrumentation<B: VectorBackend>(backend: B) -> InstrumentedBackend<B> {
    InstrumentedBackend::new(backend)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_breaker_closed() {
        let cb = CircuitBreaker::new(3, 30);
        assert!(cb.allow_request());
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_circuit_breaker_opens_after_failures() {
        let cb = CircuitBreaker::new(3, 30);

        cb.record_failure();
        cb.record_failure();
        assert!(cb.allow_request());

        cb.record_failure();
        assert!(cb.is_open());
        assert!(!cb.allow_request());
    }

    #[test]
    fn test_circuit_breaker_resets_on_success() {
        let cb = CircuitBreaker::new(3, 30);

        cb.record_failure();
        cb.record_failure();
        cb.record_success();

        assert_eq!(cb.failure_count(), 0);
        assert!(cb.allow_request());
    }
}
