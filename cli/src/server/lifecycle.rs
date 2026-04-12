//! Lifecycle manager — coordinates all background day-2 tasks.
//!
//! Spawns periodic Tokio tasks for retention purge, job cleanup,
//! remediation expiry, and future reconciliation/quota/decay jobs.
//! Uses a `watch<bool>` channel (same pattern as the HTTP shutdown)
//! for graceful cancellation.

use std::sync::Arc;
use std::time::Duration;

use memory::decay::DecayConfig;
use storage::dead_letter::DeadLetterQueue;
use storage::remediation_store::RemediationStore;
use tokio::sync::watch;

use super::backup_api;
use super::AppState;

/// Coordinates all periodic background lifecycle tasks.
pub struct LifecycleManager {
    shutdown_tx: watch::Sender<bool>,
    shutdown_rx: watch::Receiver<bool>,
}

impl LifecycleManager {
    /// Create a new lifecycle manager with its own shutdown channel.
    pub fn new() -> Self {
        let (tx, rx) = watch::channel(false);
        Self {
            shutdown_tx: tx,
            shutdown_rx: rx,
        }
    }

    /// Spawn all lifecycle tasks. Returns immediately.
    ///
    /// When `state.redis_conn` is present, each task acquires a distributed
    /// lock so that only one replica in the Kubernetes ReplicaSet runs it.
    pub fn start(&self, state: Arc<AppState>) {
        let rc = state.redis_conn.clone();

        // Retention purge — daily
        Self::spawn_task_with_lock("retention_purge", Duration::from_secs(86400), self.shutdown_rx.clone(), rc.clone(), {
            let state = state.clone();
            move || {
                let state = state.clone();
                async move { run_retention_purge(&state).await }
            }
        });

        // Job cleanup — hourly (consolidates the task previously in serve.rs)
        Self::spawn_task_with_lock("job_cleanup", Duration::from_secs(3600), self.shutdown_rx.clone(), rc.clone(), {
            move || async move {
                backup_api::cleanup_expired_export_jobs().await;
                backup_api::cleanup_expired_import_jobs().await;
                backup_api::cleanup_temp_files().await;
            }
        });

        // Remediation auto-expiry — daily
        Self::spawn_task_with_lock(
            "remediation_expiry",
            Duration::from_secs(86400),
            self.shutdown_rx.clone(),
            rc.clone(),
            {
                move || async move {
                    let store = RemediationStore::global();
                    let expired = store.expire_stale(7 * 86400).await; // 7 days
                    if expired > 0 {
                        tracing::info!(count = expired, "Expired stale remediation requests");
                    }
                    let cleaned = store.cleanup_old(30 * 86400).await; // 30 days
                    if cleaned > 0 {
                        tracing::info!(count = cleaned, "Cleaned old remediation records");
                    }
                }
            },
        );

        // Dead-letter cleanup — daily (remove discarded items older than 30 days)
        Self::spawn_task_with_lock(
            "dead_letter_cleanup",
            Duration::from_secs(86400),
            self.shutdown_rx.clone(),
            rc.clone(),
            {
                move || async move {
                    let dlq = DeadLetterQueue::global();
                    let cleaned = dlq.cleanup_discarded(30 * 86400).await;
                    if cleaned > 0 {
                        tracing::info!(count = cleaned, "Cleaned old dead-letter items");
                    }
                    let active = dlq.active_count().await;
                    if active > 0 {
                        tracing::info!(count = active, "Dead-letter queue has active items");
                    }
                }
            },
        );

        // Importance decay — hourly
        Self::spawn_task_with_lock(
            "importance_decay",
            Duration::from_secs(3600),
            self.shutdown_rx.clone(),
            rc,
            {
                let state = state.clone();
                move || {
                    let state = state.clone();
                    async move {
                        run_importance_decay(&state).await;
                    }
                }
            },
        );

        tracing::info!("Lifecycle manager started (5 tasks)");
    }

    /// Signal all spawned tasks to shut down.
    pub fn shutdown(&self) {
        let _ = self.shutdown_tx.send(true);
        tracing::info!("Lifecycle manager shutdown signal sent");
    }

    /// Spawn a single periodic task that runs at `interval`, respecting the
    /// shutdown signal.
    ///
    /// When `redis_conn` is provided, a distributed lock is acquired before
    /// each execution so that only one replica in the Kubernetes ReplicaSet
    /// runs the task per cycle.
    fn spawn_task<F, Fut>(
        name: &'static str,
        interval: Duration,
        shutdown_rx: watch::Receiver<bool>,
        task_fn: F,
    ) where
        F: Fn() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ()> + Send,
    {
        Self::spawn_task_with_lock(name, interval, shutdown_rx, None, task_fn);
    }

    /// Spawn a periodic task with optional distributed locking via Redis.
    fn spawn_task_with_lock<F, Fut>(
        name: &'static str,
        interval: Duration,
        mut shutdown_rx: watch::Receiver<bool>,
        redis_conn: Option<Arc<redis::aio::ConnectionManager>>,
        task_fn: F,
    ) where
        F: Fn() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ()> + Send,
    {
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            // Skip the first immediate tick so we don't run on startup.
            ticker.tick().await;

            loop {
                tokio::select! {
                    _ = ticker.tick() => {
                        // Try to acquire a distributed lock if Redis is available.
                        if let Some(ref conn) = redis_conn {
                            let lock_key = format!("aeterna:lifecycle:{name}");
                            let lock_ttl = interval.as_secs();
                            let mut lock_conn = (**conn).clone();
                            let acquired: Option<String> = redis::cmd("SET")
                                .arg(&lock_key)
                                .arg("1")
                                .arg("NX")
                                .arg("EX")
                                .arg(lock_ttl)
                                .query_async(&mut lock_conn)
                                .await
                                .unwrap_or(None);
                            if acquired.is_none() {
                                tracing::debug!(
                                    task = name,
                                    "Lifecycle task skipped (another replica holds the lock)"
                                );
                                continue;
                            }
                        }

                        tracing::debug!(task = name, "Lifecycle task running");
                        (task_fn)().await;
                        tracing::debug!(task = name, "Lifecycle task completed");
                    }
                    _ = shutdown_rx.changed() => {
                        if *shutdown_rx.borrow() {
                            tracing::info!(task = name, "Lifecycle task shutting down");
                            break;
                        }
                    }
                }
            }
        });
    }
}

impl Default for LifecycleManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Run the daily retention purge cycle.
///
/// Cleans up old remediation records and — when storage backends are
/// available — expired soft-deleted graph nodes and rejected promotions.
async fn run_retention_purge(state: &AppState) {
    let config = storage::retention::RetentionConfig::from_env();

    // 1. Remediation request cleanup (30 days for terminal records)
    let store = RemediationStore::global();
    let cleaned = store.cleanup_old(30 * 86400).await;
    if cleaned > 0 {
        tracing::info!(count = cleaned, "Retention purge: removed old remediation records");
    }

    // 2. Export/import job cleanup is handled by the separate job_cleanup task.

    // 3. Hard-delete soft-deleted graph nodes past retention (default 7 days)
    if let Some(ref graph) = state.graph_store {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(i64::from(config.soft_delete_days));
        match graph.cleanup_deleted(cutoff) {
            Ok(count) => {
                if count > 0 {
                    tracing::info!(count, days = config.soft_delete_days, "Retention purge: hard-deleted expired graph nodes");
                }
            }
            Err(e) => tracing::warn!(error = %e, "Retention purge: graph hard-delete failed"),
        }
    }

    // 4. Purge old governance events past retention (default 180 days)
    let cutoff_secs = config.governance_event_days as i64 * 86400;
    let cutoff = chrono::Utc::now().timestamp() - cutoff_secs;
    match sqlx::query("DELETE FROM governance_events WHERE created_at < $1")
        .bind(cutoff)
        .execute(state.postgres.pool())
        .await
    {
        Ok(result) => {
            let count = result.rows_affected();
            if count > 0 {
                tracing::info!(count, days = config.governance_event_days, "Retention purge: removed old governance events");
            }
        }
        Err(e) => tracing::warn!(error = %e, "Retention purge: governance events cleanup failed"),
    }

    // 5. Purge old drift results past retention (default 30 days)
    let cutoff_secs = config.drift_result_days as i64 * 86400;
    let cutoff = chrono::Utc::now().timestamp() - cutoff_secs;
    match sqlx::query("DELETE FROM drift_results WHERE created_at < $1")
        .bind(cutoff)
        .execute(state.postgres.pool())
        .await
    {
        Ok(result) => {
            let count = result.rows_affected();
            if count > 0 {
                tracing::info!(count, days = config.drift_result_days, "Retention purge: removed old drift results");
            }
        }
        Err(e) => tracing::warn!(error = %e, "Retention purge: drift results cleanup failed"),
    }

    tracing::info!("Retention purge cycle completed");
}

/// Run importance decay on all memory layers.
///
/// For each configured layer, applies exponential decay to entries that have
/// not been accessed (or updated) within the last day. The decay formula
/// matches `memory::decay::calculate_decay`:
///
/// `new_score = importance_score * (1 - rate) ^ days_since_last_access`
///
/// Timestamps in `memory_entries` are stored as Unix epoch seconds (BIGINT),
/// so the day-delta is computed as `(now_epoch - coalesce(last_accessed_at, updated_at)) / 86400`.
async fn run_importance_decay(state: &AppState) {
    let config = DecayConfig::from_env();
    let pool = state.postgres.pool();
    let now_epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    // Only consider entries not accessed in the last 24 hours.
    let one_day_ago = now_epoch - 86400;

    for (layer_name, decay_rate) in &config.rates {
        let result = sqlx::query(
            "UPDATE memory_entries \
             SET importance_score = importance_score \
                 * POWER(1.0 - $1, \
                     ($4 - COALESCE(last_accessed_at, updated_at))::DOUBLE PRECISION / 86400.0) \
             WHERE memory_layer = $2 \
             AND importance_score > $3 \
             AND deleted_at IS NULL \
             AND COALESCE(last_accessed_at, updated_at) < $5 \
             RETURNING id",
        )
        .bind(*decay_rate)
        .bind(layer_name.as_str())
        .bind(config.archival_threshold)
        .bind(now_epoch)
        .bind(one_day_ago)
        .fetch_all(pool)
        .await;

        match result {
            Ok(rows) => {
                if !rows.is_empty() {
                    tracing::info!(
                        layer = layer_name.as_str(),
                        entries_decayed = rows.len(),
                        "Importance decay applied"
                    );
                }
            }
            Err(e) => {
                tracing::warn!(
                    layer = layer_name.as_str(),
                    error = %e,
                    "Importance decay failed"
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_manager_creates_without_panic() {
        let _mgr = LifecycleManager::new();
    }

    #[test]
    fn lifecycle_manager_default_creates_without_panic() {
        let _mgr = LifecycleManager::default();
    }

    #[tokio::test]
    async fn shutdown_sends_signal() {
        let mgr = LifecycleManager::new();
        let mut rx = mgr.shutdown_rx.clone();
        assert!(!*rx.borrow());

        mgr.shutdown();
        // The receiver should see the change
        rx.changed().await.unwrap();
        assert!(*rx.borrow());
    }

    #[tokio::test]
    async fn spawn_task_respects_shutdown() {
        let (tx, rx) = watch::channel(false);
        let counter = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let counter_clone = counter.clone();

        LifecycleManager::spawn_task(
            "test_task",
            Duration::from_millis(50),
            rx,
            move || {
                let c = counter_clone.clone();
                async move {
                    c.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                }
            },
        );

        // Let at least one tick happen
        tokio::time::sleep(Duration::from_millis(120)).await;
        // Signal shutdown
        let _ = tx.send(true);
        // Give time for the task to exit
        tokio::time::sleep(Duration::from_millis(100)).await;

        let count = counter.load(std::sync::atomic::Ordering::SeqCst);
        // Should have run at least once but stopped after shutdown
        assert!(count >= 1, "Task should have run at least once, ran {count} times");
    }

    #[tokio::test]
    async fn run_retention_purge_completes() {
        // Smoke test — just ensure it doesn't panic with an empty store.
        run_retention_purge().await;
    }
}
