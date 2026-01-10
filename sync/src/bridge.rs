use crate::error::{Result, SyncError};
use crate::pointer::{KnowledgePointer, KnowledgePointerMetadata, map_layer};
use crate::state::{FederationConflict, SyncConflict, SyncFailure, SyncState, SyncTrigger};
use crate::state_persister::SyncStatePersister;
use knowledge::federation::FederationProvider;
use knowledge::governance::GovernanceEngine;
use memory::manager::MemoryManager;
use mk_core::traits::KnowledgeRepository;
use mk_core::types::{KnowledgeEntry, KnowledgeLayer, MemoryEntry};
use std::collections::HashMap;
use std::sync::Arc;
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
    federation_manager: Option<Arc<dyn FederationProvider>>,
    persister: Arc<dyn SyncStatePersister>,
    state: Arc<RwLock<SyncState>>,
    checkpoint: Arc<RwLock<Option<SyncState>>>
}

impl SyncManager {
    pub async fn new(
        memory_manager: Arc<MemoryManager>,
        knowledge_repo: Arc<
            dyn KnowledgeRepository<Error = knowledge::repository::RepositoryError>
        >,
        governance_engine: Arc<GovernanceEngine>,
        federation_manager: Option<Arc<dyn FederationProvider>>,
        persister: Arc<dyn SyncStatePersister>
    ) -> Result<Self> {
        let state = persister
            .load()
            .await
            .map_err(|e| SyncError::Persistence(e.to_string()))?;
        Ok(Self {
            memory_manager,
            knowledge_repo,
            governance_engine,
            federation_manager,
            persister,
            state: Arc::new(RwLock::new(state)),
            checkpoint: Arc::new(RwLock::new(None))
        })
    }

    pub async fn initialize(&self) -> Result<()> {
        tracing::info!("Initializing SyncManager");

        self.knowledge_repo.get_head_commit().await.map_err(|e| {
            tracing::error!(
                "Failed to access knowledge repository during initialization: {}",
                e
            );
            SyncError::Internal(format!("Repo access failed: {}", e))
        })?;

        let state = self.state.read().await;
        tracing::info!(
            "SyncManager initialized with version {}, last sync: {:?}",
            state.version,
            state.last_sync_at
        );

        Ok(())
    }

    pub async fn shutdown(&self) -> Result<()> {
        tracing::info!("Shutting down SyncManager");
        let state = self.state.read().await;
        self.persister.save(&state).await.map_err(|e| {
            tracing::error!("Failed to save state during shutdown: {}", e);
            SyncError::Persistence(e.to_string())
        })?;
        tracing::info!("SyncManager state persisted successfully");
        Ok(())
    }

    pub async fn scheduled_sync(&self, staleness_threshold_mins: u32) -> Result<()> {
        if let Some(trigger) = self.check_triggers(staleness_threshold_mins).await? {
            tracing::info!("Scheduled sync triggered by {:?}", trigger);
            self.run_sync_cycle(staleness_threshold_mins).await?;
        }
        Ok(())
    }
}

impl SyncManager {
    pub async fn run_sync_cycle(&self, staleness_threshold_mins: u32) -> Result<()> {
        if let Some(trigger) = self.check_triggers(staleness_threshold_mins).await? {
            tracing::info!("Sync triggered by {:?}", trigger);

            self.create_checkpoint().await;

            if let Some(fed_manager) = &self.federation_manager {
                let fed_start = std::time::Instant::now();
                if let Err(e) = self.sync_federation(fed_manager.as_ref()).await {
                    tracing::error!("Federation sync failed, rolling back: {}", e);
                    metrics::counter!("sync.federation.failures", 1);
                    self.rollback().await?;
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
            let mut sync_result = self.sync_incremental().await;

            while let Err(e) = sync_result {
                if retry_count >= max_retries {
                    tracing::error!(
                        "Incremental sync failed after {} retries, rolling back: {}",
                        max_retries,
                        e
                    );
                    metrics::counter!("sync.incremental.failures", 1);
                    self.rollback().await?;
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
                sync_result = self.sync_incremental().await;
            }

            metrics::histogram!(
                "sync.incremental.duration_ms",
                inc_start.elapsed().as_millis() as f64
            );

            self.prune_failed_items(30).await?;

            let conflicts = self.detect_conflicts().await?;
            if !conflicts.is_empty() {
                tracing::info!("Found {} conflicts during sync cycle", conflicts.len());
                metrics::counter!("sync.conflicts.detected", conflicts.len() as u64);
                let mut state = self.state.write().await;
                state.stats.total_conflicts += conflicts.len() as u64;
                drop(state);
                if let Err(e) = self.resolve_conflicts(conflicts).await {
                    tracing::error!("Conflict resolution failed, rolling back: {}", e);
                    metrics::counter!("sync.conflicts.resolution_failures", 1);
                    self.rollback().await?;
                    return Err(e);
                }
                metrics::counter!("sync.conflicts.resolved", 1);
            }
        }

        Ok(())
    }

    pub async fn create_checkpoint(&self) {
        let mut checkpoint = self.checkpoint.write().await;
        let state = self.state.read().await;
        *checkpoint = Some(state.clone());
        tracing::debug!("Sync checkpoint created");
    }

    pub async fn rollback(&self) -> Result<()> {
        let mut checkpoint = self.checkpoint.write().await;
        if let Some(old_state) = checkpoint.take() {
            let mut state = self.state.write().await;
            *state = old_state;
            self.persister.save(&state).await.map_err(|e| {
                metrics::counter!("sync.persistence.rollback_failures", 1);
                SyncError::Persistence(e.to_string())
            })?;
            tracing::info!("Sync state rolled back to checkpoint");
            Ok(())
        } else {
            tracing::warn!("Rollback requested but no checkpoint found");
            Ok(())
        }
    }

    pub async fn sync_federation(&self, fed_manager: &dyn FederationProvider) -> Result<()> {
        tracing::info!("Starting federation sync");
        let mut state = self.state.write().await;
        let upstreams = fed_manager.config().upstreams.clone();

        for upstream in upstreams {
            let upstream_id = upstream.id.clone();

            let target_path = self
                .knowledge_repo
                .root_path()
                .unwrap_or_else(|| std::path::PathBuf::from("data/knowledge"))
                .join("federated")
                .join(&upstream_id);

            match fed_manager.sync_upstream(&upstream_id, &target_path).await {
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
            .save(&state)
            .await
            .map_err(|e| SyncError::Persistence(e.to_string()))?;
        Ok(())
    }

    pub async fn get_state(&self) -> SyncState {
        self.state.read().await.clone()
    }

    pub async fn sync_incremental(&self) -> Result<()> {
        let mut state = self.state.write().await;
        let start_time = std::time::Instant::now();

        let last_commit = match &state.last_knowledge_commit {
            Some(c) => c.clone(),
            None => return self.sync_all_internal(&mut state, start_time).await
        };

        let head_commit = self.knowledge_repo.get_head_commit().await?;
        if let Some(head) = &head_commit
            && head == &last_commit
        {
            return Ok(());
        }

        let mut sync_errors = Vec::new();
        let affected_items = self.knowledge_repo.get_affected_items(&last_commit).await?;

        for (layer, path) in affected_items {
            let entry = match self.knowledge_repo.get(layer, &path).await {
                Ok(Some(e)) => e,
                Ok(None) => {
                    if let Some(memory_id) = self.find_memory_id_by_knowledge_id(&path, &state) {
                        self.memory_manager
                            .delete_from_layer(map_layer(layer), &memory_id)
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

            if let Err(e) = self.sync_entry(&entry, &mut state).await {
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
            .save(&state)
            .await
            .map_err(|e| SyncError::Persistence(e.to_string()))?;

        Ok(())
    }

    pub async fn sync_all(&self) -> Result<()> {
        let mut state = self.state.write().await;
        let start_time = std::time::Instant::now();
        self.sync_all_internal(&mut state, start_time).await
    }

    async fn sync_all_internal(
        &self,
        state: &mut SyncState,
        start_time: std::time::Instant
    ) -> Result<()> {
        let head_commit = self.knowledge_repo.get_head_commit().await?;
        let mut sync_errors = Vec::new();

        for layer in [
            mk_core::types::KnowledgeLayer::Company,
            mk_core::types::KnowledgeLayer::Org,
            mk_core::types::KnowledgeLayer::Team,
            mk_core::types::KnowledgeLayer::Project
        ] {
            let entries = match self.knowledge_repo.list(layer, "").await {
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
                if let Err(e) = self.sync_entry(&entry, state).await {
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
            .save(state)
            .await
            .map_err(|e| SyncError::Persistence(e.to_string()))?;

        Ok(())
    }

    pub async fn check_triggers(
        &self,
        staleness_threshold_mins: u32
    ) -> Result<Option<SyncTrigger>> {
        let state = self.state.read().await;

        let head_commit = self.knowledge_repo.get_head_commit().await?;
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
        upstream_id: &str,
        resolution: &str
    ) -> Result<()> {
        let mut state = self.state.write().await;

        state
            .federation_conflicts
            .retain(|c| c.upstream_id != upstream_id);

        tracing::info!(
            "Resolved federation conflict for {}: {}",
            upstream_id,
            resolution
        );

        self.persister
            .save(&state)
            .await
            .map_err(|e| SyncError::Persistence(e.to_string()))?;
        Ok(())
    }

    pub async fn resolve_conflicts(&self, conflicts: Vec<SyncConflict>) -> Result<()> {
        let mut state = self.state.write().await;

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
                    if let Some(entry) = self.knowledge_repo.get(layer, &knowledge_id).await? {
                        self.sync_entry(&entry, &mut state).await?;
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
                            .delete_from_layer(layer, &memory_id)
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
                            let _ = self.memory_manager.delete_from_layer(layer, &mid).await;
                        }
                        state.pointer_mapping.remove(&mid);
                    }

                    let layer = state
                        .knowledge_layers
                        .get(&knowledge_id)
                        .cloned()
                        .unwrap_or(mk_core::types::KnowledgeLayer::Company);
                    if let Some(entry) = self.knowledge_repo.get(layer, &knowledge_id).await? {
                        self.sync_entry(&entry, &mut state).await?;
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
                    if let Some(entry) = self.knowledge_repo.get(layer, &knowledge_id).await? {
                        self.sync_entry(&entry, &mut state).await?;
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
                        .delete_from_layer(old_memory_layer, &memory_id)
                        .await;

                    state.knowledge_hashes.remove(&knowledge_id);
                    state.pointer_mapping.remove(&memory_id);
                    state.knowledge_layers.remove(&knowledge_id);

                    if let Some(entry) =
                        self.knowledge_repo.get(actual_layer, &knowledge_id).await?
                    {
                        self.sync_entry(&entry, &mut state).await?;
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
            .save(&state)
            .await
            .map_err(|e| SyncError::Persistence(e.to_string()))?;
        Ok(())
    }

    pub async fn detect_conflicts(&self) -> Result<Vec<SyncConflict>> {
        let state = self.state.read().await;
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

            let entry_res = self.knowledge_repo.get(layer, knowledge_id).await;
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
                    match self.memory_manager.get_from_layer(m_layer, memory_id).await {
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

                        if let Ok(Some(_actual_entry)) =
                            self.knowledge_repo.get(other_layer, knowledge_id).await
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

    pub async fn sync_entry(&self, entry: &KnowledgeEntry, state: &mut SyncState) -> Result<()> {
        let mut content = entry.content.clone();
        content = utils::redact_pii(&content);

        let mut context = HashMap::new();
        context.insert("path".to_string(), serde_json::json!(entry.path));
        context.insert("content".to_string(), serde_json::json!(content));

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
            metadata: metadata_map,
            created_at: chrono::Utc::now().timestamp(),
            updated_at: chrono::Utc::now().timestamp()
        };

        self.memory_manager
            .add_to_layer(memory_layer, memory_entry)
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

    pub async fn prune_failed_items(&self, days_old: i64) -> Result<()> {
        let mut state = self.state.write().await;
        let now = chrono::Utc::now().timestamp();
        let threshold = days_old * 24 * 60 * 60;

        let before_count = state.failed_items.len();
        state
            .failed_items
            .retain(|f| (now - f.failed_at) < threshold);

        let pruned = before_count - state.failed_items.len();
        if pruned > 0 {
            tracing::info!(
                "Pruned {} failed items older than {} days",
                pruned,
                days_old
            );
            self.persister
                .save(&state)
                .await
                .map_err(|e| SyncError::Persistence(e.to_string()))?;
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

    pub async fn detect_delta(&self, state: &SyncState) -> Result<DeltaResult> {
        let mut delta = DeltaResult::default();
        let layers = [
            KnowledgeLayer::Company,
            KnowledgeLayer::Org,
            KnowledgeLayer::Team,
            KnowledgeLayer::Project
        ];

        for layer in layers {
            let entries = self.knowledge_repo.list(layer, "").await?;
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

    pub async fn start_background_sync(
        self: Arc<Self>,
        interval_secs: u64,
        staleness_threshold_mins: u32
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(interval_secs));
            loop {
                interval.tick().await;
                if let Err(e) = self.run_sync_cycle(staleness_threshold_mins).await {
                    metrics::counter!("sync.background.errors", 1);
                    tracing::error!("Background sync error: {}", e);
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mk_core::types::{KnowledgeEntry, KnowledgeLayer, KnowledgeStatus, KnowledgeType};
    use std::collections::HashMap;
    use std::time::Instant;

    struct MockPersister;
    #[async_trait::async_trait]
    impl SyncStatePersister for MockPersister {
        async fn load(
            &self
        ) -> std::result::Result<SyncState, Box<dyn std::error::Error + Send + Sync>> {
            Ok(SyncState::default())
        }
        async fn save(
            &self,
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
            _e: KnowledgeEntry,
            _m: &str
        ) -> std::result::Result<String, Self::Error> {
            Ok("hash".to_string())
        }
        async fn get(
            &self,
            _l: KnowledgeLayer,
            _p: &str
        ) -> std::result::Result<Option<KnowledgeEntry>, Self::Error> {
            Ok(None)
        }
        async fn list(
            &self,
            _l: KnowledgeLayer,
            _p: &str
        ) -> std::result::Result<Vec<KnowledgeEntry>, Self::Error> {
            Ok(Vec::new())
        }
        async fn delete(
            &self,
            _l: KnowledgeLayer,
            _p: &str,
            _m: &str
        ) -> std::result::Result<String, Self::Error> {
            Ok("hash".to_string())
        }
        async fn get_head_commit(&self) -> std::result::Result<Option<String>, Self::Error> {
            Ok(None)
        }
        async fn get_affected_items(
            &self,
            _f: &str
        ) -> std::result::Result<Vec<(KnowledgeLayer, String)>, Self::Error> {
            Ok(Vec::new())
        }
        async fn search(
            &self,
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

    #[test]
    fn test_generate_summary() {
        let sync_manager = SyncManager {
            memory_manager: Arc::new(MemoryManager::new()),
            knowledge_repo: Arc::new(MockKnowledgeRepository::new()),
            governance_engine: Arc::new(GovernanceEngine::new()),
            federation_manager: None,
            persister: Arc::new(MockPersister),
            state: Arc::new(RwLock::new(SyncState::default())),
            checkpoint: Arc::new(RwLock::new(None))
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
            updated_at: 1234567890
        };

        let summary = sync_manager.generate_summary(&entry);
        assert_eq!(summary, "[Spec] [Accepted] test.md\n\nFirst line");
    }

    #[test]
    fn test_generate_summary_empty_content() {
        let sync_manager = SyncManager {
            memory_manager: Arc::new(MemoryManager::new()),
            knowledge_repo: Arc::new(MockKnowledgeRepository::new()),
            governance_engine: Arc::new(GovernanceEngine::new()),
            federation_manager: None,
            persister: Arc::new(MockPersister),
            state: Arc::new(RwLock::new(SyncState::default())),
            checkpoint: Arc::new(RwLock::new(None))
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
            updated_at: 1234567890
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
            federation_manager: None,
            persister: Arc::new(MockPersister),
            state: Arc::new(RwLock::new(SyncState::default())),
            checkpoint: Arc::new(RwLock::new(None))
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
            _e: KnowledgeEntry,
            _m: &str
        ) -> std::result::Result<String, Self::Error> {
            Ok("hash".to_string())
        }
        async fn get(
            &self,
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
            _l: KnowledgeLayer,
            _p: &str,
            _m: &str
        ) -> std::result::Result<String, Self::Error> {
            Ok("hash".to_string())
        }
        async fn get_head_commit(&self) -> std::result::Result<Option<String>, Self::Error> {
            Ok(None)
        }
        async fn get_affected_items(
            &self,
            _f: &str
        ) -> std::result::Result<Vec<(KnowledgeLayer, String)>, Self::Error> {
            Ok(Vec::new())
        }
        async fn search(
            &self,
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
            updated_at: 0
        });

        let memory = Arc::new(MemoryManager::new());
        memory
            .register_provider(
                mk_core::types::MemoryLayer::Project,
                Box::new(memory::providers::MockProvider::new())
            )
            .await;
        memory
            .register_provider(
                mk_core::types::MemoryLayer::Org,
                Box::new(memory::providers::MockProvider::new())
            )
            .await;

        memory
            .add_to_layer(
                mk_core::types::MemoryLayer::Project,
                MemoryEntry {
                    id: m_id.clone(),
                    content: "[Spec] [Accepted] moved_item.md\n\ncontent".to_string(),
                    embedding: None,
                    layer: mk_core::types::MemoryLayer::Project,
                    metadata: HashMap::new(),
                    created_at: 0,
                    updated_at: 0
                }
            )
            .await
            .unwrap();

        let sync_manager = SyncManager {
            memory_manager: memory,
            knowledge_repo: Arc::new(repo),
            governance_engine: Arc::new(GovernanceEngine::new()),
            federation_manager: None,
            persister: Arc::new(MockPersister),
            state: Arc::new(RwLock::new(state)),
            checkpoint: Arc::new(RwLock::new(None))
        };

        let conflicts = sync_manager.detect_conflicts().await.unwrap();

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

        for i in 0..count {
            let k_id = format!("item_{}.md", i);
            let m_id = format!("ptr_{}", k_id);
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
                layer: KnowledgeLayer::Project,
                kind: KnowledgeType::Spec,
                status: KnowledgeStatus::Accepted,
                metadata: HashMap::new(),
                commit_hash: None,
                author: None,
                updated_at: 0
            });
        }

        let memory = Arc::new(MemoryManager::new());
        memory
            .register_provider(
                mk_core::types::MemoryLayer::Project,
                Box::new(memory::providers::MockProvider::new())
            )
            .await;

        for i in 0..count {
            let k_id = format!("item_{}.md", i);
            let m_id = format!("ptr_{}", k_id);
            memory
                .add_to_layer(
                    mk_core::types::MemoryLayer::Project,
                    MemoryEntry {
                        id: m_id,
                        content: "[Spec] [Accepted] item.md\n\ncontent".to_string(),
                        embedding: None,
                        layer: mk_core::types::MemoryLayer::Project,
                        metadata: HashMap::new(),
                        created_at: 0,
                        updated_at: 0
                    }
                )
                .await
                .unwrap();
        }

        let sync_manager = SyncManager {
            memory_manager: memory,
            knowledge_repo: Arc::new(repo),
            governance_engine: Arc::new(GovernanceEngine::new()),
            federation_manager: None,
            persister: Arc::new(MockPersister),
            state: Arc::new(RwLock::new(state)),
            checkpoint: Arc::new(RwLock::new(None))
        };

        let start = Instant::now();
        let _ = sync_manager.detect_conflicts().await.unwrap();
        let duration = start.elapsed();

        println!(
            "Conflict detection for {} items took: {:?}",
            count, duration
        );
        assert!(duration.as_secs() < 5);
    }
}
