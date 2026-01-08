use crate::pointer::{KnowledgePointer, KnowledgePointerMetadata, map_layer};
use crate::state::{SyncConflict, SyncFailure, SyncState, SyncTrigger};
use crate::state_persister::SyncStatePersister;
use knowledge::repository::GitRepository;
use memory::manager::MemoryManager;
use mk_core::traits::KnowledgeRepository;
use mk_core::types::{KnowledgeEntry, MemoryEntry};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct SyncManager {
    memory_manager: Arc<MemoryManager>,
    knowledge_repo: Arc<GitRepository>,
    persister: Arc<dyn SyncStatePersister>,
    state: Arc<RwLock<SyncState>>,
}

impl SyncManager {
    pub async fn new(
        memory_manager: Arc<MemoryManager>,
        knowledge_repo: Arc<GitRepository>,
        persister: Arc<dyn SyncStatePersister>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let state = persister.load().await?;
        Ok(Self {
            memory_manager,
            knowledge_repo,
            persister,
            state: Arc::new(RwLock::new(state)),
        })
    }

    pub async fn run_sync_cycle(
        &self,
        staleness_threshold_mins: u32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(trigger) = self.check_triggers(staleness_threshold_mins).await? {
            tracing::info!("Sync triggered by {:?}", trigger);

            self.sync_incremental().await?;

            let conflicts = self.detect_conflicts().await?;
            if !conflicts.is_empty() {
                tracing::info!("Found {} conflicts during sync cycle", conflicts.len());
                let mut state = self.state.write().await;
                state.stats.total_conflicts += conflicts.len() as u64;
                drop(state);
                self.resolve_conflicts(conflicts).await?;
            }
        }

        Ok(())
    }

    pub async fn get_state(&self) -> SyncState {
        self.state.read().await.clone()
    }

    pub async fn sync_incremental(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut state = self.state.write().await;
        let start_time = std::time::Instant::now();

        let last_commit = match &state.last_knowledge_commit {
            Some(c) => c.clone(),
            None => return self.sync_all_internal(&mut state, start_time).await,
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
                    }
                    continue;
                }
                Err(e) => {
                    sync_errors.push(SyncFailure {
                        knowledge_id: path,
                        error: e.to_string(),
                        failed_at: chrono::Utc::now().timestamp(),
                        retry_count: 0,
                    });
                    continue;
                }
            };

            if let Err(e) = self.sync_entry(&entry, &mut state).await {
                sync_errors.push(SyncFailure {
                    knowledge_id: entry.path.clone(),
                    error: e.to_string(),
                    failed_at: chrono::Utc::now().timestamp(),
                    retry_count: 0,
                });
            }
        }

        state.last_sync_at = Some(chrono::Utc::now().timestamp());
        state.last_knowledge_commit = head_commit;
        state.failed_items.extend(sync_errors);
        state.stats.total_syncs += 1;
        state.stats.avg_sync_duration_ms = start_time.elapsed().as_millis() as u64;

        self.persister.save(&state).await?;

        Ok(())
    }

    pub async fn sync_all(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut state = self.state.write().await;
        let start_time = std::time::Instant::now();
        self.sync_all_internal(&mut state, start_time).await
    }

    async fn sync_all_internal(
        &self,
        state: &mut SyncState,
        start_time: std::time::Instant,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let head_commit = self.knowledge_repo.get_head_commit().await?;
        let mut sync_errors = Vec::new();

        for layer in [
            mk_core::types::KnowledgeLayer::Company,
            mk_core::types::KnowledgeLayer::Org,
            mk_core::types::KnowledgeLayer::Team,
            mk_core::types::KnowledgeLayer::Project,
        ] {
            let entries = match self.knowledge_repo.list(layer, "").await {
                Ok(e) => e,
                Err(e) => {
                    sync_errors.push(SyncFailure {
                        knowledge_id: format!("layer:{layer:?}"),
                        error: e.to_string(),
                        failed_at: chrono::Utc::now().timestamp(),
                        retry_count: 0,
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
                        retry_count: 0,
                    });
                }
            }
        }

        state.last_sync_at = Some(chrono::Utc::now().timestamp());
        state.last_knowledge_commit = head_commit;
        state.failed_items = sync_errors;
        state.stats.total_syncs += 1;
        state.stats.avg_sync_duration_ms = start_time.elapsed().as_millis() as u64;

        self.persister.save(state).await?;

        Ok(())
    }

    pub async fn check_triggers(
        &self,
        staleness_threshold_mins: u32,
    ) -> Result<Option<SyncTrigger>, Box<dyn std::error::Error + Send + Sync>> {
        let state = self.state.read().await;

        let head_commit = self.knowledge_repo.get_head_commit().await?;
        if let Some(head) = head_commit {
            if let Some(last) = &state.last_knowledge_commit {
                if head != *last {
                    return Ok(Some(SyncTrigger::CommitMismatch {
                        last_commit: last.clone(),
                        head_commit: head,
                    }));
                }
            } else {
                return Ok(Some(SyncTrigger::CommitMismatch {
                    last_commit: "none".to_string(),
                    head_commit: head,
                }));
            }
        }

        if let Some(last_sync) = state.last_sync_at {
            let now = chrono::Utc::now().timestamp();
            let elapsed_mins = (now - last_sync) / 60;
            if elapsed_mins >= staleness_threshold_mins as i64 {
                return Ok(Some(SyncTrigger::Staleness {
                    last_sync_at: last_sync,
                    threshold_mins: staleness_threshold_mins,
                }));
            }
        } else {
            return Ok(Some(SyncTrigger::Manual));
        }

        Ok(None)
    }

    pub async fn resolve_conflicts(
        &self,
        conflicts: Vec<SyncConflict>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut state = self.state.write().await;

        for conflict in conflicts {
            match conflict {
                SyncConflict::HashMismatch { knowledge_id, .. }
                | SyncConflict::MissingPointer { knowledge_id, .. } => {
                    state.knowledge_hashes.remove(&knowledge_id);
                    if let Some(entry) = self.knowledge_repo.get_by_path(&knowledge_id).await? {
                        self.sync_entry(&entry, &mut state).await?;
                    }
                }
                SyncConflict::OrphanedPointer {
                    memory_id,
                    knowledge_id,
                } => {
                    for layer in [
                        mk_core::types::MemoryLayer::Company,
                        mk_core::types::MemoryLayer::Org,
                        mk_core::types::MemoryLayer::Team,
                        mk_core::types::MemoryLayer::Project,
                    ] {
                        let _ = self
                            .memory_manager
                            .delete_from_layer(layer, &memory_id)
                            .await;
                    }
                    state.knowledge_hashes.remove(&knowledge_id);
                    state.pointer_mapping.remove(&memory_id);
                }
            }
        }

        self.persister.save(&state).await?;
        Ok(())
    }

    pub async fn detect_conflicts(
        &self,
    ) -> Result<Vec<SyncConflict>, Box<dyn std::error::Error + Send + Sync>> {
        let state = self.state.read().await;
        let mut conflicts = Vec::new();

        for (memory_id, knowledge_id) in &state.pointer_mapping {
            let entry_res = self.knowledge_repo.get_by_path(knowledge_id).await;

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
                            actual_hash,
                        });
                    }

                    let m_layer = map_layer(k_entry.layer);
                    match self.memory_manager.get_from_layer(m_layer, memory_id).await {
                        Ok(None) => {
                            conflicts.push(SyncConflict::MissingPointer {
                                knowledge_id: knowledge_id.clone(),
                                expected_memory_id: memory_id.clone(),
                            });
                        }
                        Ok(Some(m_entry)) => {
                            let expected_content = self.generate_summary(&k_entry);
                            if m_entry.content != expected_content {
                                conflicts.push(SyncConflict::HashMismatch {
                                    knowledge_id: knowledge_id.clone(),
                                    memory_id: memory_id.clone(),
                                    expected_hash: "summary_mismatch".to_string(),
                                    actual_hash: "summary_mismatch".to_string(),
                                });
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Failed to check memory entry {}: {}", memory_id, e)
                        }
                    }
                }
                Ok(None) => {
                    conflicts.push(SyncConflict::OrphanedPointer {
                        memory_id: memory_id.clone(),
                        knowledge_id: knowledge_id.clone(),
                    });
                }
                Err(e) => tracing::error!(
                    "Error fetching knowledge {} for conflict detection: {}",
                    knowledge_id,
                    e
                ),
            }
        }

        Ok(conflicts)
    }

    fn find_memory_id_by_knowledge_id(
        &self,
        knowledge_id: &str,
        state: &SyncState,
    ) -> Option<String> {
        state
            .pointer_mapping
            .iter()
            .find(|(_, kid)| *kid == knowledge_id)
            .map(|(mid, _)| mid.clone())
    }

    async fn sync_entry(
        &self,
        entry: &KnowledgeEntry,
        state: &mut SyncState,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let content_hash = utils::compute_content_hash(&entry.content);
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
            is_orphaned: false,
        };

        let metadata = KnowledgePointerMetadata {
            kind: "knowledge_pointer".to_string(),
            knowledge_pointer: pointer,
            tags: Vec::new(),
        };

        let metadata_map = match serde_json::to_value(metadata)? {
            serde_json::Value::Object(map) => {
                let mut hmap = HashMap::new();
                for (k, v) in map {
                    hmap.insert(k, v);
                }
                hmap
            }
            _ => return Err("Failed to serialize metadata".into()),
        };

        let memory_entry = MemoryEntry {
            id: format!("ptr_{knowledge_id}"),
            content: self.generate_summary(entry),
            embedding: None,
            layer: memory_layer,
            metadata: metadata_map,
            created_at: chrono::Utc::now().timestamp(),
            updated_at: chrono::Utc::now().timestamp(),
        };

        self.memory_manager
            .add_to_layer(memory_layer, memory_entry)
            .await?;

        state
            .knowledge_hashes
            .insert(knowledge_id.clone(), content_hash);
        state
            .pointer_mapping
            .insert(format!("ptr_{knowledge_id}"), knowledge_id.clone());
        state.stats.total_items_synced += 1;

        Ok(())
    }

    fn generate_summary(&self, entry: &KnowledgeEntry) -> String {
        format!(
            "[{:?}] {}\n\n{}",
            entry.kind,
            entry.path,
            entry.content.lines().next().unwrap_or("")
        )
    }

    pub async fn start_background_sync(
        self: Arc<Self>,
        interval_secs: u64,
        staleness_threshold_mins: u32,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(interval_secs));
            loop {
                interval.tick().await;
                if let Err(e) = self.run_sync_cycle(staleness_threshold_mins).await {
                    tracing::error!("Background sync error: {}", e);
                }
            }
        })
    }
}
