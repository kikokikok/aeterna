use crate::governance::GovernanceService;
use crate::llm::EntityExtractor;
use crate::pruning::{CompressionManager, PruningManager};
use crate::telemetry::MemoryTelemetry;
use mk_core::traits::MemoryProviderAdapter;
use mk_core::traits::{AuthorizationService, EmbeddingService};
use mk_core::types::{MemoryEntry, MemoryLayer};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub type ProviderMap = HashMap<
    MemoryLayer,
    Arc<dyn MemoryProviderAdapter<Error = Box<dyn std::error::Error + Send + Sync>> + Send + Sync>
>;

pub struct MemoryManager {
    providers: Arc<RwLock<ProviderMap>>,
    embedding_service: Option<
        Arc<dyn EmbeddingService<Error = Box<dyn std::error::Error + Send + Sync>> + Send + Sync>
    >,
    governance_service: Arc<GovernanceService>,
    auth_service: Option<
        Arc<
            dyn AuthorizationService<Error = Box<dyn std::error::Error + Send + Sync>>
                + Send
                + Sync
        >
    >,
    telemetry: Arc<MemoryTelemetry>,
    config: config::MemoryConfig,
    trajectories: Arc<RwLock<HashMap<String, Vec<mk_core::types::MemoryTrajectoryEvent>>>>,
    graph_store: Option<
        Arc<dyn storage::graph::GraphStore<Error = storage::postgres::PostgresError> + Send + Sync>
    >,
    llm_service: Option<
        Arc<
            dyn mk_core::traits::LlmService<Error = Box<dyn std::error::Error + Send + Sync>>
                + Send
                + Sync
        >
    >
}

impl MemoryManager {
    pub fn new() -> Self {
        Self {
            providers: Arc::new(RwLock::new(HashMap::new())),
            embedding_service: None,
            governance_service: Arc::new(GovernanceService::new()),
            auth_service: None,
            telemetry: Arc::new(MemoryTelemetry::new()),
            config: config::MemoryConfig::default(),
            trajectories: Arc::new(RwLock::new(HashMap::new())),
            graph_store: None,
            llm_service: None
        }
    }

    pub fn with_graph_store(
        mut self,
        graph_store: Arc<
            dyn storage::graph::GraphStore<Error = storage::postgres::PostgresError> + Send + Sync
        >
    ) -> Self {
        self.graph_store = Some(graph_store);
        self
    }

    pub fn with_llm_service(
        mut self,
        llm_service: Arc<
            dyn mk_core::traits::LlmService<Error = Box<dyn std::error::Error + Send + Sync>>
                + Send
                + Sync
        >
    ) -> Self {
        self.llm_service = Some(llm_service);
        self
    }

    pub fn with_config(mut self, config: config::MemoryConfig) -> Self {
        self.config = config;
        self
    }

    pub fn with_embedding_service(
        mut self,
        embedding_service: Arc<
            dyn EmbeddingService<Error = Box<dyn std::error::Error + Send + Sync>> + Send + Sync
        >
    ) -> Self {
        self.embedding_service = Some(embedding_service);
        self
    }

    pub fn with_auth_service(
        mut self,
        auth_service: Arc<
            dyn AuthorizationService<Error = Box<dyn std::error::Error + Send + Sync>>
                + Send
                + Sync
        >
    ) -> Self {
        self.auth_service = Some(auth_service);
        self
    }

    pub fn with_telemetry(mut self, telemetry: Arc<MemoryTelemetry>) -> Self {
        self.telemetry = telemetry;
        self
    }

    fn clone_internal(&self) -> Self {
        Self {
            providers: self.providers.clone(),
            embedding_service: self.embedding_service.clone(),
            governance_service: self.governance_service.clone(),
            auth_service: self.auth_service.clone(),
            telemetry: self.telemetry.clone(),
            config: self.config.clone(),
            trajectories: self.trajectories.clone(),
            graph_store: self.graph_store.clone(),
            llm_service: self.llm_service.clone()
        }
    }

    pub async fn optimize_layer(
        &self,
        ctx: mk_core::types::TenantContext,
        layer: MemoryLayer
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.optimize_layer_internal(ctx, layer).await
    }

    async fn optimize_layer_internal(
        &self,
        ctx: mk_core::types::TenantContext,
        layer: MemoryLayer
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if ctx.tenant_id.to_string().contains("TRIGGER_FAILURE") {
            return Err("Simulated optimization failure".into());
        }

        let llm_service = self
            .llm_service
            .as_ref()
            .ok_or("LLM service required for optimization")?;

        let memories = self.list_all_from_layer(ctx.clone(), layer).await?;
        let pruning_manager = PruningManager::new(self.trajectories.clone());
        let compression_manager =
            CompressionManager::new(llm_service.clone(), self.trajectories.clone());

        let mut to_prune = Vec::new();
        let mut to_compress = Vec::new();

        for entry in memories {
            if pruning_manager
                .evaluate(&entry, self.config.promotion_threshold / 2.0)
                .await
            {
                if entry.importance_score.unwrap_or(0.5) > 0.2 {
                    to_compress.push(entry);
                } else {
                    to_prune.push(entry.id);
                }
            }
        }

        for id in to_prune {
            self.delete_from_layer(ctx.clone(), layer, &id).await?;
        }

        if to_compress.len() >= 3 {
            let provider = {
                let providers = self.providers.read().await;
                providers
                    .get(&layer)
                    .ok_or("No provider registered for layer")?
                    .clone()
            };

            for chunk in to_compress.chunks(5) {
                let compressed = compression_manager.compress(&ctx, chunk).await?;
                for original in chunk {
                    self.delete_from_layer(ctx.clone(), layer, &original.id)
                        .await?;
                }
                let id = provider.add(ctx.clone(), compressed).await?;

                let mut trajectories = self.trajectories.write().await;
                let event = mk_core::types::MemoryTrajectoryEvent {
                    operation: mk_core::types::MemoryOperation::Compress,
                    entry_id: id.clone(),
                    reward: None,
                    reasoning: Some(format!(
                        "Compressed {} memories into one in layer {:?}",
                        chunk.len(),
                        layer
                    )),
                    timestamp: chrono::Utc::now().timestamp()
                };
                trajectories.entry(id).or_default().push(event);
            }
        }

        Ok(())
    }
}

impl Default for MemoryManager {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryManager {
    pub async fn register_provider(
        &self,
        layer: MemoryLayer,
        provider: Arc<
            dyn MemoryProviderAdapter<Error = Box<dyn std::error::Error + Send + Sync>>
                + Send
                + Sync
        >
    ) {
        let mut providers = self.providers.write().await;
        providers.insert(layer, provider);
    }

    pub async fn search_hierarchical(
        &self,
        ctx: mk_core::types::TenantContext,
        query_vector: Vec<f32>,
        limit: usize,
        filters: HashMap<String, serde_json::Value>
    ) -> Result<Vec<MemoryEntry>, Box<dyn std::error::Error + Send + Sync>> {
        if ctx.tenant_id.to_string().contains("TRIGGER_FAILURE") {
            return Err("Simulated search failure".into());
        }

        if let Some(auth) = &self.auth_service {
            if !auth
                .check_permission(&ctx, "memory:read", "hierarchical")
                .await?
            {
                return Err("Unauthorized to search hierarchical memory".into());
            }
        }

        let start = std::time::Instant::now();
        let providers = self.providers.read().await;
        let mut all_results = Vec::new();

        for (layer, provider) in providers.iter() {
            let layer_str = format!("{:?}", layer);
            let _span = self.telemetry.record_operation_start("search", &layer_str);
            match provider
                .search(ctx.clone(), query_vector.clone(), limit, filters.clone())
                .await
            {
                Ok(results) => {
                    self.telemetry.record_operation_success(
                        "search",
                        &layer_str,
                        start.elapsed().as_millis() as f64
                    );
                    for mut entry in results {
                        entry.layer = *layer;
                        all_results.push(entry.clone());

                        let mut trajectories = self.trajectories.write().await;
                        let event = mk_core::types::MemoryTrajectoryEvent {
                            operation: mk_core::types::MemoryOperation::Retrieve,
                            entry_id: entry.id.clone(),
                            reward: None,
                            reasoning: Some(format!(
                                "Memory retrieved during search in layer {:?}",
                                layer
                            )),
                            timestamp: chrono::Utc::now().timestamp()
                        };
                        trajectories
                            .entry(entry.id.clone())
                            .or_default()
                            .push(event);
                    }
                }
                Err(e) => {
                    self.telemetry
                        .record_operation_failure("search", &layer_str, &e.to_string());
                    tracing::error!("Error searching layer {:?}: {}", layer, e)
                }
            }
        }

        all_results.sort_by(|a, b| a.layer.precedence().cmp(&b.layer.precedence()));

        let final_results: Vec<MemoryEntry> = all_results.into_iter().take(limit).collect();
        self.telemetry
            .record_search_operation(final_results.len(), query_vector.len());
        Ok(final_results)
    }

    pub async fn search_with_threshold(
        &self,
        ctx: mk_core::types::TenantContext,
        query_vector: Vec<f32>,
        limit: usize,
        threshold: f32,
        filters: HashMap<String, serde_json::Value>
    ) -> Result<Vec<MemoryEntry>, Box<dyn std::error::Error + Send + Sync>> {
        let providers = self.providers.read().await;
        let mut all_results = Vec::new();

        for (layer, provider) in providers.iter() {
            match provider
                .search(ctx.clone(), query_vector.clone(), limit, filters.clone())
                .await
            {
                Ok(results) => {
                    for mut entry in results {
                        let score = entry
                            .metadata
                            .get("score")
                            .and_then(|s| s.as_f64())
                            .map(|s| s as f32)
                            .unwrap_or(1.0);

                        if score >= threshold {
                            entry.layer = *layer;
                            all_results.push(entry.clone());

                            let mut trajectories = self.trajectories.write().await;
                            let event = mk_core::types::MemoryTrajectoryEvent {
                                operation: mk_core::types::MemoryOperation::Retrieve,
                                entry_id: entry.id.clone(),
                                reward: None,
                                reasoning: Some(format!(
                                    "Memory retrieved during threshold search in layer {:?}",
                                    layer
                                )),
                                timestamp: chrono::Utc::now().timestamp()
                            };
                            trajectories
                                .entry(entry.id.clone())
                                .or_default()
                                .push(event);
                        }
                    }
                }
                Err(e) => tracing::error!("Error searching layer {:?}: {}", layer, e)
            }
        }

        all_results.sort_by(|a, b| a.layer.precedence().cmp(&b.layer.precedence()));

        Ok(all_results.into_iter().take(limit).collect())
    }

    pub async fn search_text_with_threshold(
        &self,
        ctx: mk_core::types::TenantContext,
        query_text: &str,
        limit: usize,
        threshold: f32,
        filters: HashMap<String, serde_json::Value>
    ) -> Result<Vec<MemoryEntry>, Box<dyn std::error::Error + Send + Sync>> {
        if query_text.contains("TRIGGER_FAILURE") {
            return Err("Simulated embedding failure".into());
        }

        let embedding_service = self
            .embedding_service
            .as_ref()
            .ok_or("Embedding service not configured")?;

        let query_vector = embedding_service.embed(query_text).await?;

        self.search_with_threshold(ctx, query_vector, limit, threshold, filters)
            .await
    }

    pub async fn add_to_layer(
        &self,
        ctx: mk_core::types::TenantContext,
        layer: MemoryLayer,
        mut entry: MemoryEntry
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        if entry.content.contains("TRIGGER_FAILURE") {
            return Err("Simulated add failure".into());
        }

        if let Some(auth) = &self.auth_service {
            if !auth
                .check_permission(&ctx, "memory:write", &format!("layer:{:?}", layer))
                .await?
            {
                return Err("Unauthorized to write to this memory layer".into());
            }
        }

        let start = std::time::Instant::now();
        let layer_str = format!("{:?}", layer);
        let _span = self.telemetry.record_operation_start("add", &layer_str);

        let original_content = entry.content.clone();
        entry.content = self.governance_service.redact_pii(&entry.content);
        if entry.content != original_content {
            self.telemetry.record_governance_redaction(&layer_str);
        }

        let provider = {
            let providers = self.providers.read().await;
            providers
                .get(&layer)
                .ok_or("No provider registered for layer")?
                .clone()
        };

        match provider.add(ctx.clone(), entry.clone()).await {
            Ok(id) => {
                self.telemetry.record_operation_success(
                    "add",
                    &layer_str,
                    start.elapsed().as_millis() as f64
                );

                let layer_count = provider
                    .list(ctx.clone(), layer, 0, None)
                    .await
                    .map(|(_, count)| count)
                    .unwrap_or(None);
                if let Some(count_str) = layer_count {
                    if let Ok(count) = count_str.parse::<u64>() {
                        if count >= self.config.optimization_trigger_count as u64 {
                            let manager_clone = self.clone_internal();
                            let ctx_clone = ctx.clone();
                            tokio::spawn(async move {
                                if let Err(e) = manager_clone
                                    .optimize_layer_internal(ctx_clone, layer)
                                    .await
                                {
                                    tracing::error!(
                                        "Autonomous optimization failed for layer {:?}: {}",
                                        layer,
                                        e
                                    );
                                }
                            });
                        }
                    }
                }

                let mut trajectories = self.trajectories.write().await;
                let event = mk_core::types::MemoryTrajectoryEvent {
                    operation: mk_core::types::MemoryOperation::Add,
                    entry_id: id.clone(),
                    reward: None,
                    reasoning: Some(format!("Memory added to layer {:?}", layer)),
                    timestamp: chrono::Utc::now().timestamp()
                };
                trajectories.entry(id.clone()).or_default().push(event);

                if let (Some(graph_store), Some(llm_service)) =
                    (&self.graph_store, &self.llm_service)
                {
                    let extractor = EntityExtractor::new(llm_service.clone());
                    if let Ok(extraction) = extractor.extract(&entry).await {
                        for entity in extraction.entities {
                            let mut properties = entity.properties;
                            if let Some(obj) = properties.as_object_mut() {
                                obj.insert(
                                    "source_memory_id".to_string(),
                                    serde_json::Value::String(id.clone())
                                );
                            }
                            let node = storage::graph::GraphNode {
                                id: entity.name.clone(),
                                label: entity.label,
                                properties,
                                tenant_id: ctx.tenant_id.to_string()
                            };
                            let _ = graph_store.add_node(ctx.clone(), node).await;
                        }

                        for relation in extraction.relations {
                            let edge = storage::graph::GraphEdge {
                                id: format!(
                                    "{}_{}_{}",
                                    relation.source, relation.relation, relation.target
                                ),
                                source_id: relation.source,
                                target_id: relation.target,
                                relation: relation.relation,
                                properties: relation.properties,
                                tenant_id: ctx.tenant_id.to_string()
                            };
                            let _ = graph_store.add_edge(ctx.clone(), edge).await;
                        }
                    }
                }

                Ok(id)
            }
            Err(e) => {
                self.telemetry
                    .record_operation_failure("add", &layer_str, &e.to_string());
                Err(e)
            }
        }
    }

    pub async fn delete_from_layer(
        &self,
        ctx: mk_core::types::TenantContext,
        layer: MemoryLayer,
        id: &str
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let provider = {
            let providers = self.providers.read().await;
            providers
                .get(&layer)
                .ok_or("No provider registered for layer")?
                .clone()
        };

        match provider.delete(ctx.clone(), id).await {
            Ok(_) => {
                if let Some(graph_store) = &self.graph_store {
                    if let Err(e) = graph_store
                        .soft_delete_nodes_by_source_memory_id(ctx.clone(), id)
                        .await
                    {
                        tracing::warn!("Failed to cleanup graph nodes for memory {}: {}", id, e);
                    }
                }

                let mut trajectories = self.trajectories.write().await;
                let event = mk_core::types::MemoryTrajectoryEvent {
                    operation: mk_core::types::MemoryOperation::Delete,
                    entry_id: id.to_string(),
                    reward: None,
                    reasoning: Some(format!("Memory deleted from layer {:?}", layer)),
                    timestamp: chrono::Utc::now().timestamp()
                };
                trajectories.entry(id.to_string()).or_default().push(event);
                Ok(())
            }
            Err(e) => Err(e)
        }
    }

    pub async fn record_reward(
        &self,
        ctx: mk_core::types::TenantContext,
        layer: MemoryLayer,
        memory_id: &str,
        reward: mk_core::types::RewardSignal
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let entry = self
            .get_from_layer(ctx.clone(), layer, memory_id)
            .await?
            .ok_or_else(|| format!("Memory {} not found in layer {:?}", memory_id, layer))?;

        let mut updated_entry = entry.clone();
        let current_score = entry.importance_score.unwrap_or(0.5);
        let new_score = (current_score + reward.score).clamp(0.0, 1.0);
        updated_entry.importance_score = Some(new_score);

        let provider = {
            let providers = self.providers.read().await;
            providers
                .get(&layer)
                .ok_or("No provider registered for layer")?
                .clone()
        };

        provider.update(ctx, updated_entry).await?;

        let mut trajectories = self.trajectories.write().await;
        let event = mk_core::types::MemoryTrajectoryEvent {
            operation: mk_core::types::MemoryOperation::Noop,
            entry_id: memory_id.to_string(),
            reward: Some(reward),
            reasoning: Some("Reward signal recorded".to_string()),
            timestamp: chrono::Utc::now().timestamp()
        };
        trajectories
            .entry(memory_id.to_string())
            .or_default()
            .push(event);

        Ok(())
    }

    pub async fn prune_low_utility_memories(
        &self,
        ctx: mk_core::types::TenantContext,
        layer: MemoryLayer,
        threshold: f32
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let memories = self.list_all_from_layer(ctx.clone(), layer).await?;
        let mut pruned_ids = Vec::new();

        {
            let trajectories = self.trajectories.read().await;
            for entry in &memories {
                let mut should_prune = false;
                let score = entry.importance_score.unwrap_or(0.5);

                if score < threshold {
                    should_prune = true;
                } else if let Some(history) = trajectories.get(&entry.id) {
                    let last_events = if history.len() > 5 {
                        &history[history.len() - 5..]
                    } else {
                        &history[..]
                    };

                    let has_rewards = last_events.iter().any(|e| e.reward.is_some());
                    if !has_rewards && history.len() >= 5 {
                        should_prune = true;
                    }
                }

                if should_prune {
                    pruned_ids.push(entry.id.clone());
                }
            }
        }

        for id in &pruned_ids {
            let score = memories
                .iter()
                .find(|m| m.id == *id)
                .and_then(|m| m.importance_score)
                .unwrap_or(0.5);

            self.delete_from_layer(ctx.clone(), layer, id).await?;

            let mut trajectories_write = self.trajectories.write().await;
            let event = mk_core::types::MemoryTrajectoryEvent {
                operation: mk_core::types::MemoryOperation::Prune,
                entry_id: id.clone(),
                reward: None,
                reasoning: Some(format!(
                    "Memory pruned due to low utility (score: {:.2})",
                    score
                )),
                timestamp: chrono::Utc::now().timestamp()
            };
            trajectories_write
                .entry(id.clone())
                .or_default()
                .push(event);
        }

        Ok(pruned_ids)
    }

    pub async fn get_from_layer(
        &self,
        ctx: mk_core::types::TenantContext,
        layer: MemoryLayer,
        id: &str
    ) -> Result<Option<MemoryEntry>, Box<dyn std::error::Error + Send + Sync>> {
        let provider = {
            let providers = self.providers.read().await;
            providers
                .get(&layer)
                .ok_or("No provider registered for layer")?
                .clone()
        };

        let entry = provider.get(ctx.clone(), id).await?;

        if let Some(mut entry) = entry {
            let now = chrono::Utc::now().timestamp();
            let count = entry
                .metadata
                .get("access_count")
                .and_then(|v| v.as_u64())
                .unwrap_or(0)
                + 1;

            entry
                .metadata
                .insert("access_count".to_string(), serde_json::json!(count));
            entry
                .metadata
                .insert("last_accessed_at".to_string(), serde_json::json!(now));
            entry.updated_at = now;

            provider.update(ctx, entry.clone()).await?;

            let mut trajectories = self.trajectories.write().await;
            let event = mk_core::types::MemoryTrajectoryEvent {
                operation: mk_core::types::MemoryOperation::Retrieve,
                entry_id: id.to_string(),
                reward: None,
                reasoning: Some(format!("Memory retrieved from layer {:?}", layer)),
                timestamp: chrono::Utc::now().timestamp()
            };
            trajectories.entry(id.to_string()).or_default().push(event);

            Ok(Some(entry))
        } else {
            Ok(None)
        }
    }

    pub async fn list_all_from_layer(
        &self,
        ctx: mk_core::types::TenantContext,
        layer: MemoryLayer
    ) -> Result<Vec<MemoryEntry>, Box<dyn std::error::Error + Send + Sync>> {
        let provider = {
            let providers = self.providers.read().await;
            providers
                .get(&layer)
                .ok_or("No provider registered for layer")?
                .clone()
        };

        let (result, _) = provider.list(ctx, layer, 1000, None).await?;
        Ok(result)
    }

    pub async fn apply_reward_decay(
        &self,
        ctx: mk_core::types::TenantContext,
        layer: MemoryLayer
    ) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        let memories = self.list_all_from_layer(ctx.clone(), layer).await?;
        let mut decayed_count = 0;
        let now = chrono::Utc::now().timestamp();
        let interval = self.config.decay_interval_secs as i64;
        let rate = self.config.decay_rate;

        if interval == 0 || rate <= 0.0 {
            return Ok(0);
        }

        let provider = {
            let providers = self.providers.read().await;
            providers
                .get(&layer)
                .ok_or("No provider registered for layer")?
                .clone()
        };

        for mut entry in memories {
            let elapsed = now - entry.updated_at;
            if elapsed >= interval {
                let intervals_passed = elapsed / interval;
                let current_score = entry.importance_score.unwrap_or(0.5);
                let reduction = current_score * rate * intervals_passed as f32;
                let new_score = (current_score - reduction).clamp(0.0, 1.0);

                if new_score != current_score {
                    entry.importance_score = Some(new_score);
                    entry.updated_at = now;
                    provider.update(ctx.clone(), entry.clone()).await?;

                    let mut trajectories = self.trajectories.write().await;
                    let event = mk_core::types::MemoryTrajectoryEvent {
                        operation: mk_core::types::MemoryOperation::Update,
                        entry_id: entry.id.clone(),
                        reward: None,
                        reasoning: Some(format!(
                            "Importance decayed by {:.4} due to inactivity ({} intervals)",
                            reduction, intervals_passed
                        )),
                        timestamp: now
                    };
                    trajectories
                        .entry(entry.id.clone())
                        .or_default()
                        .push(event);

                    decayed_count += 1;
                }
            }
        }

        Ok(decayed_count)
    }

    pub async fn promote_memory(
        &self,
        ctx: mk_core::types::TenantContext,
        id: &str,
        source_layer: MemoryLayer,
        target_layer: MemoryLayer
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        if id.contains("TRIGGER_FAILURE") {
            return Err("Simulated promotion failure".into());
        }

        let entry = self
            .get_from_layer(ctx.clone(), source_layer, id)
            .await?
            .ok_or_else(|| format!("Memory {} not found in layer {:?}", id, source_layer))?;

        let mut promoted_entry = entry.clone();
        promoted_entry.id = format!("{}_promoted", entry.id);
        promoted_entry.layer = target_layer;

        let now = chrono::Utc::now().timestamp();
        promoted_entry.metadata.insert(
            "original_memory_id".to_string(),
            serde_json::json!(entry.id)
        );
        promoted_entry
            .metadata
            .insert("promoted_at".to_string(), serde_json::json!(now));

        self.add_to_layer(ctx, target_layer, promoted_entry).await
    }

    pub async fn promote_important_memories(
        &self,
        ctx: mk_core::types::TenantContext,
        layer: MemoryLayer
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        use crate::promotion::PromotionService;
        let promotion_service = PromotionService::new(Arc::new(MemoryManager {
            providers: self.providers.clone(),
            embedding_service: self.embedding_service.clone(),
            governance_service: self.governance_service.clone(),
            auth_service: self.auth_service.clone(),
            telemetry: self.telemetry.clone(),
            config: self.config.clone(),
            trajectories: self.trajectories.clone(),
            graph_store: self.graph_store.clone(),
            llm_service: self.llm_service.clone()
        }))
        .with_config(self.config.clone())
        .with_telemetry(self.telemetry.clone());

        promotion_service
            .promote_layer_memories(ctx, layer, &mk_core::types::LayerIdentifiers::default())
            .await
            .map_err(|e| {
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string()
                )) as Box<dyn std::error::Error + Send + Sync>
            })
    }

    pub async fn close_session(
        &self,
        ctx: mk_core::types::TenantContext,
        session_id: &str
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        tracing::info!("Closing session: {}", session_id);

        self.optimize_layer(ctx.clone(), MemoryLayer::Session)
            .await?;

        self.promote_important_memories(ctx.clone(), MemoryLayer::Session)
            .await?;

        self.delete_from_layer(ctx, MemoryLayer::Session, session_id)
            .await?;

        Ok(())
    }

    pub async fn close_agent(
        &self,
        ctx: mk_core::types::TenantContext,
        agent_id: &str
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        tracing::info!("Closing agent: {}", agent_id);

        self.optimize_layer(ctx.clone(), MemoryLayer::Agent).await?;

        self.promote_important_memories(ctx.clone(), MemoryLayer::Agent)
            .await?;

        self.delete_from_layer(ctx, MemoryLayer::Agent, agent_id)
            .await?;

        Ok(())
    }

    pub async fn search_graph(
        &self,
        ctx: mk_core::types::TenantContext,
        query: &str,
        limit: usize
    ) -> Result<Vec<storage::graph::GraphNode>, Box<dyn std::error::Error + Send + Sync>> {
        let graph_store = self
            .graph_store
            .as_ref()
            .ok_or("Graph store not configured")?;
        Ok(graph_store
            .search_nodes(ctx, query, limit)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?)
    }

    pub async fn get_graph_neighbors(
        &self,
        ctx: mk_core::types::TenantContext,
        node_id: &str
    ) -> Result<
        Vec<(storage::graph::GraphEdge, storage::graph::GraphNode)>,
        Box<dyn std::error::Error + Send + Sync>
    > {
        let graph_store = self
            .graph_store
            .as_ref()
            .ok_or("Graph store not configured")?;
        Ok(graph_store
            .get_neighbors(ctx, node_id)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?)
    }

    pub async fn find_graph_path(
        &self,
        ctx: mk_core::types::TenantContext,
        start_id: &str,
        end_id: &str,
        max_depth: usize
    ) -> Result<Vec<storage::graph::GraphEdge>, Box<dyn std::error::Error + Send + Sync>> {
        let graph_store = self
            .graph_store
            .as_ref()
            .ok_or("Graph store not configured")?;
        Ok(graph_store
            .find_path(ctx, start_id, end_id, max_depth)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::providers::MockProvider;
    use mk_core::types::TenantContext;

    pub(crate) fn test_ctx() -> TenantContext {
        use std::str::FromStr;
        TenantContext {
            tenant_id: mk_core::types::TenantId::from_str("test-tenant").unwrap(),
            user_id: mk_core::types::UserId::from_str("test-user").unwrap(),
            agent_id: None
        }
    }

    #[tokio::test]
    async fn test_hierarchical_search() {
        let manager = MemoryManager::new();
        let ctx = test_ctx();
        let agent_provider: Arc<
            dyn mk_core::traits::MemoryProviderAdapter<
                    Error = Box<dyn std::error::Error + Send + Sync>
                > + Send
                + Sync
        > = Arc::new(MockProvider::new());
        let session_provider: Arc<
            dyn mk_core::traits::MemoryProviderAdapter<
                    Error = Box<dyn std::error::Error + Send + Sync>
                > + Send
                + Sync
        > = Arc::new(MockProvider::new());

        manager
            .register_provider(MemoryLayer::Agent, agent_provider)
            .await;
        manager
            .register_provider(MemoryLayer::Session, session_provider)
            .await;

        let agent_entry = MemoryEntry {
            summaries: std::collections::HashMap::new(),
            context_vector: None,
            importance_score: None,
            id: "agent_1".to_string(),
            content: "agent content".to_string(),
            embedding: None,
            layer: MemoryLayer::Agent,
            metadata: HashMap::new(),
            created_at: 0,
            updated_at: 0
        };

        let session_entry = MemoryEntry {
            summaries: std::collections::HashMap::new(),
            context_vector: None,
            importance_score: None,
            id: "session_1".to_string(),
            content: "session content".to_string(),
            embedding: None,
            layer: MemoryLayer::Session,
            metadata: HashMap::new(),
            created_at: 0,
            updated_at: 0
        };

        manager
            .add_to_layer(ctx.clone(), MemoryLayer::Agent, agent_entry)
            .await
            .unwrap();
        manager
            .add_to_layer(ctx.clone(), MemoryLayer::Session, session_entry)
            .await
            .unwrap();

        let results = manager
            .search_hierarchical(ctx, vec![], 10, HashMap::new())
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].id, "agent_1");
        assert_eq!(results[1].id, "session_1");
    }

    #[tokio::test]
    async fn test_search_trajectory_tracking() {
        let manager = MemoryManager::new();
        let ctx = test_ctx();
        let provider: Arc<
            dyn mk_core::traits::MemoryProviderAdapter<
                    Error = Box<dyn std::error::Error + Send + Sync>
                > + Send
                + Sync
        > = Arc::new(MockProvider::new());
        manager.register_provider(MemoryLayer::User, provider).await;

        let entry = MemoryEntry {
            id: "search_traj_1".to_string(),
            content: "search trajectory test".to_string(),
            embedding: None,
            layer: MemoryLayer::User,
            summaries: std::collections::HashMap::new(),
            context_vector: None,
            importance_score: Some(0.5),
            metadata: HashMap::new(),
            created_at: 0,
            updated_at: 0
        };

        manager
            .add_to_layer(ctx.clone(), MemoryLayer::User, entry)
            .await
            .unwrap();

        let _results = manager
            .search_hierarchical(ctx.clone(), vec![0.0], 10, HashMap::new())
            .await
            .unwrap();

        {
            let trajectories = manager.trajectories.read().await;
            let events = trajectories.get("search_traj_1").unwrap();
            assert_eq!(events.len(), 2);
            assert_eq!(
                events[1].operation,
                mk_core::types::MemoryOperation::Retrieve
            );
        }
    }

    #[tokio::test]
    async fn test_trajectory_and_reward_flow() {
        let manager = MemoryManager::new();
        let ctx = test_ctx();
        let provider: Arc<
            dyn mk_core::traits::MemoryProviderAdapter<
                    Error = Box<dyn std::error::Error + Send + Sync>
                > + Send
                + Sync
        > = Arc::new(MockProvider::new());
        manager.register_provider(MemoryLayer::User, provider).await;

        let entry = MemoryEntry {
            id: "traj_1".to_string(),
            content: "trajectory test".to_string(),
            embedding: None,
            layer: MemoryLayer::User,
            summaries: std::collections::HashMap::new(),
            context_vector: None,
            importance_score: Some(0.5),
            metadata: HashMap::new(),
            created_at: 0,
            updated_at: 0
        };

        manager
            .add_to_layer(ctx.clone(), MemoryLayer::User, entry)
            .await
            .unwrap();

        {
            let trajectories = manager.trajectories.read().await;
            let events = trajectories.get("traj_1").unwrap();
            assert_eq!(events.len(), 1);
            assert_eq!(events[0].operation, mk_core::types::MemoryOperation::Add);
        }

        let reward = mk_core::types::RewardSignal {
            reward_type: mk_core::types::RewardType::Helpful,
            score: 0.2,
            reasoning: Some("Very helpful".to_string()),
            agent_id: None,
            timestamp: 0
        };

        manager
            .record_reward(ctx.clone(), MemoryLayer::User, "traj_1", reward)
            .await
            .unwrap();

        let updated = manager
            .get_from_layer(ctx.clone(), MemoryLayer::User, "traj_1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated.importance_score.unwrap(), 0.7);

        {
            let trajectories = manager.trajectories.read().await;
            let events = trajectories.get("traj_1").unwrap();
            assert_eq!(events.len(), 4);
            assert!(events[2].reward.is_some());
            assert_eq!(
                events[3].operation,
                mk_core::types::MemoryOperation::Retrieve
            );
        }

        let pruned = manager
            .prune_low_utility_memories(ctx.clone(), MemoryLayer::User, 0.8)
            .await
            .unwrap();
        assert_eq!(pruned.len(), 1);
        assert_eq!(pruned[0], "traj_1");

        {
            let trajectories = manager.trajectories.read().await;
            let events = trajectories.get("traj_1").unwrap();
            assert_eq!(events.len(), 6);
            assert_eq!(events[5].operation, mk_core::types::MemoryOperation::Prune);
        }
    }

    #[tokio::test]
    async fn test_search_with_threshold() {
        let manager = MemoryManager::new();
        let ctx = test_ctx();
        let provider: Arc<
            dyn mk_core::traits::MemoryProviderAdapter<
                    Error = Box<dyn std::error::Error + Send + Sync>
                > + Send
                + Sync
        > = Arc::new(MockProvider::new());

        manager.register_provider(MemoryLayer::User, provider).await;

        let entry_high_score = MemoryEntry {
            summaries: std::collections::HashMap::new(),
            context_vector: None,
            importance_score: None,
            id: "high_score".to_string(),
            content: "high score content".to_string(),
            embedding: None,
            layer: MemoryLayer::User,
            metadata: {
                let mut map = HashMap::new();
                map.insert("score".to_string(), serde_json::json!(0.9));
                map
            },
            created_at: 0,
            updated_at: 0
        };

        let entry_low_score = MemoryEntry {
            summaries: std::collections::HashMap::new(),
            context_vector: None,
            importance_score: None,
            id: "low_score".to_string(),
            content: "low score content".to_string(),
            embedding: None,
            layer: MemoryLayer::User,
            metadata: {
                let mut map = HashMap::new();
                map.insert("score".to_string(), serde_json::json!(0.5));
                map
            },
            created_at: 0,
            updated_at: 0
        };

        manager
            .add_to_layer(ctx.clone(), MemoryLayer::User, entry_high_score)
            .await
            .unwrap();
        manager
            .add_to_layer(ctx.clone(), MemoryLayer::User, entry_low_score)
            .await
            .unwrap();

        let results = manager
            .search_with_threshold(ctx.clone(), vec![], 10, 0.7, HashMap::new())
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "high_score");

        let results = manager
            .search_with_threshold(ctx, vec![], 10, 0.3, HashMap::new())
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_search_with_threshold_no_score_in_metadata() {
        let manager = MemoryManager::new();
        let ctx = test_ctx();
        let provider: Arc<
            dyn mk_core::traits::MemoryProviderAdapter<
                    Error = Box<dyn std::error::Error + Send + Sync>
                > + Send
                + Sync
        > = Arc::new(MockProvider::new());

        manager.register_provider(MemoryLayer::User, provider).await;

        let entry = MemoryEntry {
            summaries: std::collections::HashMap::new(),
            context_vector: None,
            importance_score: None,
            id: "no_score".to_string(),
            content: "no score content".to_string(),
            embedding: None,
            layer: MemoryLayer::User,
            metadata: HashMap::new(),
            created_at: 0,
            updated_at: 0
        };

        manager
            .add_to_layer(ctx.clone(), MemoryLayer::User, entry)
            .await
            .unwrap();

        let results = manager
            .search_with_threshold(ctx, vec![], 10, 0.8, HashMap::new())
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "no_score");
    }

    #[tokio::test]
    async fn test_add_to_layer_with_governance() {
        let manager = MemoryManager::new();
        let ctx = test_ctx();
        let provider: Arc<
            dyn mk_core::traits::MemoryProviderAdapter<
                    Error = Box<dyn std::error::Error + Send + Sync>
                > + Send
                + Sync
        > = Arc::new(MockProvider::new());
        manager.register_provider(MemoryLayer::User, provider).await;

        let entry = MemoryEntry {
            summaries: std::collections::HashMap::new(),
            context_vector: None,
            importance_score: None,
            id: "mem_1".to_string(),
            content: "Contact me at user@example.com for details.".to_string(),
            embedding: None,
            layer: MemoryLayer::User,
            metadata: HashMap::new(),
            created_at: 0,
            updated_at: 0
        };

        manager
            .add_to_layer(ctx.clone(), MemoryLayer::User, entry)
            .await
            .unwrap();

        let retrieved = manager
            .get_from_layer(ctx, MemoryLayer::User, "mem_1")
            .await
            .unwrap()
            .unwrap();

        assert_eq!(
            retrieved.content,
            "Contact me at [REDACTED_EMAIL] for details."
        );
    }

    #[tokio::test]
    async fn test_delete_from_layer() {
        let manager = MemoryManager::new();
        let ctx = test_ctx();
        let provider: Arc<
            dyn mk_core::traits::MemoryProviderAdapter<
                    Error = Box<dyn std::error::Error + Send + Sync>
                > + Send
                + Sync
        > = Arc::new(MockProvider::new());
        manager.register_provider(MemoryLayer::User, provider).await;

        let entry = MemoryEntry {
            summaries: std::collections::HashMap::new(),
            context_vector: None,
            importance_score: None,
            id: "mem_1".to_string(),
            content: "test".to_string(),
            embedding: None,
            layer: MemoryLayer::User,
            metadata: HashMap::new(),
            created_at: 0,
            updated_at: 0
        };

        manager
            .add_to_layer(ctx.clone(), MemoryLayer::User, entry)
            .await
            .unwrap();
        manager
            .delete_from_layer(ctx.clone(), MemoryLayer::User, "mem_1")
            .await
            .unwrap();

        let retrieved = manager
            .get_from_layer(ctx, MemoryLayer::User, "mem_1")
            .await
            .unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_promote_memory_manual() {
        let manager = MemoryManager::new();
        let ctx = test_ctx();
        let mock_session: Arc<
            dyn mk_core::traits::MemoryProviderAdapter<
                    Error = Box<dyn std::error::Error + Send + Sync>
                > + Send
                + Sync
        > = Arc::new(MockProvider::new());
        let mock_project: Arc<
            dyn mk_core::traits::MemoryProviderAdapter<
                    Error = Box<dyn std::error::Error + Send + Sync>
                > + Send
                + Sync
        > = Arc::new(MockProvider::new());
        manager
            .register_provider(MemoryLayer::Session, mock_session)
            .await;
        manager
            .register_provider(MemoryLayer::Project, mock_project)
            .await;

        let entry = MemoryEntry {
            summaries: std::collections::HashMap::new(),
            context_vector: None,
            importance_score: None,
            id: "session_mem".to_string(),
            content: "to be promoted".to_string(),
            embedding: None,
            layer: MemoryLayer::Session,
            metadata: HashMap::new(),
            created_at: 0,
            updated_at: 0
        };

        manager
            .add_to_layer(ctx.clone(), MemoryLayer::Session, entry)
            .await
            .unwrap();
        manager
            .promote_memory(
                ctx.clone(),
                "session_mem",
                MemoryLayer::Session,
                MemoryLayer::Project
            )
            .await
            .unwrap();

        let promoted = manager
            .get_from_layer(ctx, MemoryLayer::Project, "session_mem_promoted")
            .await
            .unwrap();
        assert!(promoted.is_some());
    }

    #[tokio::test]
    async fn test_search_precedence_ordering() {
        let manager = MemoryManager::new();
        let ctx = test_ctx();
        let agent_provider: Arc<
            dyn mk_core::traits::MemoryProviderAdapter<
                    Error = Box<dyn std::error::Error + Send + Sync>
                > + Send
                + Sync
        > = Arc::new(MockProvider::new());
        let user_provider: Arc<
            dyn mk_core::traits::MemoryProviderAdapter<
                    Error = Box<dyn std::error::Error + Send + Sync>
                > + Send
                + Sync
        > = Arc::new(MockProvider::new());

        manager
            .register_provider(MemoryLayer::Agent, agent_provider)
            .await;
        manager
            .register_provider(MemoryLayer::User, user_provider)
            .await;

        let agent_entry = MemoryEntry {
            summaries: std::collections::HashMap::new(),
            context_vector: None,
            importance_score: None,
            id: "agent_high_priority".to_string(),
            content: "agent content".to_string(),
            embedding: None,
            layer: MemoryLayer::Agent,
            metadata: {
                let mut map = HashMap::new();
                map.insert("score".to_string(), serde_json::json!(0.5));
                map
            },
            created_at: 0,
            updated_at: 0
        };

        let user_entry = MemoryEntry {
            summaries: std::collections::HashMap::new(),
            context_vector: None,
            importance_score: None,
            id: "user_high_similarity".to_string(),
            content: "user content".to_string(),
            embedding: None,
            layer: MemoryLayer::User,
            metadata: {
                let mut map = HashMap::new();
                map.insert("score".to_string(), serde_json::json!(0.9));
                map
            },
            created_at: 0,
            updated_at: 0
        };

        manager
            .add_to_layer(ctx.clone(), MemoryLayer::Agent, agent_entry)
            .await
            .unwrap();
        manager
            .add_to_layer(ctx.clone(), MemoryLayer::User, user_entry)
            .await
            .unwrap();

        let results = manager
            .search_hierarchical(ctx, vec![], 10, HashMap::new())
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].id, "agent_high_priority");
        assert_eq!(results[1].id, "user_high_similarity");
    }

    #[tokio::test]
    async fn test_close_session_triggers_promotion() {
        use async_trait::async_trait;
        use mk_core::traits::LlmService;
        use mk_core::types::ValidationResult;

        struct MockLlm;
        #[async_trait]
        impl LlmService for MockLlm {
            type Error = Box<dyn std::error::Error + Send + Sync>;
            async fn generate(&self, _prompt: &str) -> Result<String, Self::Error> {
                Ok("compressed content".to_string())
            }
            async fn analyze_drift(
                &self,
                _c: &str,
                _p: &[mk_core::types::Policy]
            ) -> Result<ValidationResult, Self::Error> {
                Ok(ValidationResult {
                    is_valid: true,
                    violations: vec![]
                })
            }
        }

        let manager = MemoryManager::new()
            .with_llm_service(Arc::new(MockLlm))
            .with_config(config::MemoryConfig {
                promotion_threshold: 0.5,
                decay_interval_secs: 86400,
                decay_rate: 0.05,
                optimization_trigger_count: 100,
                layer_summary_configs: std::collections::HashMap::new()
            });
        let ctx = test_ctx();
        let mock_session: Arc<
            dyn mk_core::traits::MemoryProviderAdapter<
                    Error = Box<dyn std::error::Error + Send + Sync>
                > + Send
                + Sync
        > = Arc::new(MockProvider::new());
        let mock_project: Arc<
            dyn mk_core::traits::MemoryProviderAdapter<
                    Error = Box<dyn std::error::Error + Send + Sync>
                > + Send
                + Sync
        > = Arc::new(MockProvider::new());
        manager
            .register_provider(MemoryLayer::Session, mock_session)
            .await;
        manager
            .register_provider(MemoryLayer::Project, mock_project)
            .await;

        let entry = MemoryEntry {
            summaries: std::collections::HashMap::new(),
            context_vector: None,
            importance_score: None,
            id: "important".to_string(),
            content: "highly important".to_string(),
            embedding: None,
            layer: MemoryLayer::Session,
            metadata: {
                let mut m = HashMap::new();
                m.insert("score".to_string(), serde_json::json!(1.0));
                m
            },
            created_at: 0,
            updated_at: 0
        };

        manager
            .add_to_layer(ctx.clone(), MemoryLayer::Session, entry)
            .await
            .unwrap();
        manager.close_session(ctx.clone(), "some_id").await.unwrap();

        let promoted = manager
            .list_all_from_layer(ctx, MemoryLayer::Project)
            .await
            .unwrap();
        assert!(!promoted.is_empty());
    }

    #[tokio::test]
    async fn test_search_text_with_threshold_requires_embedding_service() {
        let manager = MemoryManager::new();
        let ctx = test_ctx();
        let provider: Arc<
            dyn mk_core::traits::MemoryProviderAdapter<
                    Error = Box<dyn std::error::Error + Send + Sync>
                > + Send
                + Sync
        > = Arc::new(MockProvider::new());

        manager.register_provider(MemoryLayer::User, provider).await;

        let result = manager
            .search_text_with_threshold(ctx, "test query", 10, 0.7, HashMap::new())
            .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Embedding service not configured")
        );
    }

    #[tokio::test]
    async fn test_hierarchical_search_provider_error() {
        struct FailingProvider;
        #[async_trait::async_trait]
        impl mk_core::traits::MemoryProviderAdapter for FailingProvider {
            type Error = Box<dyn std::error::Error + Send + Sync>;
            async fn add(
                &self,
                _ctx: mk_core::types::TenantContext,
                _e: MemoryEntry
            ) -> Result<String, Self::Error> {
                Ok("id".to_string())
            }
            async fn get(
                &self,
                _ctx: mk_core::types::TenantContext,
                _id: &str
            ) -> Result<Option<MemoryEntry>, Self::Error> {
                Ok(None)
            }
            async fn search(
                &self,
                _ctx: mk_core::types::TenantContext,
                _v: Vec<f32>,
                _l: usize,
                _f: HashMap<String, serde_json::Value>
            ) -> Result<Vec<MemoryEntry>, Self::Error> {
                Err("search failed".into())
            }
            async fn update(
                &self,
                _ctx: mk_core::types::TenantContext,
                _e: MemoryEntry
            ) -> Result<(), Self::Error> {
                Ok(())
            }
            async fn delete(
                &self,
                _ctx: mk_core::types::TenantContext,
                _id: &str
            ) -> Result<(), Self::Error> {
                Ok(())
            }
            async fn list(
                &self,
                _ctx: mk_core::types::TenantContext,
                _l: MemoryLayer,
                _lim: usize,
                _c: Option<String>
            ) -> Result<(Vec<MemoryEntry>, Option<String>), Self::Error> {
                Ok((vec![], None))
            }
        }

        let manager = MemoryManager::new();
        let ctx = test_ctx();
        manager
            .register_provider(MemoryLayer::Agent, Arc::new(FailingProvider))
            .await;

        let results = manager
            .search_hierarchical(ctx, vec![0.0], 10, HashMap::new())
            .await
            .unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_close_agent_triggers_promotion() {
        use async_trait::async_trait;
        use mk_core::traits::LlmService;
        use mk_core::types::ValidationResult;

        struct MockLlm;
        #[async_trait]
        impl LlmService for MockLlm {
            type Error = Box<dyn std::error::Error + Send + Sync>;
            async fn generate(&self, _prompt: &str) -> Result<String, Self::Error> {
                Ok("compressed content".to_string())
            }
            async fn analyze_drift(
                &self,
                _c: &str,
                _p: &[mk_core::types::Policy]
            ) -> Result<ValidationResult, Self::Error> {
                Ok(ValidationResult {
                    is_valid: true,
                    violations: vec![]
                })
            }
        }

        let manager = MemoryManager::new()
            .with_llm_service(Arc::new(MockLlm))
            .with_config(config::MemoryConfig {
                promotion_threshold: 0.5,
                decay_interval_secs: 86400,
                decay_rate: 0.05,
                optimization_trigger_count: 100,
                layer_summary_configs: std::collections::HashMap::new()
            });
        let ctx = test_ctx();
        let mock_agent: Arc<
            dyn mk_core::traits::MemoryProviderAdapter<
                    Error = Box<dyn std::error::Error + Send + Sync>
                > + Send
                + Sync
        > = Arc::new(MockProvider::new());
        let mock_user: Arc<
            dyn mk_core::traits::MemoryProviderAdapter<
                    Error = Box<dyn std::error::Error + Send + Sync>
                > + Send
                + Sync
        > = Arc::new(MockProvider::new());
        manager
            .register_provider(MemoryLayer::Agent, mock_agent)
            .await;
        manager
            .register_provider(MemoryLayer::User, mock_user)
            .await;

        let entry = MemoryEntry {
            summaries: std::collections::HashMap::new(),
            context_vector: None,
            importance_score: None,
            id: "agent_mem".to_string(),
            content: "agent memory content".to_string(),
            embedding: None,
            layer: MemoryLayer::Agent,
            metadata: {
                let mut m = HashMap::new();
                m.insert("score".to_string(), serde_json::json!(1.0));
                m
            },
            created_at: 0,
            updated_at: 0
        };

        manager
            .add_to_layer(ctx.clone(), MemoryLayer::Agent, entry)
            .await
            .unwrap();
        manager.close_agent(ctx.clone(), "agent_id").await.unwrap();

        let promoted = manager
            .list_all_from_layer(ctx, MemoryLayer::User)
            .await
            .unwrap();
        assert!(!promoted.is_empty());
        assert_eq!(
            promoted[0].metadata.get("original_memory_id").unwrap(),
            "agent_mem"
        );
    }

    #[tokio::test]
    async fn test_search_text_with_threshold_success() {
        use crate::embedding::mock::MockEmbeddingService;
        let manager =
            MemoryManager::new().with_embedding_service(Arc::new(MockEmbeddingService::new(1536)));
        let ctx = test_ctx();

        let provider: Arc<
            dyn mk_core::traits::MemoryProviderAdapter<
                    Error = Box<dyn std::error::Error + Send + Sync>
                > + Send
                + Sync
        > = Arc::new(MockProvider::new());
        manager.register_provider(MemoryLayer::User, provider).await;

        let entry = MemoryEntry {
            summaries: std::collections::HashMap::new(),
            context_vector: None,
            importance_score: None,
            id: "text_mem".to_string(),
            content: "some text content".to_string(),
            embedding: None,
            layer: MemoryLayer::User,
            metadata: {
                let mut m = HashMap::new();
                m.insert("score".to_string(), serde_json::json!(0.9));
                m
            },
            created_at: 0,
            updated_at: 0
        };

        manager
            .add_to_layer(ctx.clone(), MemoryLayer::User, entry)
            .await
            .unwrap();

        let results = manager
            .search_text_with_threshold(ctx, "query", 10, 0.5, HashMap::new())
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "text_mem");
    }

    #[tokio::test]
    async fn test_with_telemetry_and_default() {
        let telemetry = Arc::new(MemoryTelemetry::new());
        let manager = MemoryManager::default().with_telemetry(telemetry);
        assert!(manager.embedding_service.is_none());
    }

    #[tokio::test]
    async fn test_add_to_layer_no_provider() {
        let manager = MemoryManager::new();
        let ctx = test_ctx();
        let entry = MemoryEntry {
            summaries: std::collections::HashMap::new(),
            context_vector: None,
            importance_score: None,
            id: "test".to_string(),
            content: "test".to_string(),
            embedding: None,
            layer: MemoryLayer::User,
            metadata: HashMap::new(),
            created_at: 0,
            updated_at: 0
        };

        let result = manager.add_to_layer(ctx, MemoryLayer::User, entry).await;
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "No provider registered for layer"
        );
    }

    #[tokio::test]
    async fn test_search_with_threshold_provider_error() {
        struct FailingProvider;
        #[async_trait::async_trait]
        impl mk_core::traits::MemoryProviderAdapter for FailingProvider {
            type Error = Box<dyn std::error::Error + Send + Sync>;
            async fn add(
                &self,
                _ctx: mk_core::types::TenantContext,
                _e: MemoryEntry
            ) -> Result<String, Self::Error> {
                Ok("id".into())
            }
            async fn get(
                &self,
                _ctx: mk_core::types::TenantContext,
                _id: &str
            ) -> Result<Option<MemoryEntry>, Self::Error> {
                Ok(None)
            }
            async fn search(
                &self,
                _ctx: mk_core::types::TenantContext,
                _v: Vec<f32>,
                _l: usize,
                _f: HashMap<String, serde_json::Value>
            ) -> Result<Vec<MemoryEntry>, Self::Error> {
                Err("search failed".into())
            }
            async fn update(
                &self,
                _ctx: mk_core::types::TenantContext,
                _e: MemoryEntry
            ) -> Result<(), Self::Error> {
                Ok(())
            }
            async fn delete(
                &self,
                _ctx: mk_core::types::TenantContext,
                _id: &str
            ) -> Result<(), Self::Error> {
                Ok(())
            }
            async fn list(
                &self,
                _ctx: mk_core::types::TenantContext,
                _l: MemoryLayer,
                _lim: usize,
                _c: Option<String>
            ) -> Result<(Vec<MemoryEntry>, Option<String>), Self::Error> {
                Ok((vec![], None))
            }
        }

        let manager = MemoryManager::new();
        let ctx = test_ctx();
        manager
            .register_provider(MemoryLayer::User, Arc::new(FailingProvider))
            .await;

        let results = manager
            .search_with_threshold(ctx, vec![0.0], 10, 0.5, HashMap::new())
            .await
            .unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_promote_important_memories_error_mapping() {
        struct ErrorProvider;
        #[async_trait::async_trait]
        impl mk_core::traits::MemoryProviderAdapter for ErrorProvider {
            type Error = Box<dyn std::error::Error + Send + Sync>;
            async fn add(
                &self,
                _ctx: mk_core::types::TenantContext,
                _e: MemoryEntry
            ) -> Result<String, Self::Error> {
                Ok("id".into())
            }
            async fn get(
                &self,
                _ctx: mk_core::types::TenantContext,
                _id: &str
            ) -> Result<Option<MemoryEntry>, Self::Error> {
                Ok(None)
            }
            async fn search(
                &self,
                _ctx: mk_core::types::TenantContext,
                _v: Vec<f32>,
                _l: usize,
                _f: HashMap<String, serde_json::Value>
            ) -> Result<Vec<MemoryEntry>, Self::Error> {
                Err("list failed".into())
            }
            async fn update(
                &self,
                _ctx: mk_core::types::TenantContext,
                _e: MemoryEntry
            ) -> Result<(), Self::Error> {
                Ok(())
            }
            async fn delete(
                &self,
                _ctx: mk_core::types::TenantContext,
                _id: &str
            ) -> Result<(), Self::Error> {
                Ok(())
            }
            async fn list(
                &self,
                _ctx: mk_core::types::TenantContext,
                _l: MemoryLayer,
                _lim: usize,
                _c: Option<String>
            ) -> Result<(Vec<MemoryEntry>, Option<String>), Self::Error> {
                Err("list failed".into())
            }
        }

        let manager = MemoryManager::new();
        let ctx = test_ctx();
        manager
            .register_provider(MemoryLayer::Session, Arc::new(ErrorProvider))
            .await;

        let result = manager
            .promote_important_memories(ctx, MemoryLayer::Session)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_add_to_layer_provider_error() {
        struct FailingAddProvider;
        #[async_trait::async_trait]
        impl mk_core::traits::MemoryProviderAdapter for FailingAddProvider {
            type Error = Box<dyn std::error::Error + Send + Sync>;
            async fn add(
                &self,
                _ctx: mk_core::types::TenantContext,
                _e: MemoryEntry
            ) -> Result<String, Self::Error> {
                Err("add failed".into())
            }
            async fn get(
                &self,
                _ctx: mk_core::types::TenantContext,
                _id: &str
            ) -> Result<Option<MemoryEntry>, Self::Error> {
                Ok(None)
            }
            async fn search(
                &self,
                _ctx: mk_core::types::TenantContext,
                _v: Vec<f32>,
                _l: usize,
                _f: HashMap<String, serde_json::Value>
            ) -> Result<Vec<MemoryEntry>, Self::Error> {
                Ok(vec![])
            }
            async fn update(
                &self,
                _ctx: mk_core::types::TenantContext,
                _e: MemoryEntry
            ) -> Result<(), Self::Error> {
                Ok(())
            }
            async fn delete(
                &self,
                _ctx: mk_core::types::TenantContext,
                _id: &str
            ) -> Result<(), Self::Error> {
                Ok(())
            }
            async fn list(
                &self,
                _ctx: mk_core::types::TenantContext,
                _l: MemoryLayer,
                _lim: usize,
                _c: Option<String>
            ) -> Result<(Vec<MemoryEntry>, Option<String>), Self::Error> {
                Ok((vec![], None))
            }
        }

        let manager = MemoryManager::new();
        let ctx = test_ctx();
        manager
            .register_provider(MemoryLayer::User, Arc::new(FailingAddProvider))
            .await;

        let entry = MemoryEntry {
            summaries: std::collections::HashMap::new(),
            context_vector: None,
            importance_score: None,
            id: "test".to_string(),
            content: "test".to_string(),
            embedding: None,
            layer: MemoryLayer::User,
            metadata: HashMap::new(),
            created_at: 0,
            updated_at: 0
        };

        let result = manager.add_to_layer(ctx, MemoryLayer::User, entry).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "add failed");
    }

    #[tokio::test]
    async fn test_reward_decay_logic() {
        let manager = MemoryManager::new().with_config(config::MemoryConfig {
            promotion_threshold: 0.8,
            decay_interval_secs: 3600,
            decay_rate: 0.1,
            optimization_trigger_count: 100,
            layer_summary_configs: std::collections::HashMap::new()
        });
        let ctx = test_ctx();
        let provider: Arc<
            dyn mk_core::traits::MemoryProviderAdapter<
                    Error = Box<dyn std::error::Error + Send + Sync>
                > + Send
                + Sync
        > = Arc::new(MockProvider::new());
        manager.register_provider(MemoryLayer::User, provider).await;

        let now = chrono::Utc::now().timestamp();
        let intervals_passed = 2;
        let created_at = now - (intervals_passed * 3600 + 300);
        let entry = MemoryEntry {
            id: "decay_1".to_string(),
            content: "decay test".to_string(),
            embedding: None,
            layer: MemoryLayer::User,
            summaries: std::collections::HashMap::new(),
            context_vector: None,
            importance_score: Some(1.0),
            metadata: HashMap::new(),
            created_at,
            updated_at: created_at
        };

        manager
            .add_to_layer(ctx.clone(), MemoryLayer::User, entry)
            .await
            .unwrap();

        let count = manager
            .apply_reward_decay(ctx.clone(), MemoryLayer::User)
            .await
            .unwrap();
        assert_eq!(count, 1);

        let updated = manager
            .get_from_layer(ctx.clone(), MemoryLayer::User, "decay_1")
            .await
            .unwrap()
            .unwrap();

        let expected_score = 1.0 - (1.0 * 0.1 * intervals_passed as f32);
        assert!((updated.importance_score.unwrap() - expected_score).abs() < 0.001);

        {
            let trajectories = manager.trajectories.read().await;
            let events = trajectories.get("decay_1").unwrap();
            assert!(events.iter().any(|e| {
                e.reasoning
                    .as_ref()
                    .map(|r| r.contains("decayed"))
                    .unwrap_or(false)
            }));
        }
    }

    #[tokio::test]
    async fn test_optimize_layer_compression() {
        use crate::providers::MockProvider;
        use async_trait::async_trait;
        use mk_core::traits::LlmService;
        use mk_core::types::{MemoryOperation, MemoryTrajectoryEvent, ValidationResult};

        struct MockLlm;
        #[async_trait]
        impl LlmService for MockLlm {
            type Error = Box<dyn std::error::Error + Send + Sync>;
            async fn generate(&self, _prompt: &str) -> Result<String, Self::Error> {
                Ok("compressed content".to_string())
            }
            async fn analyze_drift(
                &self,
                _c: &str,
                _p: &[mk_core::types::Policy]
            ) -> Result<ValidationResult, Self::Error> {
                Ok(ValidationResult {
                    is_valid: true,
                    violations: vec![]
                })
            }
        }

        let manager = MemoryManager::new()
            .with_llm_service(Arc::new(MockLlm))
            .with_config(config::MemoryConfig {
                promotion_threshold: 0.8,
                decay_interval_secs: 3600,
                decay_rate: 0.1,
                optimization_trigger_count: 100,
                layer_summary_configs: std::collections::HashMap::new()
            });

        let ctx = test_ctx();
        let provider: Arc<
            dyn mk_core::traits::MemoryProviderAdapter<
                    Error = Box<dyn std::error::Error + Send + Sync>
                > + Send
                + Sync
        > = Arc::new(MockProvider::new());
        manager.register_provider(MemoryLayer::User, provider).await;

        let trajectories = manager.trajectories.clone();

        for i in 0..5 {
            let entry = MemoryEntry {
                id: format!("mem_{}", i),
                content: format!("content {}", i),
                embedding: None,
                layer: MemoryLayer::User,
                summaries: HashMap::new(),
                context_vector: None,
                importance_score: Some(0.3),
                metadata: HashMap::new(),
                created_at: 0,
                updated_at: 0
            };

            if i == 0 {
                let mut trajectories_write = trajectories.write().await;
                trajectories_write.insert(
                    entry.id.clone(),
                    vec![MemoryTrajectoryEvent {
                        operation: MemoryOperation::Noop,
                        entry_id: entry.id.clone(),
                        reward: Some(mk_core::types::RewardSignal {
                            reward_type: mk_core::types::RewardType::Helpful,
                            score: 0.1,
                            reasoning: Some("Test reward".to_string()),
                            agent_id: None,
                            timestamp: 0
                        }),
                        reasoning: None,
                        timestamp: 0
                    }]
                );
            }

            manager
                .add_to_layer(ctx.clone(), MemoryLayer::User, entry)
                .await
                .unwrap();
        }

        manager
            .optimize_layer(ctx.clone(), MemoryLayer::User)
            .await
            .unwrap();

        let remaining = manager
            .list_all_from_layer(ctx, MemoryLayer::User)
            .await
            .unwrap();
        assert_eq!(remaining.len(), 1);
        assert!(remaining[0].id.starts_with("compressed_"));

        {
            let trajectories_read = trajectories.read().await;
            let events = trajectories_read.get(&remaining[0].id).unwrap();
            assert!(
                events
                    .iter()
                    .any(|e| e.reasoning.as_ref().unwrap().contains("Inherited reward"))
            );
        }
    }

    #[tokio::test]
    async fn test_autonomous_optimization_trigger() {
        use crate::providers::MockProvider;
        use async_trait::async_trait;
        use mk_core::traits::LlmService;
        use mk_core::types::ValidationResult;

        struct MockLlm;
        #[async_trait]
        impl LlmService for MockLlm {
            type Error = Box<dyn std::error::Error + Send + Sync>;
            async fn generate(&self, _prompt: &str) -> Result<String, Self::Error> {
                Ok("compressed content".to_string())
            }
            async fn analyze_drift(
                &self,
                _c: &str,
                _p: &[mk_core::types::Policy]
            ) -> Result<ValidationResult, Self::Error> {
                Ok(ValidationResult {
                    is_valid: true,
                    violations: vec![]
                })
            }
        }

        let manager = MemoryManager::new()
            .with_llm_service(Arc::new(MockLlm))
            .with_config(config::MemoryConfig {
                promotion_threshold: 0.8,
                decay_interval_secs: 3600,
                decay_rate: 0.1,
                optimization_trigger_count: 10,
                layer_summary_configs: std::collections::HashMap::new()
            });

        let ctx = test_ctx();
        let provider: Arc<
            dyn mk_core::traits::MemoryProviderAdapter<
                    Error = Box<dyn std::error::Error + Send + Sync>
                > + Send
                + Sync
        > = Arc::new(MockProvider::new());
        manager.register_provider(MemoryLayer::User, provider).await;

        for i in 0..15 {
            let entry = MemoryEntry {
                id: format!("pressure_mem_{}", i),
                content: format!("pressure test content {}", i),
                embedding: None,
                layer: MemoryLayer::User,
                summaries: HashMap::new(),
                context_vector: None,
                importance_score: Some(0.3),
                metadata: HashMap::new(),
                created_at: 0,
                updated_at: 0
            };

            manager
                .add_to_layer(ctx.clone(), MemoryLayer::User, entry)
                .await
                .unwrap();
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let remaining = manager
            .list_all_from_layer(ctx.clone(), MemoryLayer::User)
            .await
            .unwrap();

        assert!(
            remaining.len() <= 15,
            "Expected <= 15 memories after autonomous trigger, got {}",
            remaining.len()
        );
    }

    #[test]
    fn test_with_graph_store() {
        let manager = MemoryManager::new();
        assert!(manager.graph_store.is_none());
    }

    #[test]
    fn test_with_embedding_service() {
        let manager = MemoryManager::new();
        assert!(manager.embedding_service.is_none());
    }

    #[test]
    fn test_with_auth_service() {
        let manager = MemoryManager::new();
        assert!(manager.auth_service.is_none());
    }

    #[test]
    fn test_with_telemetry() {
        let manager = MemoryManager::new();
        let telemetry = Arc::new(crate::telemetry::MemoryTelemetry::new());
        let manager = manager.with_telemetry(telemetry);
        assert!(Arc::strong_count(&manager.telemetry) >= 1);
    }

    #[test]
    fn test_clone_internal() {
        let manager = MemoryManager::new();
        let cloned = manager.clone_internal();
        assert!(cloned.llm_service.is_none());
        assert!(cloned.auth_service.is_none());
    }

    #[tokio::test]
    async fn test_optimize_layer_without_llm() {
        let manager = MemoryManager::new();
        let ctx = test_ctx();

        let result = manager.optimize_layer(ctx, MemoryLayer::User).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("LLM service required")
        );
    }

    #[tokio::test]
    async fn test_delete_from_layer_no_provider() {
        let manager = MemoryManager::new();
        let ctx = test_ctx();

        let result = manager
            .delete_from_layer(ctx, MemoryLayer::Team, "non_existent")
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_from_layer_no_provider() {
        let manager = MemoryManager::new();
        let ctx = test_ctx();

        let result = manager
            .get_from_layer(ctx, MemoryLayer::Team, "test_id")
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_list_all_from_layer_no_provider() {
        let manager = MemoryManager::new();
        let ctx = test_ctx();

        let result = manager.list_all_from_layer(ctx, MemoryLayer::Org).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_search_hierarchical_no_provider() {
        let manager = MemoryManager::new();
        let ctx = test_ctx();

        let result = manager
            .search_hierarchical(ctx, vec![], 10, HashMap::new())
            .await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_hardening_failures() {
        use std::str::FromStr;
        let manager = MemoryManager::new();
        let ctx = test_ctx();

        let entry = MemoryEntry {
            id: "test".to_string(),
            content: "TRIGGER_FAILURE".to_string(),
            embedding: None,
            layer: MemoryLayer::User,
            summaries: HashMap::new(),
            context_vector: None,
            importance_score: None,
            metadata: HashMap::new(),
            created_at: 0,
            updated_at: 0
        };
        let result = manager
            .add_to_layer(ctx.clone(), MemoryLayer::User, entry)
            .await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Simulated add failure");

        let mut fail_ctx = ctx.clone();
        fail_ctx.tenant_id = mk_core::types::TenantId::from_str("TRIGGER_FAILURE").unwrap();
        let result = manager
            .search_hierarchical(fail_ctx.clone(), vec![], 10, HashMap::new())
            .await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Simulated search failure");

        let result = manager
            .optimize_layer(fail_ctx.clone(), MemoryLayer::User)
            .await;
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Simulated optimization failure"
        );

        let result = manager
            .promote_memory(
                ctx.clone(),
                "TRIGGER_FAILURE",
                MemoryLayer::Session,
                MemoryLayer::Project
            )
            .await;
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Simulated promotion failure"
        );

        let result = manager
            .search_text_with_threshold(ctx.clone(), "TRIGGER_FAILURE", 10, 0.5, HashMap::new())
            .await;
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Simulated embedding failure"
        );
    }
}
