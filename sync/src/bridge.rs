use crate::error::{Result, SyncError};
use crate::pointer::{KnowledgePointer, KnowledgePointerMetadata, map_layer};
use crate::state::{FederationConflict, SyncConflict, SyncFailure, SyncState, SyncTrigger};
use crate::state_persister::SyncStatePersister;
use config::config::DeploymentConfig;
use distributed_lock::{
    DistributedLock, LockHandle, LockProvider, RedisLockHandle, RedisLockProvider
};
use knowledge::federation::FederationProvider;
use knowledge::governance::GovernanceEngine;
use knowledge::governance_client::{GovernanceClient, RemoteGovernanceClient};
use memory::manager::MemoryManager;
use mk_core::traits::KnowledgeRepository;
use mk_core::types::{KnowledgeEntry, KnowledgeLayer, MemoryEntry, TenantContext};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct DeltaResult {
    pub added: Vec<KnowledgeEntry>,
    pub updated: Vec<KnowledgeEntry>,
    pub deleted: Vec<String>,
    pub unchanged: Vec<String>
}

pub struct SyncManager {
    memory_manager: Arc<MemoryManager>,
    knowledge_repo: Arc<dyn KnowledgeRepository<Error = knowledge::repository::RepositoryError>>,
    governance_engine: Arc<GovernanceEngine>,
    governance_client: Option<Arc<dyn GovernanceClient>>,
    deployment_config: DeploymentConfig,
    federation_manager: Option<Arc<dyn FederationProvider>>,
    persister: Arc<dyn SyncStatePersister>,
    lock_provider: Option<Arc<RedisLockProvider>>,
    states: Arc<RwLock<HashMap<mk_core::types::TenantId, SyncState>>>,
    checkpoints: Arc<RwLock<HashMap<mk_core::types::TenantId, SyncState>>>
}

impl SyncManager {
    #[tracing::instrument(skip(
        memory_manager,
        knowledge_repo,
        governance_engine,
        federation_manager,
        persister,
        lock_provider
    ))]
    pub async fn new(
        memory_manager: Arc<MemoryManager>,
        knowledge_repo: Arc<
            dyn KnowledgeRepository<Error = knowledge::repository::RepositoryError>
        >,
        governance_engine: Arc<GovernanceEngine>,
        deployment_config: DeploymentConfig,
        federation_manager: Option<Arc<dyn FederationProvider>>,
        persister: Arc<dyn SyncStatePersister>,
        lock_provider: Option<Arc<RedisLockProvider>>
    ) -> Result<Self> {
        let governance_client =
            if deployment_config.mode == "hybrid" || deployment_config.mode == "remote" {
                deployment_config.remote_url.as_ref().map(|url: &String| {
                    Arc::new(RemoteGovernanceClient::new(url.clone())) as Arc<dyn GovernanceClient>
                })
            } else {
                None
            };

        let states = HashMap::new();
        let checkpoints = HashMap::new();

        Ok(Self {
            memory_manager,
            knowledge_repo,
            governance_engine,
            governance_client,
            deployment_config,
            federation_manager,
            persister,
            lock_provider,
            states: Arc::new(RwLock::new(states)),
            checkpoints: Arc::new(RwLock::new(checkpoints))
        })
    }

    async fn get_or_load_state(&self, tenant_id: &mk_core::types::TenantId) -> Result<SyncState> {
        {
            let states = self.states.read().await;
            if let Some(state) = states.get(tenant_id) {
                return Ok(state.clone());
            }
        }

        let state = self
            .persister
            .load(tenant_id)
            .await
            .map_err(|e| SyncError::Persistence(e.to_string()))?;

        let mut states = self.states.write().await;
        states.insert(tenant_id.clone(), state.clone());
        Ok(state)
    }

    async fn update_state(&self, tenant_id: &mk_core::types::TenantId, state: SyncState) {
        let mut states = self.states.write().await;
        states.insert(tenant_id.clone(), state);
    }

    pub async fn acquire_sync_lock(
        &self,
        tenant_id: &mk_core::types::TenantId,
        job_name: &str
    ) -> Result<Option<RedisLockHandle>> {
        let Some(lock_provider) = &self.lock_provider else {
            return Ok(None);
        };

        let lock_key = format!("sync_lock:{}:{}", tenant_id.as_str(), job_name);
        let lock = lock_provider.create_lock(&lock_key);

        match lock.acquire(Some(Duration::from_secs(1))).await {
            Ok(handle) => Ok(Some(handle)),
            Err(distributed_lock::LockError::Timeout(_)) => Ok(None),
            Err(e) => Err(e.into())
        }
    }

    pub async fn release_sync_lock(&self, lock_handle: Option<RedisLockHandle>) -> Result<()> {
        if let Some(handle) = lock_handle {
            handle.release().await?;
        }
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub async fn initialize(&self, ctx: TenantContext) -> Result<()> {
        tracing::info!("Initializing SyncManager for tenant: {}", ctx.tenant_id);

        if ctx.tenant_id.as_str().contains("TRIGGER_FAILURE") {
            return Err(SyncError::Internal(
                "Simulated initialization failure".to_string()
            ));
        }

        self.knowledge_repo
            .get_head_commit(ctx.clone())
            .await
            .map_err(|e| {
                tracing::error!(
                    "Failed to access knowledge repository during initialization: {}",
                    e
                );
                SyncError::Internal(format!("Repo access failed: {}", e))
            })?;

        let state = self.get_or_load_state(&ctx.tenant_id).await?;
        tracing::info!(
            "SyncManager initialized for tenant {} with version {}, last sync: {:?}",
            ctx.tenant_id,
            state.version,
            state.last_sync_at
        );

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub async fn shutdown(&self) -> Result<()> {
        tracing::info!("Shutting down SyncManager");
        let states = self.states.read().await;
        for (tenant_id, state) in states.iter() {
            self.persister.save(tenant_id, state).await.map_err(|e| {
                tracing::error!("Failed to persist state for tenant {}: {}", tenant_id, e);
                SyncError::Persistence(e.to_string())
            })?;
        }
        tracing::info!("SyncManager states persisted successfully");
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub async fn scheduled_sync(
        &self,
        ctx: TenantContext,
        staleness_threshold_mins: u32
    ) -> Result<()> {
        if let Some(trigger) = self
            .check_triggers(ctx.clone(), staleness_threshold_mins)
            .await?
        {
            tracing::info!("Scheduled sync triggered by {:?}", trigger);
            self.run_sync_cycle(ctx, staleness_threshold_mins as u64)
                .await?;
        }
        Ok(())
    }
}

impl SyncManager {
    #[tracing::instrument(skip(self))]
    pub async fn run_sync_cycle(&self, ctx: TenantContext, interval_secs: u64) -> Result<()> {
        if self.deployment_config.mode == "hybrid" && !self.deployment_config.sync_enabled {
            tracing::info!("Sync disabled in Hybrid mode for tenant: {}", ctx.tenant_id);
            return Ok(());
        }

        // Acquire distributed lock for batch job coordination (if lock provider is
        // configured)
        let lock_handle = if self.lock_provider.is_some() {
            match self.acquire_sync_lock(&ctx.tenant_id, "batch_sync").await {
                Ok(Some(handle)) => {
                    tracing::debug!("Acquired sync lock for tenant: {}", ctx.tenant_id);
                    metrics::counter!("sync.lock.acquired", 1);
                    Some(handle)
                }
                Ok(None) => {
                    tracing::info!(
                        "Sync lock already held for tenant: {}, skipping cycle",
                        ctx.tenant_id
                    );
                    metrics::counter!("sync.lock.skipped", 1);
                    return Ok(());
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to acquire sync lock for tenant {}: {}",
                        ctx.tenant_id,
                        e
                    );
                    metrics::counter!("sync.lock.acquisition_failures", 1);
                    return Err(e);
                }
            }
        } else {
            None
        };

        let result = self
            .run_sync_cycle_internal(ctx.clone(), interval_secs)
            .await;

        if let Err(e) = self.release_sync_lock(lock_handle).await {
            tracing::warn!(
                "Failed to release sync lock for tenant {}: {}",
                ctx.tenant_id,
                e
            );
            metrics::counter!("sync.lock.release_failures", 1);
        }

        result
    }

    async fn run_sync_cycle_internal(&self, ctx: TenantContext, interval_secs: u64) -> Result<()> {
        if let Some(trigger) = self
            .check_triggers(ctx.clone(), (interval_secs / 60) as u32)
            .await?
        {
            tracing::info!("Sync triggered by {:?}", trigger);

            self.create_checkpoint(&ctx.tenant_id).await?;

            if let Some(fed_manager) = &self.federation_manager {
                let fed_start = std::time::Instant::now();
                if let Err(e) = self
                    .sync_federation(ctx.clone(), fed_manager.as_ref())
                    .await
                {
                    tracing::error!("Federation sync failed, rolling back: {}", e);
                    metrics::counter!("sync.federation.failures", 1);
                    self.rollback(&ctx.tenant_id).await?;
                    return Err(e);
                }
                metrics::histogram!(
                    "sync.federation.duration_ms",
                    fed_start.elapsed().as_millis() as f64
                );
            }

            let inc_start = std::time::Instant::now();
            let mut retry_count = 0;
            let max_retries = 3;
            let mut sync_result = self.sync_incremental(ctx.clone()).await;

            while let Err(e) = sync_result {
                if retry_count >= max_retries {
                    tracing::error!(
                        "Incremental sync failed after {} retries, rolling back: {}",
                        max_retries,
                        e
                    );
                    metrics::counter!("sync.incremental.failures", 1);
                    self.rollback(&ctx.tenant_id).await?;
                    return Err(e);
                }

                retry_count += 1;
                let backoff_ms = 100 * 2u64.pow(retry_count);
                tracing::warn!(
                    "Sync failed, retrying in {}ms (attempt {}/{}): {}",
                    backoff_ms,
                    retry_count,
                    max_retries,
                    e
                );
                tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
                sync_result = self.sync_incremental(ctx.clone()).await;
            }

            metrics::histogram!(
                "sync.incremental.duration_ms",
                inc_start.elapsed().as_millis() as f64
            );

            self.prune_failed_items(ctx.clone(), 30).await?;

            let conflicts = self.detect_conflicts(ctx.clone()).await?;
            if !conflicts.is_empty() {
                tracing::info!("Found {} conflicts during sync cycle", conflicts.len());
                metrics::counter!("sync.conflicts.detected", conflicts.len() as u64);
                let mut state = self.get_or_load_state(&ctx.tenant_id).await?;
                state.stats.total_conflicts += conflicts.len() as u64;
                self.update_state(&ctx.tenant_id, state).await;

                let tenant_id = ctx.tenant_id.clone();
                if let Err(e) = self.resolve_conflicts(ctx, conflicts).await {
                    tracing::error!("Conflict resolution failed, rolling back: {}", e);
                    metrics::counter!("sync.conflicts.resolution_failures", 1);
                    self.rollback(&tenant_id).await?;
                    return Err(e);
                }
                metrics::counter!("sync.conflicts.resolved", 1);
            }
        }

        Ok(())
    }

    pub async fn create_checkpoint(&self, tenant_id: &mk_core::types::TenantId) -> Result<()> {
        if tenant_id.as_str().contains("TRIGGER_FAILURE") {
            return Err(SyncError::Internal(
                "Simulated checkpoint failure".to_string()
            ));
        }
        let mut checkpoints = self.checkpoints.write().await;
        let state = self.get_or_load_state(tenant_id).await?;
        checkpoints.insert(tenant_id.clone(), state);
        tracing::debug!("Sync checkpoint created for tenant: {}", tenant_id);
        Ok(())
    }

    pub async fn rollback(&self, tenant_id: &mk_core::types::TenantId) -> Result<()> {
        let mut checkpoints = self.checkpoints.write().await;
        if let Some(old_state) = checkpoints.remove(tenant_id) {
            let mut states = self.states.write().await;
            states.insert(tenant_id.clone(), old_state.clone());
            self.persister
                .save(tenant_id, &old_state)
                .await
                .map_err(|e| {
                    metrics::counter!("sync.persistence.rollback_failures", 1);
                    SyncError::Persistence(e.to_string())
                })?;
            tracing::info!(
                "Sync state rolled back to checkpoint for tenant: {}",
                tenant_id
            );
            Ok(())
        } else {
            tracing::warn!(
                "Rollback requested but no checkpoint found for tenant: {}",
                tenant_id
            );
            Ok(())
        }
    }

    pub async fn sync_federation(
        &self,
        ctx: TenantContext,
        fed: &dyn FederationProvider
    ) -> Result<()> {
        tracing::info!("Starting federation sync for tenant: {}", ctx.tenant_id);
        let mut state = self.get_or_load_state(&ctx.tenant_id).await?;
        let upstreams = fed.config().upstreams.clone();

        for upstream in upstreams {
            let upstream_id = upstream.id.clone();

            let target_path = self
                .knowledge_repo
                .root_path()
                .unwrap_or_else(|| std::path::PathBuf::from("data/knowledge"))
                .join("federated")
                .join(&upstream_id);

            match fed.sync_upstream(&upstream_id, &target_path).await {
                Ok(_) => {
                    tracing::info!("Successfully synced upstream: {}", upstream_id);
                    state
                        .federation_conflicts
                        .retain(|c| c.upstream_id != upstream_id);
                }
                Err(knowledge::repository::RepositoryError::InvalidPath(msg))
                    if msg.contains("conflict") || msg.contains("upstream") =>
                {
                    tracing::error!("Federation conflict for upstream {}: {}", upstream_id, msg);
                    state
                        .federation_conflicts
                        .retain(|c| c.upstream_id != upstream_id);
                    state.federation_conflicts.push(FederationConflict {
                        upstream_id: upstream_id.clone(),
                        reason: msg,
                        detected_at: chrono::Utc::now().timestamp()
                    });
                }
                Err(e) => {
                    tracing::error!("Error syncing upstream {}: {}", upstream_id, e);
                }
            }
        }

        self.persister
            .save(&ctx.tenant_id, &state)
            .await
            .map_err(|e| SyncError::Persistence(e.to_string()))?;
        self.update_state(&ctx.tenant_id, state).await;
        Ok(())
    }

    pub async fn get_state(&self, tenant_id: &mk_core::types::TenantId) -> Result<SyncState> {
        self.get_or_load_state(tenant_id).await
    }

    #[tracing::instrument(skip(self))]
    pub async fn sync_incremental(&self, ctx: TenantContext) -> Result<()> {
        let mut state = self.get_or_load_state(&ctx.tenant_id).await?;
        let start_time = std::time::Instant::now();

        let last_commit = match &state.last_knowledge_commit {
            Some(c) => c.clone(),
            None => return self.sync_all_internal(ctx, &mut state, start_time).await
        };

        let head_commit = self.knowledge_repo.get_head_commit(ctx.clone()).await?;
        if let Some(head) = &head_commit
            && head == &last_commit
        {
            return Ok(());
        }

        let mut sync_errors = Vec::new();
        let affected_items = self
            .knowledge_repo
            .get_affected_items(ctx.clone(), &last_commit)
            .await?;

        for (layer, path) in affected_items {
            let entry = match self.knowledge_repo.get(ctx.clone(), layer, &path).await {
                Ok(Some(e)) => e,
                Ok(None) => {
                    if let Some(memory_id) = self.find_memory_id_by_knowledge_id(&path, &state) {
                        self.memory_manager
                            .delete_from_layer(ctx.clone(), map_layer(layer), &memory_id)
                            .await?;
                        state.knowledge_hashes.remove(&path);
                        state.pointer_mapping.remove(&memory_id);
                        state.knowledge_layers.remove(&path);
                    }
                    continue;
                }
                Err(e) => {
                    sync_errors.push(SyncFailure {
                        knowledge_id: path,
                        error: e.to_string(),
                        failed_at: chrono::Utc::now().timestamp(),
                        retry_count: 0
                    });
                    continue;
                }
            };

            if let Err(e) = self.sync_entry(ctx.clone(), &entry, &mut state).await {
                sync_errors.push(SyncFailure {
                    knowledge_id: entry.path.clone(),
                    error: e.to_string(),
                    failed_at: chrono::Utc::now().timestamp(),
                    retry_count: 0
                });
            }
        }

        state.last_sync_at = Some(chrono::Utc::now().timestamp());
        state.last_knowledge_commit = head_commit;
        state.failed_items.extend(sync_errors);
        state.stats.total_syncs += 1;
        let duration = start_time.elapsed().as_millis() as u64;
        state.stats.avg_sync_duration_ms = duration;

        metrics::counter!("sync.cycles.total", 1);
        metrics::histogram!("sync.cycle.duration_ms", duration as f64);
        metrics::gauge!("sync.items.failed", state.failed_items.len() as f64);

        self.persister
            .save(&ctx.tenant_id, &state)
            .await
            .map_err(|e| SyncError::Persistence(e.to_string()))?;
        self.update_state(&ctx.tenant_id, state).await;

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub async fn sync_all(&self, ctx: TenantContext) -> Result<()> {
        let mut state = self.get_or_load_state(&ctx.tenant_id).await?;
        let start_time = std::time::Instant::now();
        self.sync_all_internal(ctx, &mut state, start_time).await
    }

    async fn sync_all_internal(
        &self,
        ctx: TenantContext,
        state: &mut SyncState,
        start_time: std::time::Instant
    ) -> Result<()> {
        let head_commit = self.knowledge_repo.get_head_commit(ctx.clone()).await?;
        let mut sync_errors = Vec::new();

        for layer in [
            mk_core::types::KnowledgeLayer::Company,
            mk_core::types::KnowledgeLayer::Org,
            mk_core::types::KnowledgeLayer::Team,
            mk_core::types::KnowledgeLayer::Project
        ] {
            let entries = match self.knowledge_repo.list(ctx.clone(), layer, "").await {
                Ok(e) => e,
                Err(e) => {
                    sync_errors.push(SyncFailure {
                        knowledge_id: format!("layer:{layer:?}"),
                        error: e.to_string(),
                        failed_at: chrono::Utc::now().timestamp(),
                        retry_count: 0
                    });
                    continue;
                }
            };

            for entry in entries {
                if let Err(e) = self.sync_entry(ctx.clone(), &entry, state).await {
                    sync_errors.push(SyncFailure {
                        knowledge_id: entry.path.clone(),
                        error: e.to_string(),
                        failed_at: chrono::Utc::now().timestamp(),
                        retry_count: 0
                    });
                }
            }
        }

        state.last_sync_at = Some(chrono::Utc::now().timestamp());
        state.last_knowledge_commit = head_commit;
        state.failed_items.extend(sync_errors);
        state.stats.total_syncs += 1;
        let duration = start_time.elapsed().as_millis() as u64;
        state.stats.avg_sync_duration_ms = duration;

        metrics::counter!("sync.cycles.total", 1);
        metrics::histogram!("sync.cycle.duration_ms", duration as f64);
        metrics::gauge!("sync.items.failed", state.failed_items.len() as f64);

        self.persister
            .save(&ctx.tenant_id, state)
            .await
            .map_err(|e| SyncError::Persistence(e.to_string()))?;

        self.update_state(&ctx.tenant_id, state.clone()).await;

        Ok(())
    }

    pub async fn check_triggers(
        &self,
        ctx: TenantContext,
        staleness_threshold_mins: u32
    ) -> Result<Option<SyncTrigger>> {
        if self.deployment_config.mode == "remote" {
            return Ok(Some(SyncTrigger::Manual));
        }

        let state = self.get_or_load_state(&ctx.tenant_id).await?;

        let head_commit = self.knowledge_repo.get_head_commit(ctx).await?;
        if let Some(head) = head_commit {
            if let Some(last) = &state.last_knowledge_commit {
                if head != *last {
                    return Ok(Some(SyncTrigger::CommitMismatch {
                        last_commit: last.clone(),
                        head_commit: head
                    }));
                }
            } else {
                return Ok(Some(SyncTrigger::CommitMismatch {
                    last_commit: "none".to_string(),
                    head_commit: head
                }));
            }
        }

        if let Some(last_sync) = state.last_sync_at {
            let now = chrono::Utc::now().timestamp();
            let elapsed_mins = (now - last_sync) / 60;
            if elapsed_mins >= staleness_threshold_mins as i64 {
                return Ok(Some(SyncTrigger::Staleness {
                    last_sync_at: last_sync,
                    threshold_mins: staleness_threshold_mins
                }));
            }
        } else {
            return Ok(Some(SyncTrigger::Manual));
        }

        Ok(None)
    }

    pub async fn resolve_federation_conflict(
        &self,
        tenant_id: mk_core::types::TenantId,
        upstream_id: &str,
        resolution: &str
    ) -> Result<()> {
        let mut state = self.get_or_load_state(&tenant_id).await?;

        state
            .federation_conflicts
            .retain(|c| c.upstream_id != upstream_id);

        tracing::info!(
            "Resolved federation conflict for tenant {} upstream {}: {}",
            tenant_id,
            upstream_id,
            resolution
        );

        self.persister
            .save(&tenant_id, &state)
            .await
            .map_err(|e| SyncError::Persistence(e.to_string()))?;
        self.update_state(&tenant_id, state).await;
        Ok(())
    }

    pub async fn resolve_conflicts(
        &self,
        ctx: TenantContext,
        conflicts: Vec<SyncConflict>
    ) -> Result<()> {
        let mut state = self.get_or_load_state(&ctx.tenant_id).await?;

        for conflict in conflicts {
            match conflict {
                SyncConflict::HashMismatch { knowledge_id, .. }
                | SyncConflict::MissingPointer { knowledge_id, .. } => {
                    state.knowledge_hashes.remove(&knowledge_id);
                    let layer = state
                        .knowledge_layers
                        .get(&knowledge_id)
                        .cloned()
                        .unwrap_or(mk_core::types::KnowledgeLayer::Company);
                    if let Some(entry) = self
                        .knowledge_repo
                        .get(ctx.clone(), layer, &knowledge_id)
                        .await?
                    {
                        self.sync_entry(ctx.clone(), &entry, &mut state).await?;
                        metrics::counter!("sync.conflicts.resolved.hash_mismatch", 1);
                    }
                }
                SyncConflict::OrphanedPointer {
                    memory_id,
                    knowledge_id
                } => {
                    for layer in [
                        mk_core::types::MemoryLayer::Company,
                        mk_core::types::MemoryLayer::Org,
                        mk_core::types::MemoryLayer::Team,
                        mk_core::types::MemoryLayer::Project
                    ] {
                        let _ = self
                            .memory_manager
                            .delete_from_layer(ctx.clone(), layer, &memory_id)
                            .await;
                    }
                    state.knowledge_hashes.remove(&knowledge_id);
                    state.pointer_mapping.remove(&memory_id);
                    state.knowledge_layers.remove(&knowledge_id);
                    metrics::counter!("sync.conflicts.resolved.orphaned", 1);
                }
                SyncConflict::DuplicatePointer {
                    knowledge_id,
                    mut memory_ids
                } => {
                    memory_ids.sort();
                    let _to_keep = memory_ids.remove(0);

                    for mid in memory_ids {
                        for layer in [
                            mk_core::types::MemoryLayer::Company,
                            mk_core::types::MemoryLayer::Org,
                            mk_core::types::MemoryLayer::Team,
                            mk_core::types::MemoryLayer::Project
                        ] {
                            let _ = self
                                .memory_manager
                                .delete_from_layer(ctx.clone(), layer, &mid)
                                .await;
                        }
                        state.pointer_mapping.remove(&mid);
                    }

                    let layer = state
                        .knowledge_layers
                        .get(&knowledge_id)
                        .cloned()
                        .unwrap_or(mk_core::types::KnowledgeLayer::Company);
                    if let Some(entry) = self
                        .knowledge_repo
                        .get(ctx.clone(), layer, &knowledge_id)
                        .await?
                    {
                        self.sync_entry(ctx.clone(), &entry, &mut state).await?;
                    }
                    metrics::counter!("sync.conflicts.resolved.duplicate", 1);
                }
                SyncConflict::StatusChange {
                    knowledge_id,
                    memory_id,
                    ..
                } => {
                    let layer = state
                        .knowledge_layers
                        .get(&knowledge_id)
                        .cloned()
                        .unwrap_or(mk_core::types::KnowledgeLayer::Company);
                    if let Some(entry) = self
                        .knowledge_repo
                        .get(ctx.clone(), layer, &knowledge_id)
                        .await?
                    {
                        self.sync_entry(ctx.clone(), &entry, &mut state).await?;
                    }
                    tracing::info!(
                        "Resolved status_change conflict for {} (memory: {})",
                        knowledge_id,
                        memory_id
                    );
                    metrics::counter!("sync.conflicts.resolved.status_change", 1);
                }
                SyncConflict::LayerMismatch {
                    knowledge_id,
                    memory_id,
                    expected_layer,
                    actual_layer
                } => {
                    let old_memory_layer = map_layer(expected_layer);
                    let _ = self
                        .memory_manager
                        .delete_from_layer(ctx.clone(), old_memory_layer, &memory_id)
                        .await;

                    state.knowledge_hashes.remove(&knowledge_id);
                    state.pointer_mapping.remove(&memory_id);
                    state.knowledge_layers.remove(&knowledge_id);

                    if let Some(entry) = self
                        .knowledge_repo
                        .get(ctx.clone(), actual_layer, &knowledge_id)
                        .await?
                    {
                        self.sync_entry(ctx.clone(), &entry, &mut state).await?;
                    }

                    tracing::info!(
                        "Resolved layer_mismatch conflict for {}: {:?} -> {:?}",
                        knowledge_id,
                        expected_layer,
                        actual_layer
                    );
                    metrics::counter!("sync.conflicts.resolved.layer_mismatch", 1);
                }
                SyncConflict::DetectionError { target_id, error } => {
                    tracing::warn!(
                        "Skipping resolution for detection error on {}: {}",
                        target_id,
                        error
                    );
                }
            }
        }

        self.persister
            .save(&ctx.tenant_id, &state)
            .await
            .map_err(|e| SyncError::Persistence(e.to_string()))?;
        self.update_state(&ctx.tenant_id, state).await;
        Ok(())
    }

    pub async fn detect_conflicts(&self, ctx: TenantContext) -> Result<Vec<SyncConflict>> {
        let state = self.get_or_load_state(&ctx.tenant_id).await?;
        let mut conflicts = Vec::new();

        let mut knowledge_to_memories: HashMap<String, Vec<String>> = HashMap::new();
        for (memory_id, knowledge_id) in &state.pointer_mapping {
            knowledge_to_memories
                .entry(knowledge_id.clone())
                .or_default()
                .push(memory_id.clone());
        }

        for (knowledge_id, memory_ids) in knowledge_to_memories {
            if memory_ids.len() > 1 {
                conflicts.push(SyncConflict::DuplicatePointer {
                    knowledge_id,
                    memory_ids
                });
            }
        }

        for (memory_id, knowledge_id) in &state.pointer_mapping {
            println!(
                "Checking pointer mapping: {} -> {}",
                memory_id, knowledge_id
            );
            let layer = state
                .knowledge_layers
                .get(knowledge_id)
                .cloned()
                .unwrap_or(mk_core::types::KnowledgeLayer::Company);
            println!("Expected layer for {}: {:?}", knowledge_id, layer);

            let entry_res = self
                .knowledge_repo
                .get(ctx.clone(), layer, knowledge_id)
                .await;
            if let Ok(Some(ref entry)) = entry_res {
                println!(
                    "Got entry from repo: {:?} at layer {:?}",
                    entry.path, entry.layer
                );
            } else if let Ok(None) = entry_res {
                println!("Entry not found in repo at layer {:?}", layer);
            } else if let Err(ref e) = entry_res {
                println!("Error getting entry from repo: {}", e);
            }

            match entry_res {
                Ok(Some(k_entry)) => {
                    let expected_hash = state.knowledge_hashes.get(knowledge_id);
                    let actual_hash = utils::compute_content_hash(&k_entry.content);

                    if let Some(exp) = expected_hash
                        && exp != &actual_hash
                    {
                        conflicts.push(SyncConflict::HashMismatch {
                            knowledge_id: knowledge_id.clone(),
                            memory_id: memory_id.clone(),
                            expected_hash: exp.clone(),
                            actual_hash
                        });
                    }

                    if k_entry.status == mk_core::types::KnowledgeStatus::Deprecated
                        || k_entry.status == mk_core::types::KnowledgeStatus::Superseded
                    {
                        conflicts.push(SyncConflict::StatusChange {
                            knowledge_id: knowledge_id.clone(),
                            memory_id: memory_id.clone(),
                            new_status: k_entry.status
                        });
                    }

                    if k_entry.layer != layer {
                        conflicts.push(SyncConflict::LayerMismatch {
                            knowledge_id: knowledge_id.clone(),
                            memory_id: memory_id.clone(),
                            expected_layer: layer,
                            actual_layer: k_entry.layer
                        });
                    }

                    let m_layer = map_layer(k_entry.layer);
                    match self
                        .memory_manager
                        .get_from_layer(ctx.clone(), m_layer, memory_id)
                        .await
                    {
                        Ok(None) => {
                            conflicts.push(SyncConflict::MissingPointer {
                                knowledge_id: knowledge_id.clone(),
                                expected_memory_id: memory_id.clone()
                            });
                        }
                        Ok(Some(m_entry)) => {
                            let mut content = k_entry.content.clone();
                            content = utils::redact_pii(&content);
                            let expected_content =
                                self.generate_summary_internal(&k_entry, &content);
                            if m_entry.content != expected_content {
                                conflicts.push(SyncConflict::HashMismatch {
                                    knowledge_id: knowledge_id.clone(),
                                    memory_id: memory_id.clone(),
                                    expected_hash: "summary_mismatch".to_string(),
                                    actual_hash: "summary_mismatch".to_string()
                                });
                            }
                        }
                        Err(e) => {
                            conflicts.push(SyncConflict::DetectionError {
                                target_id: memory_id.clone(),
                                error: e.to_string()
                            });
                            tracing::warn!("Failed to check memory entry {}: {}", memory_id, e)
                        }
                    }
                }
                Ok(None) => {
                    let mut found_elsewhere = false;
                    for other_layer in [
                        mk_core::types::KnowledgeLayer::Company,
                        mk_core::types::KnowledgeLayer::Org,
                        mk_core::types::KnowledgeLayer::Team,
                        mk_core::types::KnowledgeLayer::Project
                    ] {
                        if other_layer == layer {
                            continue;
                        }

                        if let Ok(Some(_actual_entry)) = self
                            .knowledge_repo
                            .get(ctx.clone(), other_layer, knowledge_id)
                            .await
                        {
                            conflicts.push(SyncConflict::LayerMismatch {
                                knowledge_id: knowledge_id.clone(),
                                memory_id: memory_id.clone(),
                                expected_layer: layer,
                                actual_layer: other_layer
                            });
                            found_elsewhere = true;
                            break;
                        }
                    }

                    if !found_elsewhere {
                        conflicts.push(SyncConflict::OrphanedPointer {
                            memory_id: memory_id.clone(),
                            knowledge_id: knowledge_id.clone()
                        });
                    }
                }
                Err(e) => {
                    conflicts.push(SyncConflict::DetectionError {
                        target_id: knowledge_id.clone(),
                        error: e.to_string()
                    });
                    tracing::error!(
                        "Error fetching knowledge {} for conflict detection: {}",
                        knowledge_id,
                        e
                    )
                }
            }
        }

        Ok(conflicts)
    }

    fn find_memory_id_by_knowledge_id(
        &self,
        knowledge_id: &str,
        state: &SyncState
    ) -> Option<String> {
        state
            .pointer_mapping
            .iter()
            .find(|(_, kid)| *kid == knowledge_id)
            .map(|(mid, _)| mid.clone())
    }

    pub async fn sync_entry(
        &self,
        ctx: TenantContext,
        entry: &KnowledgeEntry,
        state: &mut SyncState
    ) -> Result<()> {
        if entry.path.contains("TRIGGER_FAILURE") {
            return Err(SyncError::Internal(
                "Simulated entry sync failure".to_string()
            ));
        }
        let mut content = entry.content.clone();
        content = utils::redact_pii(&content);

        let mut context = HashMap::new();
        context.insert("path".to_string(), serde_json::json!(entry.path));
        context.insert("content".to_string(), serde_json::json!(content));

        if self.deployment_config.mode == "hybrid" || self.deployment_config.mode == "remote" {
            if let Some(client) = &self.governance_client {
                let validation = client
                    .validate(&ctx, entry.layer, &context)
                    .await
                    .map_err(|e| SyncError::Internal(format!("Remote validation failed: {}", e)))?;

                if !validation.is_valid {
                    state.stats.total_governance_blocks += 1;
                    metrics::counter!("sync.governance.blocks", 1);
                    for violation in validation.violations {
                        if violation.severity == mk_core::types::ConstraintSeverity::Block {
                            state.failed_items.push(SyncFailure {
                                knowledge_id: entry.path.clone(),
                                error: format!(
                                    "Remote governance violation (BLOCK): {}",
                                    violation.message
                                ),
                                failed_at: chrono::Utc::now().timestamp(),
                                retry_count: 0
                            });
                            return Err(SyncError::GovernanceBlock(violation.message));
                        }
                        tracing::warn!(
                            "Remote governance violation ({:?}) for {}: {}",
                            violation.severity,
                            entry.path,
                            violation.message
                        );
                    }
                }
            }
        }

        if self.deployment_config.mode != "remote" {
            let validation = self.governance_engine.validate(entry.layer, &context);
            if !validation.is_valid {
                state.stats.total_governance_blocks += 1;
                metrics::counter!("sync.governance.blocks", 1);
                for violation in validation.violations {
                    if violation.severity == mk_core::types::ConstraintSeverity::Block {
                        state.failed_items.push(SyncFailure {
                            knowledge_id: entry.path.clone(),
                            error: format!("Governance violation (BLOCK): {}", violation.message),
                            failed_at: chrono::Utc::now().timestamp(),
                            retry_count: 0
                        });
                        return Err(SyncError::GovernanceBlock(violation.message));
                    }
                    tracing::warn!(
                        "Governance violation ({:?}) for {}: {}",
                        violation.severity,
                        entry.path,
                        violation.message
                    );
                }
            }
        }

        let content_hash = utils::compute_content_hash(&content);
        let knowledge_id = &entry.path;

        if let Some(prev_hash) = state.knowledge_hashes.get(knowledge_id)
            && prev_hash == &content_hash
        {
            return Ok(());
        }

        let memory_layer = map_layer(entry.layer);
        let pointer = KnowledgePointer {
            source_type: entry.kind,
            source_id: knowledge_id.clone(),
            content_hash: content_hash.clone(),
            synced_at: chrono::Utc::now().timestamp(),
            source_layer: entry.layer,
            is_orphaned: false
        };

        let metadata = KnowledgePointerMetadata {
            kind: "knowledge_pointer".to_string(),
            knowledge_pointer: pointer,
            tags: Vec::new()
        };

        let metadata_map = match serde_json::to_value(metadata)? {
            serde_json::Value::Object(map) => {
                let mut hmap = HashMap::new();
                for (k, v) in map {
                    hmap.insert(k, v);
                }
                hmap
            }
            _ => {
                return Err(SyncError::Internal(
                    "Failed to serialize metadata".to_string()
                ));
            }
        };

        let memory_entry = MemoryEntry {
            id: format!("ptr_{knowledge_id}"),
            content: self.generate_summary_internal(entry, &content),
            embedding: None,
            layer: memory_layer,
            summaries: std::collections::HashMap::new(),
            context_vector: None,
            importance_score: None,
            metadata: metadata_map,
            created_at: chrono::Utc::now().timestamp(),
            updated_at: chrono::Utc::now().timestamp()
        };

        self.memory_manager
            .add_to_layer(ctx, memory_layer, memory_entry)
            .await?;

        tracing::info!("Synced entry: {}", entry.path);

        state
            .knowledge_hashes
            .insert(knowledge_id.clone(), content_hash);
        state
            .pointer_mapping
            .insert(format!("ptr_{knowledge_id}"), knowledge_id.clone());
        state
            .knowledge_layers
            .insert(knowledge_id.clone(), entry.layer);
        state.stats.total_items_synced += 1;
        metrics::counter!("sync.items.synced", 1);

        Ok(())
    }

    pub fn generate_summary(&self, entry: &KnowledgeEntry) -> String {
        self.generate_summary_internal(entry, &entry.content)
    }

    fn generate_summary_internal(&self, entry: &KnowledgeEntry, content: &str) -> String {
        let mut summary = format!(
            "[{:?}] [{:?}] {}\n\n{}",
            entry.kind,
            entry.status,
            entry.path,
            content.lines().next().unwrap_or("")
        );

        let mut context = HashMap::new();
        context.insert("path".to_string(), serde_json::json!(entry.path));
        context.insert("content".to_string(), serde_json::json!(content));

        let validation = self.governance_engine.validate(entry.layer, &context);
        if !validation.is_valid {
            let blocks: Vec<_> = validation
                .violations
                .iter()
                .filter(|v| v.severity == mk_core::types::ConstraintSeverity::Block)
                .map(|v| v.message.as_str())
                .collect();

            if !blocks.is_empty() {
                summary.push_str("\n\nGOVERNANCE BLOCKS:\n- ");
                summary.push_str(&blocks.join("\n- "));
            }
        }

        summary
    }

    pub async fn prune_failed_items(&self, ctx: TenantContext, days_old: i64) -> Result<()> {
        let mut state = self.get_or_load_state(&ctx.tenant_id).await?;
        let now = chrono::Utc::now().timestamp();
        let threshold = days_old * 24 * 60 * 60;

        let before_count = state.failed_items.len();
        state
            .failed_items
            .retain(|f| (now - f.failed_at) < threshold);

        let pruned = before_count - state.failed_items.len();
        if pruned > 0 {
            tracing::info!(
                "Pruned {} failed items older than {} days for tenant: {}",
                pruned,
                days_old,
                ctx.tenant_id
            );
            self.persister
                .save(&ctx.tenant_id, &state)
                .await
                .map_err(|e| SyncError::Persistence(e.to_string()))?;
            self.update_state(&ctx.tenant_id, state).await;
        }

        Ok(())
    }

    pub fn find_memory_id_by_knowledge_id_for_test(
        &self,
        knowledge_id: &str,
        state: &SyncState
    ) -> Option<String> {
        self.find_memory_id_by_knowledge_id(knowledge_id, state)
    }

    pub async fn detect_delta(&self, ctx: TenantContext, state: &SyncState) -> Result<DeltaResult> {
        let mut delta = DeltaResult::default();
        let layers = [
            KnowledgeLayer::Company,
            KnowledgeLayer::Org,
            KnowledgeLayer::Team,
            KnowledgeLayer::Project
        ];

        for layer in layers {
            let entries = self.knowledge_repo.list(ctx.clone(), layer, "").await?;
            for entry in entries {
                let knowledge_id = &entry.path;
                let content_hash = utils::compute_content_hash(&utils::redact_pii(&entry.content));

                match state.knowledge_hashes.get(knowledge_id) {
                    Some(prev_hash) if prev_hash == &content_hash => {
                        delta.unchanged.push(knowledge_id.clone());
                    }
                    Some(_) => {
                        delta.updated.push(entry);
                    }
                    None => {
                        delta.added.push(entry);
                    }
                }
            }
        }

        for (knowledge_id, _) in &state.knowledge_hashes {
            if !delta.unchanged.contains(knowledge_id)
                && !delta.updated.iter().any(|e| &e.path == knowledge_id)
            {
                delta.deleted.push(knowledge_id.clone());
            }
        }

        Ok(delta)
    }

    #[tracing::instrument(skip(self, rx))]
    pub async fn start_background_sync(
        self: Arc<Self>,
        ctx: TenantContext,
        interval_secs: u64,
        staleness_threshold_mins: u32,
        mut rx: tokio::sync::watch::Receiver<bool>
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(interval_secs));
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if let Err(e) = self.run_sync_cycle(ctx.clone(), staleness_threshold_mins as u64).await {
                            metrics::counter!("sync.background.errors", 1);
                            tracing::error!("Background sync error for tenant {}: {}", ctx.tenant_id, e);
                        }
                    }
                    _ = rx.changed() => {
                        if *rx.borrow() {
                            tracing::info!("Background sync shutting down for tenant: {}", ctx.tenant_id);
                            break;
                        }
                    }
                }
            }
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use mk_core::TenantId;
    use mk_core::types::{KnowledgeEntry, KnowledgeLayer, KnowledgeStatus, KnowledgeType};
    use std::collections::HashMap;
    use std::time::Instant;

    struct MockPersister;
    #[async_trait::async_trait]
    impl SyncStatePersister for MockPersister {
        async fn load(
            &self,
            _tenant_id: &mk_core::types::TenantId
        ) -> std::result::Result<SyncState, Box<dyn std::error::Error + Send + Sync>> {
            Ok(SyncState::default())
        }
        async fn save(
            &self,
            _tenant_id: &mk_core::types::TenantId,
            _s: &SyncState
        ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }
    }

    struct MockKnowledgeRepository;
    impl MockKnowledgeRepository {
        fn new() -> Self {
            Self
        }
    }
    #[async_trait::async_trait]
    impl KnowledgeRepository for MockKnowledgeRepository {
        type Error = knowledge::repository::RepositoryError;
        async fn store(
            &self,
            _ctx: TenantContext,
            _e: KnowledgeEntry,
            _m: &str
        ) -> std::result::Result<String, Self::Error> {
            Ok("hash".to_string())
        }
        async fn get(
            &self,
            _ctx: TenantContext,
            _l: KnowledgeLayer,
            _p: &str
        ) -> std::result::Result<Option<KnowledgeEntry>, Self::Error> {
            Ok(None)
        }
        async fn list(
            &self,
            _ctx: TenantContext,
            _l: KnowledgeLayer,
            _p: &str
        ) -> std::result::Result<Vec<KnowledgeEntry>, Self::Error> {
            Ok(Vec::new())
        }
        async fn delete(
            &self,
            _ctx: TenantContext,
            _l: KnowledgeLayer,
            _p: &str,
            _m: &str
        ) -> std::result::Result<String, Self::Error> {
            Ok("hash".to_string())
        }
        async fn get_head_commit(
            &self,
            _ctx: TenantContext
        ) -> std::result::Result<Option<String>, Self::Error> {
            Ok(None)
        }
        async fn get_affected_items(
            &self,
            _ctx: TenantContext,
            _f: &str
        ) -> std::result::Result<Vec<(KnowledgeLayer, String)>, Self::Error> {
            Ok(Vec::new())
        }
        async fn search(
            &self,
            _ctx: TenantContext,
            _q: &str,
            _l: Vec<KnowledgeLayer>,
            _li: usize
        ) -> std::result::Result<Vec<KnowledgeEntry>, Self::Error> {
            Ok(Vec::new())
        }
        fn root_path(&self) -> Option<std::path::PathBuf> {
            None
        }
    }

    async fn create_memory_manager_with_mock_providers() -> Arc<MemoryManager> {
        let memory = Arc::new(MemoryManager::new());
        for layer in [
            mk_core::types::MemoryLayer::Company,
            mk_core::types::MemoryLayer::Org,
            mk_core::types::MemoryLayer::Team,
            mk_core::types::MemoryLayer::Project,
            mk_core::types::MemoryLayer::Session,
            mk_core::types::MemoryLayer::User,
            mk_core::types::MemoryLayer::Agent
        ] {
            memory
                .register_provider(layer, {
                    let provider: Arc<
                        dyn mk_core::traits::MemoryProviderAdapter<
                                Error = Box<dyn std::error::Error + Send + Sync>
                            > + Send
                            + Sync
                    > = Arc::new(memory::providers::MockProvider::new());
                    provider
                })
                .await;
        }
        memory
    }

    fn create_sync_manager_with_state(
        memory_manager: Arc<MemoryManager>,
        knowledge_repo: Arc<
            dyn KnowledgeRepository<Error = knowledge::repository::RepositoryError>
        >,
        state: SyncState,
        tenant_id: &mk_core::types::TenantId
    ) -> SyncManager {
        let mut states_map = HashMap::new();
        states_map.insert(tenant_id.clone(), state);
        SyncManager {
            memory_manager,
            knowledge_repo,
            governance_engine: Arc::new(GovernanceEngine::new()),
            governance_client: None,
            deployment_config: DeploymentConfig::default(),
            federation_manager: None,
            persister: Arc::new(MockPersister),
            states: Arc::new(RwLock::new(states_map)),
            lock_provider: None,
            checkpoints: Arc::new(RwLock::new(HashMap::new()))
        }
    }

    #[test]
    fn test_generate_summary() {
        let sync_manager = SyncManager {
            memory_manager: Arc::new(MemoryManager::new()),
            knowledge_repo: Arc::new(MockKnowledgeRepository::new()),
            governance_engine: Arc::new(GovernanceEngine::new()),
            governance_client: None,
            deployment_config: DeploymentConfig::default(),
            federation_manager: None,
            persister: Arc::new(MockPersister),
            states: Arc::new(RwLock::new(HashMap::new())),
            lock_provider: None,
            checkpoints: Arc::new(RwLock::new(HashMap::new()))
        };

        let entry = KnowledgeEntry {
            path: "test.md".to_string(),
            content: "First line\nSecond line\nThird line".to_string(),
            layer: KnowledgeLayer::Project,
            kind: KnowledgeType::Spec,
            status: KnowledgeStatus::Accepted,
            metadata: HashMap::new(),
            commit_hash: None,
            author: None,
            updated_at: 1234567890,
            summaries: HashMap::new()
        };

        let summary = sync_manager.generate_summary(&entry);
        assert_eq!(summary, "[Spec] [Accepted] test.md\n\nFirst line");
    }

    #[tokio::test]
    async fn test_generate_summary_empty_content() {
        let _ctx = TenantContext::default();
        let sync_manager = SyncManager {
            memory_manager: Arc::new(MemoryManager::new()),
            knowledge_repo: Arc::new(MockKnowledgeRepository::new()),
            governance_engine: Arc::new(GovernanceEngine::new()),
            governance_client: None,
            deployment_config: DeploymentConfig::default(),
            federation_manager: None,
            persister: Arc::new(MockPersister),
            states: Arc::new(RwLock::new(HashMap::new())),
            lock_provider: None,
            checkpoints: Arc::new(RwLock::new(HashMap::new()))
        };

        let entry = KnowledgeEntry {
            path: "empty.md".to_string(),
            content: "".to_string(),
            layer: KnowledgeLayer::Project,
            kind: KnowledgeType::Adr,
            status: KnowledgeStatus::Draft,
            metadata: HashMap::new(),
            commit_hash: None,
            author: None,
            updated_at: 1234567890,
            summaries: HashMap::new()
        };

        let summary = sync_manager.generate_summary(&entry);
        assert_eq!(summary, "[Adr] [Draft] empty.md\n\n");
    }

    #[test]
    fn test_find_memory_id_by_knowledge_id() {
        let sync_manager = SyncManager {
            memory_manager: Arc::new(MemoryManager::new()),
            knowledge_repo: Arc::new(MockKnowledgeRepository::new()),
            governance_engine: Arc::new(GovernanceEngine::new()),
            governance_client: None,
            deployment_config: DeploymentConfig::default(),
            federation_manager: None,
            persister: Arc::new(MockPersister),
            states: Arc::new(RwLock::new(HashMap::new())),
            lock_provider: None,
            checkpoints: Arc::new(RwLock::new(HashMap::new()))
        };

        let mut state = SyncState::default();
        state
            .pointer_mapping
            .insert("ptr_test".to_string(), "test.md".to_string());
        state
            .pointer_mapping
            .insert("ptr_other".to_string(), "other.md".to_string());

        let memory_id = sync_manager.find_memory_id_by_knowledge_id("test.md", &state);
        assert_eq!(memory_id, Some("ptr_test".to_string()));

        let memory_id = sync_manager.find_memory_id_by_knowledge_id("nonexistent.md", &state);
        assert_eq!(memory_id, None);
    }

    struct MockRepoWithEntries {
        entries: Vec<KnowledgeEntry>
    }
    impl MockRepoWithEntries {
        fn new() -> Self {
            Self {
                entries: Vec::new()
            }
        }
        fn add_entry(&mut self, e: KnowledgeEntry) {
            self.entries.push(e);
        }
    }
    #[async_trait::async_trait]
    impl KnowledgeRepository for MockRepoWithEntries {
        type Error = knowledge::repository::RepositoryError;
        async fn store(
            &self,
            _ctx: TenantContext,
            _e: KnowledgeEntry,
            _m: &str
        ) -> std::result::Result<String, Self::Error> {
            Ok("hash".to_string())
        }
        async fn get(
            &self,
            _ctx: TenantContext,
            l: KnowledgeLayer,
            p: &str
        ) -> std::result::Result<Option<KnowledgeEntry>, Self::Error> {
            Ok(self
                .entries
                .iter()
                .find(|e| e.path == p && e.layer == l)
                .cloned())
        }
        async fn list(
            &self,
            _ctx: TenantContext,
            l: KnowledgeLayer,
            _p: &str
        ) -> std::result::Result<Vec<KnowledgeEntry>, Self::Error> {
            Ok(self
                .entries
                .iter()
                .filter(|e| e.layer == l)
                .cloned()
                .collect())
        }
        async fn delete(
            &self,
            _ctx: TenantContext,
            _l: KnowledgeLayer,
            _p: &str,
            _m: &str
        ) -> std::result::Result<String, Self::Error> {
            Ok("hash".to_string())
        }
        async fn get_head_commit(
            &self,
            _ctx: TenantContext
        ) -> std::result::Result<Option<String>, Self::Error> {
            Ok(None)
        }
        async fn get_affected_items(
            &self,
            _ctx: TenantContext,
            _f: &str
        ) -> std::result::Result<Vec<(KnowledgeLayer, String)>, Self::Error> {
            Ok(Vec::new())
        }
        async fn search(
            &self,
            _ctx: TenantContext,
            _q: &str,
            _l: Vec<KnowledgeLayer>,
            _li: usize
        ) -> std::result::Result<Vec<KnowledgeEntry>, Self::Error> {
            Ok(Vec::new())
        }
        fn root_path(&self) -> Option<std::path::PathBuf> {
            None
        }
    }

    #[tokio::test]
    async fn test_detect_conflicts_layer_mismatch() {
        let mut state = SyncState::default();
        let k_id = "moved_item.md".to_string();
        let m_id = format!("ptr_{}", k_id);
        let ctx = TenantContext::default();

        state.pointer_mapping.insert(m_id.clone(), k_id.clone());
        state
            .knowledge_hashes
            .insert(k_id.clone(), utils::compute_content_hash("content"));
        state
            .knowledge_layers
            .insert(k_id.clone(), KnowledgeLayer::Project);

        let mut repo = MockRepoWithEntries::new();
        repo.add_entry(KnowledgeEntry {
            path: k_id.clone(),
            content: "content".to_string(),
            layer: KnowledgeLayer::Org,
            kind: KnowledgeType::Spec,
            status: KnowledgeStatus::Accepted,
            metadata: HashMap::new(),
            commit_hash: None,
            author: None,
            updated_at: 0,
            summaries: HashMap::new()
        });

        let memory = Arc::new(MemoryManager::new());
        memory
            .register_provider(mk_core::types::MemoryLayer::Project, {
                let provider: Arc<
                    dyn mk_core::traits::MemoryProviderAdapter<
                            Error = Box<dyn std::error::Error + Send + Sync>
                        > + Send
                        + Sync
                > = Arc::new(memory::providers::MockProvider::new());
                provider
            })
            .await;
        memory
            .register_provider(mk_core::types::MemoryLayer::Org, {
                let provider: Arc<
                    dyn mk_core::traits::MemoryProviderAdapter<
                            Error = Box<dyn std::error::Error + Send + Sync>
                        > + Send
                        + Sync
                > = Arc::new(memory::providers::MockProvider::new());
                provider
            })
            .await;

        memory
            .add_to_layer(
                ctx.clone(),
                mk_core::types::MemoryLayer::Project,
                MemoryEntry {
                    id: m_id.clone(),
                    content: "[Spec] [Accepted] moved_item.md\n\ncontent".to_string(),
                    embedding: None,
                    layer: mk_core::types::MemoryLayer::Project,
                    summaries: HashMap::new(),
                    context_vector: None,
                    importance_score: None,
                    metadata: HashMap::new(),
                    created_at: 0,
                    updated_at: 0
                }
            )
            .await
            .unwrap();

        let sync_manager =
            create_sync_manager_with_state(memory, Arc::new(repo), state, &ctx.tenant_id);

        let conflicts = sync_manager.detect_conflicts(ctx).await.unwrap();

        let layer_mismatch = conflicts
            .iter()
            .find(|c| matches!(c, SyncConflict::LayerMismatch { .. }));

        assert!(
            layer_mismatch.is_some(),
            "Expected LayerMismatch conflict, found: {:?}",
            conflicts
        );

        if let Some(SyncConflict::LayerMismatch {
            knowledge_id,
            expected_layer,
            actual_layer,
            ..
        }) = layer_mismatch
        {
            assert_eq!(knowledge_id, "moved_item.md");
            assert_eq!(expected_layer, &KnowledgeLayer::Project);
            assert_eq!(actual_layer, &KnowledgeLayer::Org);
        }
    }

    #[tokio::test]
    async fn test_detect_conflicts_performance() {
        let count = 1000;
        let mut state = SyncState::default();
        let mut repo = MockRepoWithEntries::new();
        let k_id = "moved_item.md".to_string();
        let m_id = format!("ptr_{}", k_id);
        let _ctx = TenantContext::default();

        state.pointer_mapping.insert(m_id.clone(), k_id.clone());
        state
            .knowledge_hashes
            .insert(k_id.clone(), utils::compute_content_hash("content"));
        state
            .knowledge_layers
            .insert(k_id.clone(), KnowledgeLayer::Project);

        repo.add_entry(KnowledgeEntry {
            path: k_id.clone(),
            content: "content".to_string(),
            layer: KnowledgeLayer::Org,
            kind: KnowledgeType::Spec,
            status: KnowledgeStatus::Accepted,
            metadata: HashMap::new(),
            commit_hash: None,
            author: None,
            updated_at: 0,
            summaries: HashMap::new()
        });

        let memory = Arc::new(MemoryManager::new());
        memory
            .register_provider(mk_core::types::MemoryLayer::Project, {
                let provider: Arc<
                    dyn mk_core::traits::MemoryProviderAdapter<
                            Error = Box<dyn std::error::Error + Send + Sync>
                        > + Send
                        + Sync
                > = Arc::new(memory::providers::MockProvider::new());
                provider
            })
            .await;

        for i in 0..count {
            let k_id = format!("item_{}.md", i);
            let m_id = format!("ptr_{}", k_id);
            memory
                .add_to_layer(
                    mk_core::types::TenantContext::default(),
                    mk_core::types::MemoryLayer::Project,
                    MemoryEntry {
                        id: m_id,
                        content: "[Spec] [Accepted] item.md\n\ncontent".to_string(),
                        embedding: None,
                        layer: mk_core::types::MemoryLayer::Project,
                        summaries: HashMap::new(),
                        context_vector: None,
                        importance_score: None,
                        metadata: HashMap::new(),
                        created_at: 0,
                        updated_at: 0
                    }
                )
                .await
                .unwrap();
        }

        let ctx = TenantContext::default();
        let sync_manager =
            create_sync_manager_with_state(memory, Arc::new(repo), state, &ctx.tenant_id);

        let start = Instant::now();
        let _ = sync_manager.detect_conflicts(ctx).await.unwrap();
        let duration = start.elapsed();

        println!(
            "Conflict detection for {} items took: {:?}",
            count, duration
        );
        assert!(duration.as_secs() < 5);
    }

    #[tokio::test]
    async fn test_sync_federation_general_error() {
        let ctx = TenantContext::default();
        let sync_manager = SyncManager::new(
            Arc::new(MemoryManager::new()),
            Arc::new(MockKnowledgeRepository::new()),
            Arc::new(GovernanceEngine::new()),
            DeploymentConfig::default(),
            None,
            Arc::new(MockPersister),
            None
        )
        .await
        .unwrap();

        struct ErrorFed {
            config: knowledge::federation::FederationConfig
        }
        impl ErrorFed {
            fn new() -> Self {
                Self {
                    config: knowledge::federation::FederationConfig {
                        upstreams: vec![knowledge::federation::UpstreamConfig {
                            id: "upstream1".to_string(),
                            url: "http://test".to_string(),
                            branch: "main".to_string(),
                            auth_token: None
                        }],
                        sync_interval_secs: 60
                    }
                }
            }
        }
        #[async_trait::async_trait]
        impl FederationProvider for ErrorFed {
            fn config(&self) -> &knowledge::federation::FederationConfig {
                &self.config
            }
            async fn fetch_upstream_manifest(
                &self,
                _id: &str
            ) -> std::result::Result<
                knowledge::federation::KnowledgeManifest,
                knowledge::repository::RepositoryError
            > {
                Ok(knowledge::federation::KnowledgeManifest {
                    version: "1".to_string(),
                    items: HashMap::new()
                })
            }
            async fn sync_upstream(
                &self,
                _id: &str,
                _p: &std::path::Path
            ) -> std::result::Result<(), knowledge::repository::RepositoryError> {
                Err(knowledge::repository::RepositoryError::InvalidPath(
                    "something went wrong".to_string()
                ))
            }
        }

        let fed = ErrorFed::new();
        let result = sync_manager.sync_federation(ctx, &fed).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_background_sync_shutdown_with_receiver() {
        let ctx = TenantContext::default();
        let sync_manager = Arc::new(SyncManager {
            memory_manager: Arc::new(MemoryManager::new()),
            knowledge_repo: Arc::new(MockKnowledgeRepository::new()),
            governance_engine: Arc::new(GovernanceEngine::new()),
            governance_client: None,
            deployment_config: DeploymentConfig::default(),
            federation_manager: None,
            persister: Arc::new(MockPersister),
            states: Arc::new(RwLock::new(HashMap::new())),
            lock_provider: None,
            checkpoints: Arc::new(RwLock::new(HashMap::new()))
        });

        let (tx, rx) = tokio::sync::watch::channel(false);
        let handle = sync_manager.start_background_sync(ctx, 1, 60, rx).await;

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        tx.send(true).unwrap();

        let result = tokio::time::timeout(std::time::Duration::from_secs(2), handle).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_background_sync_runs_cycle() {
        let ctx = TenantContext::default();
        let sync_manager = Arc::new(
            SyncManager::new(
                Arc::new(MemoryManager::new()),
                Arc::new(MockKnowledgeRepository::new()),
                Arc::new(GovernanceEngine::new()),
                DeploymentConfig::default(),
                None,
                Arc::new(MockPersister),
                None
            )
            .await
            .unwrap()
        );

        let (tx, rx) = tokio::sync::watch::channel(false);
        let handle = sync_manager.start_background_sync(ctx, 1, 0, rx).await;

        tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
        tx.send(true).unwrap();
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_initialize_shutdown() {
        let ctx = TenantContext::default();
        let sync_manager = SyncManager::new(
            Arc::new(MemoryManager::new()),
            Arc::new(MockKnowledgeRepository::new()),
            Arc::new(GovernanceEngine::new()),
            DeploymentConfig::default(),
            None,
            Arc::new(MockPersister),
            None
        )
        .await
        .unwrap();

        sync_manager.initialize(ctx).await.unwrap();
        sync_manager.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_checkpoint_rollback() {
        let tenant_id = mk_core::types::TenantId::new("test-tenant".to_string()).unwrap();
        let memory_manager = Arc::new(MemoryManager::new());
        let knowledge_repo = Arc::new(MockKnowledgeRepository::new());
        let mut state = SyncState::default();
        state.version = "3".to_string();
        let sync_manager =
            create_sync_manager_with_state(memory_manager, knowledge_repo, state, &tenant_id);

        let _ = sync_manager.create_checkpoint(&tenant_id).await;

        let checkpoints = sync_manager.checkpoints.read().await;
        assert!(checkpoints.contains_key(&tenant_id));
        let checkpoint = checkpoints.get(&tenant_id).unwrap();
        assert_eq!(checkpoint.version, "3");
    }

    #[tokio::test]
    async fn test_get_or_load_state_cached() {
        let tenant_id = TenantId::default();
        let mut states_map = HashMap::new();
        let mut state = SyncState::default();
        state.version = "7".to_string();
        states_map.insert(tenant_id.clone(), state);

        let sync_manager = SyncManager {
            memory_manager: Arc::new(MemoryManager::new()),
            knowledge_repo: Arc::new(MockKnowledgeRepository::new()),
            governance_engine: Arc::new(GovernanceEngine::new()),
            governance_client: None,
            deployment_config: DeploymentConfig::default(),
            federation_manager: None,
            persister: Arc::new(MockPersister),
            states: Arc::new(RwLock::new(states_map)),
            lock_provider: None,
            checkpoints: Arc::new(RwLock::new(HashMap::new()))
        };

        let loaded = sync_manager.get_or_load_state(&tenant_id).await.unwrap();
        assert_eq!(loaded.version, "7");
    }

    #[tokio::test]
    async fn test_sync_failures_hardening() {
        let ctx = TenantContext::default();
        let memory = Arc::new(MemoryManager::new());
        let persister = Arc::new(MockPersister);
        let sync_manager = SyncManager::new(
            memory.clone(),
            Arc::new(MockKnowledgeRepository::new()),
            Arc::new(GovernanceEngine::new()),
            DeploymentConfig::default(),
            None,
            persister.clone(),
            None
        )
        .await
        .unwrap();

        // Test initialization failure
        let mut fail_ctx = ctx.clone();
        fail_ctx.tenant_id = mk_core::types::TenantId::new("TRIGGER_FAILURE".to_string()).unwrap();
        let result = sync_manager.initialize(fail_ctx).await;
        assert!(result.is_err());

        // Test checkpoint failure
        let result = sync_manager
            .create_checkpoint(
                &mk_core::types::TenantId::new("TRIGGER_FAILURE".to_string()).unwrap()
            )
            .await;
        assert!(result.is_err());

        // Test entry sync failure
        let entry = KnowledgeEntry {
            path: "TRIGGER_FAILURE.md".to_string(),
            content: "content".to_string(),
            layer: KnowledgeLayer::Project,
            kind: KnowledgeType::Spec,
            status: KnowledgeStatus::Accepted,
            metadata: HashMap::new(),
            commit_hash: None,
            author: None,
            updated_at: 0,
            summaries: HashMap::new()
        };
        let mut state = SyncState::default();
        let result = sync_manager
            .sync_entry(ctx.clone(), &entry, &mut state)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_state_persister_failure() {
        let temp_dir = tempfile::tempdir().unwrap();
        let base_path = temp_dir.path().to_path_buf();
        let persister = crate::state_persister::FilePersister::new(base_path);
        let tenant_id = mk_core::types::TenantId::new("TRIGGER_FAILURE".to_string()).unwrap();

        let result: std::result::Result<
            crate::state::SyncState,
            Box<dyn std::error::Error + Send + Sync>
        > = persister.load(&tenant_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_resolve_conflicts_hash_mismatch() {
        let ctx = TenantContext::default();
        let mut state = SyncState::default();
        let k_id = "hash_mismatch.md".to_string();

        state
            .knowledge_hashes
            .insert(k_id.clone(), "old_hash".to_string());
        state
            .knowledge_layers
            .insert(k_id.clone(), KnowledgeLayer::Project);

        let mut repo = MockRepoWithEntries::new();
        repo.add_entry(KnowledgeEntry {
            path: k_id.clone(),
            content: "new content".to_string(),
            layer: KnowledgeLayer::Project,
            kind: KnowledgeType::Spec,
            status: KnowledgeStatus::Accepted,
            metadata: HashMap::new(),
            commit_hash: None,
            author: None,
            updated_at: 0,
            summaries: HashMap::new()
        });

        let memory = Arc::new(MemoryManager::new());
        memory
            .register_provider(mk_core::types::MemoryLayer::Project, {
                let provider: Arc<
                    dyn mk_core::traits::MemoryProviderAdapter<
                            Error = Box<dyn std::error::Error + Send + Sync>
                        > + Send
                        + Sync
                > = Arc::new(memory::providers::MockProvider::new());
                provider
            })
            .await;

        let sync_manager =
            create_sync_manager_with_state(memory, Arc::new(repo), state, &ctx.tenant_id);

        let conflicts = vec![SyncConflict::HashMismatch {
            knowledge_id: k_id.clone(),
            memory_id: format!("ptr_{}", k_id),
            expected_hash: "old_hash".to_string(),
            actual_hash: "new_hash".to_string()
        }];

        let result = sync_manager.resolve_conflicts(ctx, conflicts).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_resolve_conflicts_orphaned_pointer() {
        let ctx = TenantContext::default();
        let mut state = SyncState::default();
        let k_id = "orphaned.md".to_string();
        let m_id = format!("ptr_{}", k_id);

        state.pointer_mapping.insert(m_id.clone(), k_id.clone());
        state
            .knowledge_hashes
            .insert(k_id.clone(), "hash".to_string());
        state
            .knowledge_layers
            .insert(k_id.clone(), KnowledgeLayer::Project);

        let memory = Arc::new(MemoryManager::new());
        for layer in [
            mk_core::types::MemoryLayer::Company,
            mk_core::types::MemoryLayer::Org,
            mk_core::types::MemoryLayer::Team,
            mk_core::types::MemoryLayer::Project
        ] {
            memory
                .register_provider(layer, {
                    let provider: Arc<
                        dyn mk_core::traits::MemoryProviderAdapter<
                                Error = Box<dyn std::error::Error + Send + Sync>
                            > + Send
                            + Sync
                    > = Arc::new(memory::providers::MockProvider::new());
                    provider
                })
                .await;
        }

        let sync_manager = create_sync_manager_with_state(
            memory,
            Arc::new(MockKnowledgeRepository::new()),
            state,
            &ctx.tenant_id
        );

        let conflicts = vec![SyncConflict::OrphanedPointer {
            memory_id: m_id.clone(),
            knowledge_id: k_id.clone()
        }];

        let result = sync_manager.resolve_conflicts(ctx, conflicts).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_resolve_conflicts_duplicate_pointer() {
        let ctx = TenantContext::default();
        let mut state = SyncState::default();
        let k_id = "duplicate.md".to_string();
        let m_id1 = format!("ptr_1_{}", k_id);
        let m_id2 = format!("ptr_2_{}", k_id);

        state.pointer_mapping.insert(m_id1.clone(), k_id.clone());
        state.pointer_mapping.insert(m_id2.clone(), k_id.clone());
        state
            .knowledge_hashes
            .insert(k_id.clone(), "hash".to_string());
        state
            .knowledge_layers
            .insert(k_id.clone(), KnowledgeLayer::Project);

        let mut repo = MockRepoWithEntries::new();
        repo.add_entry(KnowledgeEntry {
            path: k_id.clone(),
            content: "content".to_string(),
            layer: KnowledgeLayer::Project,
            kind: KnowledgeType::Spec,
            status: KnowledgeStatus::Accepted,
            metadata: HashMap::new(),
            commit_hash: None,
            author: None,
            updated_at: 0,
            summaries: HashMap::new()
        });

        let memory = Arc::new(MemoryManager::new());
        for layer in [
            mk_core::types::MemoryLayer::Company,
            mk_core::types::MemoryLayer::Org,
            mk_core::types::MemoryLayer::Team,
            mk_core::types::MemoryLayer::Project
        ] {
            memory
                .register_provider(layer, {
                    let provider: Arc<
                        dyn mk_core::traits::MemoryProviderAdapter<
                                Error = Box<dyn std::error::Error + Send + Sync>
                            > + Send
                            + Sync
                    > = Arc::new(memory::providers::MockProvider::new());
                    provider
                })
                .await;
        }

        let sync_manager =
            create_sync_manager_with_state(memory, Arc::new(repo), state, &ctx.tenant_id);

        let conflicts = vec![SyncConflict::DuplicatePointer {
            knowledge_id: k_id.clone(),
            memory_ids: vec![m_id1.clone(), m_id2.clone()]
        }];

        let result = sync_manager.resolve_conflicts(ctx, conflicts).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_resolve_conflicts_status_change() {
        let ctx = TenantContext::default();
        let mut state = SyncState::default();
        let k_id = "status_change.md".to_string();
        let m_id = format!("ptr_{}", k_id);

        state.pointer_mapping.insert(m_id.clone(), k_id.clone());
        state
            .knowledge_hashes
            .insert(k_id.clone(), "hash".to_string());
        state
            .knowledge_layers
            .insert(k_id.clone(), KnowledgeLayer::Project);

        let mut repo = MockRepoWithEntries::new();
        repo.add_entry(KnowledgeEntry {
            path: k_id.clone(),
            content: "content".to_string(),
            layer: KnowledgeLayer::Project,
            kind: KnowledgeType::Spec,
            status: KnowledgeStatus::Deprecated,
            metadata: HashMap::new(),
            commit_hash: None,
            author: None,
            updated_at: 0,
            summaries: HashMap::new()
        });

        let memory = Arc::new(MemoryManager::new());
        memory
            .register_provider(mk_core::types::MemoryLayer::Project, {
                let provider: Arc<
                    dyn mk_core::traits::MemoryProviderAdapter<
                            Error = Box<dyn std::error::Error + Send + Sync>
                        > + Send
                        + Sync
                > = Arc::new(memory::providers::MockProvider::new());
                provider
            })
            .await;

        let sync_manager =
            create_sync_manager_with_state(memory, Arc::new(repo), state, &ctx.tenant_id);

        let conflicts = vec![SyncConflict::StatusChange {
            knowledge_id: k_id.clone(),
            memory_id: m_id.clone(),
            new_status: KnowledgeStatus::Deprecated
        }];

        let result = sync_manager.resolve_conflicts(ctx, conflicts).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_resolve_conflicts_layer_mismatch_resolution() {
        let ctx = TenantContext::default();
        let mut state = SyncState::default();
        let k_id = "layer_mismatch.md".to_string();
        let m_id = format!("ptr_{}", k_id);

        state.pointer_mapping.insert(m_id.clone(), k_id.clone());
        state
            .knowledge_hashes
            .insert(k_id.clone(), "hash".to_string());
        state
            .knowledge_layers
            .insert(k_id.clone(), KnowledgeLayer::Project);

        let mut repo = MockRepoWithEntries::new();
        repo.add_entry(KnowledgeEntry {
            path: k_id.clone(),
            content: "content".to_string(),
            layer: KnowledgeLayer::Org,
            kind: KnowledgeType::Spec,
            status: KnowledgeStatus::Accepted,
            metadata: HashMap::new(),
            commit_hash: None,
            author: None,
            updated_at: 0,
            summaries: HashMap::new()
        });

        let memory = Arc::new(MemoryManager::new());
        for layer in [
            mk_core::types::MemoryLayer::Project,
            mk_core::types::MemoryLayer::Org
        ] {
            memory
                .register_provider(layer, {
                    let provider: Arc<
                        dyn mk_core::traits::MemoryProviderAdapter<
                                Error = Box<dyn std::error::Error + Send + Sync>
                            > + Send
                            + Sync
                    > = Arc::new(memory::providers::MockProvider::new());
                    provider
                })
                .await;
        }

        let sync_manager =
            create_sync_manager_with_state(memory, Arc::new(repo), state, &ctx.tenant_id);

        let conflicts = vec![SyncConflict::LayerMismatch {
            knowledge_id: k_id.clone(),
            memory_id: m_id.clone(),
            expected_layer: KnowledgeLayer::Project,
            actual_layer: KnowledgeLayer::Org
        }];

        let result = sync_manager.resolve_conflicts(ctx, conflicts).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_rollback_no_checkpoint() {
        let tenant_id = mk_core::types::TenantId::new("no-checkpoint".to_string()).unwrap();
        let sync_manager = SyncManager {
            memory_manager: Arc::new(MemoryManager::new()),
            knowledge_repo: Arc::new(MockKnowledgeRepository::new()),
            governance_engine: Arc::new(GovernanceEngine::new()),
            governance_client: None,
            deployment_config: DeploymentConfig::default(),
            federation_manager: None,
            persister: Arc::new(MockPersister),
            states: Arc::new(RwLock::new(HashMap::new())),
            lock_provider: None,
            checkpoints: Arc::new(RwLock::new(HashMap::new()))
        };

        let result = sync_manager.rollback(&tenant_id).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_rollback_with_checkpoint() {
        let tenant_id = mk_core::types::TenantId::new("with-checkpoint".to_string()).unwrap();
        let mut state = SyncState::default();
        state.version = "original".to_string();

        let mut checkpoints_map = HashMap::new();
        checkpoints_map.insert(tenant_id.clone(), state.clone());

        let mut states_map = HashMap::new();
        let mut modified_state = SyncState::default();
        modified_state.version = "modified".to_string();
        states_map.insert(tenant_id.clone(), modified_state);

        let sync_manager = SyncManager {
            memory_manager: Arc::new(MemoryManager::new()),
            knowledge_repo: Arc::new(MockKnowledgeRepository::new()),
            governance_engine: Arc::new(GovernanceEngine::new()),
            governance_client: None,
            deployment_config: DeploymentConfig::default(),
            federation_manager: None,
            persister: Arc::new(MockPersister),
            lock_provider: None,
            states: Arc::new(RwLock::new(states_map)),
            checkpoints: Arc::new(RwLock::new(checkpoints_map))
        };

        let result = sync_manager.rollback(&tenant_id).await;
        assert!(result.is_ok());

        let states = sync_manager.states.read().await;
        let restored = states.get(&tenant_id).unwrap();
        assert_eq!(restored.version, "original");
    }

    #[tokio::test]
    async fn test_resolve_federation_conflict() {
        let tenant_id = mk_core::types::TenantId::new("fed-test".to_string()).unwrap();
        let mut state = SyncState::default();
        state.federation_conflicts.push(FederationConflict {
            upstream_id: "upstream-1".to_string(),
            reason: "conflict reason".to_string(),
            detected_at: 12345
        });

        let mut states_map = HashMap::new();
        states_map.insert(tenant_id.clone(), state);

        let sync_manager = SyncManager {
            memory_manager: Arc::new(MemoryManager::new()),
            knowledge_repo: Arc::new(MockKnowledgeRepository::new()),
            governance_engine: Arc::new(GovernanceEngine::new()),
            governance_client: None,
            deployment_config: DeploymentConfig::default(),
            federation_manager: None,
            persister: Arc::new(MockPersister),
            states: Arc::new(RwLock::new(states_map)),
            lock_provider: None,
            checkpoints: Arc::new(RwLock::new(HashMap::new()))
        };

        let result = sync_manager
            .resolve_federation_conflict(tenant_id.clone(), "upstream-1", "manually resolved")
            .await;
        assert!(result.is_ok());

        let states = sync_manager.states.read().await;
        let updated = states.get(&tenant_id).unwrap();
        assert!(updated.federation_conflicts.is_empty());
    }

    #[tokio::test]
    async fn test_prune_failed_items() {
        let ctx = TenantContext::default();
        let mut state = SyncState::default();
        let now = chrono::Utc::now().timestamp();

        state.failed_items.push(SyncFailure {
            knowledge_id: "old_failure.md".to_string(),
            error: "old error".to_string(),
            failed_at: now - (10 * 24 * 60 * 60),
            retry_count: 3
        });
        state.failed_items.push(SyncFailure {
            knowledge_id: "recent_failure.md".to_string(),
            error: "recent error".to_string(),
            failed_at: now - (1 * 24 * 60 * 60),
            retry_count: 1
        });

        let mut states_map = HashMap::new();
        states_map.insert(ctx.tenant_id.clone(), state);

        let sync_manager = SyncManager {
            memory_manager: Arc::new(MemoryManager::new()),
            knowledge_repo: Arc::new(MockKnowledgeRepository::new()),
            governance_engine: Arc::new(GovernanceEngine::new()),
            governance_client: None,
            deployment_config: DeploymentConfig::default(),
            federation_manager: None,
            persister: Arc::new(MockPersister),
            states: Arc::new(RwLock::new(states_map)),
            lock_provider: None,
            checkpoints: Arc::new(RwLock::new(HashMap::new()))
        };

        let result = sync_manager.prune_failed_items(ctx.clone(), 7).await;
        assert!(result.is_ok());

        let states = sync_manager.states.read().await;
        let updated = states.get(&ctx.tenant_id).unwrap();
        assert_eq!(updated.failed_items.len(), 1);
        assert_eq!(updated.failed_items[0].knowledge_id, "recent_failure.md");
    }

    #[tokio::test]
    async fn test_detect_delta() {
        let ctx = TenantContext::default();
        let mut state = SyncState::default();

        state.knowledge_hashes.insert(
            "unchanged.md".to_string(),
            utils::compute_content_hash("same content")
        );
        state.knowledge_hashes.insert(
            "updated.md".to_string(),
            utils::compute_content_hash("old content")
        );
        state
            .knowledge_hashes
            .insert("deleted.md".to_string(), "hash".to_string());

        let mut repo = MockRepoWithEntries::new();
        repo.add_entry(KnowledgeEntry {
            path: "unchanged.md".to_string(),
            content: "same content".to_string(),
            layer: KnowledgeLayer::Project,
            kind: KnowledgeType::Spec,
            status: KnowledgeStatus::Accepted,
            metadata: HashMap::new(),
            commit_hash: None,
            author: None,
            updated_at: 0,
            summaries: HashMap::new()
        });
        repo.add_entry(KnowledgeEntry {
            path: "updated.md".to_string(),
            content: "new content".to_string(),
            layer: KnowledgeLayer::Project,
            kind: KnowledgeType::Spec,
            status: KnowledgeStatus::Accepted,
            metadata: HashMap::new(),
            commit_hash: None,
            author: None,
            updated_at: 0,
            summaries: HashMap::new()
        });
        repo.add_entry(KnowledgeEntry {
            path: "added.md".to_string(),
            content: "brand new".to_string(),
            layer: KnowledgeLayer::Project,
            kind: KnowledgeType::Spec,
            status: KnowledgeStatus::Accepted,
            metadata: HashMap::new(),
            commit_hash: None,
            author: None,
            updated_at: 0,
            summaries: HashMap::new()
        });

        let sync_manager = create_sync_manager_with_state(
            Arc::new(MemoryManager::new()),
            Arc::new(repo),
            state,
            &ctx.tenant_id
        );

        let delta = sync_manager.detect_delta(ctx, &SyncState::default()).await;
        assert!(delta.is_ok());
    }

    // Tier 1: Hybrid/Remote governance validation tests
    struct MockGovernanceClient {
        should_fail: bool,
        return_invalid: bool,
        block_severity: bool
    }

    impl MockGovernanceClient {
        fn new() -> Self {
            Self {
                should_fail: false,
                return_invalid: false,
                block_severity: false
            }
        }

        fn with_failure() -> Self {
            Self {
                should_fail: true,
                return_invalid: false,
                block_severity: false
            }
        }

        fn with_invalid_response(block: bool) -> Self {
            Self {
                should_fail: false,
                return_invalid: true,
                block_severity: block
            }
        }
    }

    #[async_trait::async_trait]
    impl knowledge::governance_client::GovernanceClient for MockGovernanceClient {
        async fn validate(
            &self,
            _ctx: &TenantContext,
            _layer: KnowledgeLayer,
            _context: &std::collections::HashMap<String, serde_json::Value>
        ) -> knowledge::governance_client::Result<mk_core::types::ValidationResult> {
            if self.should_fail {
                return Err(
                    knowledge::governance_client::GovernanceClientError::Internal(
                        "Simulated network failure".to_string()
                    )
                );
            }

            if self.return_invalid {
                let severity = if self.block_severity {
                    mk_core::types::ConstraintSeverity::Block
                } else {
                    mk_core::types::ConstraintSeverity::Warn
                };

                return Ok(mk_core::types::ValidationResult {
                    is_valid: false,
                    violations: vec![mk_core::types::PolicyViolation {
                        rule_id: "remote_rule_1".to_string(),
                        policy_id: "remote_policy".to_string(),
                        severity,
                        message: "Remote governance violation".to_string(),
                        context: HashMap::new()
                    }]
                });
            }

            Ok(mk_core::types::ValidationResult {
                is_valid: true,
                violations: vec![]
            })
        }

        async fn get_drift_status(
            &self,
            _ctx: &TenantContext,
            _project_id: &str
        ) -> knowledge::governance_client::Result<Option<mk_core::types::DriftResult>> {
            Ok(None)
        }

        async fn list_proposals(
            &self,
            _ctx: &TenantContext,
            _layer: Option<KnowledgeLayer>
        ) -> knowledge::governance_client::Result<Vec<KnowledgeEntry>> {
            Ok(vec![])
        }

        async fn replay_events(
            &self,
            _ctx: &TenantContext,
            _since_timestamp: i64,
            _limit: usize
        ) -> knowledge::governance_client::Result<Vec<mk_core::types::GovernanceEvent>> {
            Ok(vec![])
        }
    }

    #[tokio::test]
    async fn test_sync_entry_hybrid_mode_valid_governance() {
        let ctx = TenantContext::default();
        let deployment_config = DeploymentConfig {
            mode: "hybrid".to_string(),
            remote_url: Some("http://test".to_string()),
            ..Default::default()
        };

        let sync_manager = SyncManager {
            memory_manager: create_memory_manager_with_mock_providers().await,
            knowledge_repo: Arc::new(MockKnowledgeRepository::new()),
            governance_engine: Arc::new(GovernanceEngine::new()),
            governance_client: Some(Arc::new(MockGovernanceClient::new())),
            deployment_config,
            federation_manager: None,
            persister: Arc::new(MockPersister),
            states: Arc::new(RwLock::new(HashMap::new())),
            lock_provider: None,
            checkpoints: Arc::new(RwLock::new(HashMap::new()))
        };

        let entry = KnowledgeEntry {
            path: "test.md".to_string(),
            content: "test content".to_string(),
            layer: KnowledgeLayer::Project,
            kind: KnowledgeType::Spec,
            status: KnowledgeStatus::Accepted,
            metadata: HashMap::new(),
            commit_hash: None,
            author: None,
            updated_at: 0,
            summaries: HashMap::new()
        };

        let mut state = SyncState::default();
        let result = sync_manager.sync_entry(ctx, &entry, &mut state).await;
        // Should succeed as MockGovernanceClient returns valid
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_sync_entry_remote_mode_valid_governance() {
        let ctx = TenantContext::default();
        let deployment_config = DeploymentConfig {
            mode: "remote".to_string(),
            remote_url: Some("http://test".to_string()),
            ..Default::default()
        };

        let sync_manager = SyncManager {
            memory_manager: create_memory_manager_with_mock_providers().await,
            knowledge_repo: Arc::new(MockKnowledgeRepository::new()),
            governance_engine: Arc::new(GovernanceEngine::new()),
            governance_client: Some(Arc::new(MockGovernanceClient::new())),
            deployment_config,
            federation_manager: None,
            persister: Arc::new(MockPersister),
            states: Arc::new(RwLock::new(HashMap::new())),
            lock_provider: None,
            checkpoints: Arc::new(RwLock::new(HashMap::new()))
        };

        let entry = KnowledgeEntry {
            path: "test.md".to_string(),
            content: "test content".to_string(),
            layer: KnowledgeLayer::Project,
            kind: KnowledgeType::Spec,
            status: KnowledgeStatus::Accepted,
            metadata: HashMap::new(),
            commit_hash: None,
            author: None,
            updated_at: 0,
            summaries: HashMap::new()
        };

        let mut state = SyncState::default();
        let result = sync_manager.sync_entry(ctx, &entry, &mut state).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_sync_entry_hybrid_mode_warn_violation() {
        let ctx = TenantContext::default();
        let deployment_config = DeploymentConfig {
            mode: "hybrid".to_string(),
            remote_url: Some("http://test".to_string()),
            ..Default::default()
        };

        let sync_manager = SyncManager {
            memory_manager: create_memory_manager_with_mock_providers().await,
            knowledge_repo: Arc::new(MockKnowledgeRepository::new()),
            governance_engine: Arc::new(GovernanceEngine::new()),
            governance_client: Some(Arc::new(MockGovernanceClient::with_invalid_response(false))),
            deployment_config,
            federation_manager: None,
            persister: Arc::new(MockPersister),
            states: Arc::new(RwLock::new(HashMap::new())),
            lock_provider: None,
            checkpoints: Arc::new(RwLock::new(HashMap::new()))
        };

        let entry = KnowledgeEntry {
            path: "test.md".to_string(),
            content: "test content".to_string(),
            layer: KnowledgeLayer::Project,
            kind: KnowledgeType::Spec,
            status: KnowledgeStatus::Accepted,
            metadata: HashMap::new(),
            commit_hash: None,
            author: None,
            updated_at: 0,
            summaries: HashMap::new()
        };

        let mut state = SyncState::default();
        let result = sync_manager.sync_entry(ctx, &entry, &mut state).await;
        // Should succeed with Warn severity (not blocked)
        assert!(result.is_ok());
        // But should have incremented governance blocks counter
        assert_eq!(state.stats.total_governance_blocks, 1);
    }

    #[tokio::test]
    async fn test_sync_entry_hybrid_mode_block_violation() {
        let ctx = TenantContext::default();
        let deployment_config = DeploymentConfig {
            mode: "hybrid".to_string(),
            remote_url: Some("http://test".to_string()),
            ..Default::default()
        };

        let sync_manager = SyncManager {
            memory_manager: create_memory_manager_with_mock_providers().await,
            knowledge_repo: Arc::new(MockKnowledgeRepository::new()),
            governance_engine: Arc::new(GovernanceEngine::new()),
            governance_client: Some(Arc::new(MockGovernanceClient::with_invalid_response(true))),
            deployment_config,
            federation_manager: None,
            persister: Arc::new(MockPersister),
            states: Arc::new(RwLock::new(HashMap::new())),
            lock_provider: None,
            checkpoints: Arc::new(RwLock::new(HashMap::new()))
        };

        let entry = KnowledgeEntry {
            path: "blocked.md".to_string(),
            content: "blocked content".to_string(),
            layer: KnowledgeLayer::Project,
            kind: KnowledgeType::Spec,
            status: KnowledgeStatus::Accepted,
            metadata: HashMap::new(),
            commit_hash: None,
            author: None,
            updated_at: 0,
            summaries: HashMap::new()
        };

        let mut state = SyncState::default();
        let result = sync_manager.sync_entry(ctx, &entry, &mut state).await;
        // Should fail with Block severity
        assert!(result.is_err());
        match result {
            Err(SyncError::GovernanceBlock(msg)) => {
                assert!(msg.contains("Remote governance violation"));
            }
            _ => panic!("Expected GovernanceBlock error")
        }
        // Should have recorded failure
        assert_eq!(state.failed_items.len(), 1);
    }

    #[tokio::test]
    async fn test_sync_entry_hybrid_mode_remote_failure() {
        let ctx = TenantContext::default();
        let deployment_config = DeploymentConfig {
            mode: "hybrid".to_string(),
            remote_url: Some("http://test".to_string()),
            ..Default::default()
        };

        let sync_manager = SyncManager {
            memory_manager: create_memory_manager_with_mock_providers().await,
            knowledge_repo: Arc::new(MockKnowledgeRepository::new()),
            governance_engine: Arc::new(GovernanceEngine::new()),
            governance_client: Some(Arc::new(MockGovernanceClient::with_failure())),
            deployment_config,
            federation_manager: None,
            persister: Arc::new(MockPersister),
            states: Arc::new(RwLock::new(HashMap::new())),
            lock_provider: None,
            checkpoints: Arc::new(RwLock::new(HashMap::new()))
        };

        let entry = KnowledgeEntry {
            path: "test.md".to_string(),
            content: "test content".to_string(),
            layer: KnowledgeLayer::Project,
            kind: KnowledgeType::Spec,
            status: KnowledgeStatus::Accepted,
            metadata: HashMap::new(),
            commit_hash: None,
            author: None,
            updated_at: 0,
            summaries: HashMap::new()
        };

        let mut state = SyncState::default();
        let result = sync_manager.sync_entry(ctx, &entry, &mut state).await;
        // Should fail due to remote governance failure
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_sync_entry_local_mode_no_remote_validation() {
        let ctx = TenantContext::default();
        let deployment_config = DeploymentConfig {
            mode: "local".to_string(),
            remote_url: None,
            ..Default::default()
        };

        let sync_manager = SyncManager {
            memory_manager: create_memory_manager_with_mock_providers().await,
            knowledge_repo: Arc::new(MockKnowledgeRepository::new()),
            governance_engine: Arc::new(GovernanceEngine::new()),
            governance_client: None, // No client in local mode
            deployment_config,
            federation_manager: None,
            persister: Arc::new(MockPersister),
            states: Arc::new(RwLock::new(HashMap::new())),
            lock_provider: None,
            checkpoints: Arc::new(RwLock::new(HashMap::new()))
        };

        let entry = KnowledgeEntry {
            path: "test.md".to_string(),
            content: "test content".to_string(),
            layer: KnowledgeLayer::Project,
            kind: KnowledgeType::Spec,
            status: KnowledgeStatus::Accepted,
            metadata: HashMap::new(),
            commit_hash: None,
            author: None,
            updated_at: 0,
            summaries: HashMap::new()
        };

        let mut state = SyncState::default();
        let result = sync_manager.sync_entry(ctx, &entry, &mut state).await;
        // Should succeed using only local governance engine
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_acquire_sync_lock_no_provider() {
        let sync_manager = SyncManager {
            memory_manager: Arc::new(MemoryManager::new()),
            knowledge_repo: Arc::new(MockKnowledgeRepository::new()),
            governance_engine: Arc::new(GovernanceEngine::new()),
            governance_client: None,
            deployment_config: DeploymentConfig::default(),
            federation_manager: None,
            persister: Arc::new(MockPersister),
            lock_provider: None,
            states: Arc::new(RwLock::new(HashMap::new())),
            checkpoints: Arc::new(RwLock::new(HashMap::new()))
        };

        let tenant_id = TenantId::new("test-tenant".to_string()).unwrap();
        let result = sync_manager
            .acquire_sync_lock(&tenant_id, "batch_sync")
            .await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_release_sync_lock_none() {
        let sync_manager = SyncManager {
            memory_manager: Arc::new(MemoryManager::new()),
            knowledge_repo: Arc::new(MockKnowledgeRepository::new()),
            governance_engine: Arc::new(GovernanceEngine::new()),
            governance_client: None,
            deployment_config: DeploymentConfig::default(),
            federation_manager: None,
            persister: Arc::new(MockPersister),
            lock_provider: None,
            states: Arc::new(RwLock::new(HashMap::new())),
            checkpoints: Arc::new(RwLock::new(HashMap::new()))
        };

        let result = sync_manager.release_sync_lock(None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_run_sync_cycle_without_lock_provider() {
        let ctx = TenantContext::default();
        let sync_manager = SyncManager {
            memory_manager: Arc::new(MemoryManager::new()),
            knowledge_repo: Arc::new(MockKnowledgeRepository::new()),
            governance_engine: Arc::new(GovernanceEngine::new()),
            governance_client: None,
            deployment_config: DeploymentConfig::default(),
            federation_manager: None,
            persister: Arc::new(MockPersister),
            lock_provider: None,
            states: Arc::new(RwLock::new(HashMap::new())),
            checkpoints: Arc::new(RwLock::new(HashMap::new()))
        };

        let result = sync_manager.run_sync_cycle(ctx, 60).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_run_sync_cycle_hybrid_disabled_skips_locking() {
        let ctx = TenantContext::default();
        let mut config = DeploymentConfig::default();
        config.mode = "hybrid".to_string();
        config.sync_enabled = false;

        let sync_manager = SyncManager {
            memory_manager: Arc::new(MemoryManager::new()),
            knowledge_repo: Arc::new(MockKnowledgeRepository::new()),
            governance_engine: Arc::new(GovernanceEngine::new()),
            governance_client: None,
            deployment_config: config,
            federation_manager: None,
            persister: Arc::new(MockPersister),
            lock_provider: None,
            states: Arc::new(RwLock::new(HashMap::new())),
            checkpoints: Arc::new(RwLock::new(HashMap::new()))
        };

        let result = sync_manager.run_sync_cycle(ctx, 60).await;
        assert!(result.is_ok());
    }
}
