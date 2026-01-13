use crate::tools::Tool;
use async_trait::async_trait;
use memory::manager::MemoryManager;
use mk_core::types::TenantContext;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::Arc;
use validator::Validate;

pub struct MemoryAddTool {
    memory_manager: Arc<MemoryManager>,
}

impl MemoryAddTool {
    pub fn new(memory_manager: Arc<MemoryManager>) -> Self {
        Self { memory_manager }
    }
}

#[derive(Serialize, Deserialize, JsonSchema, Validate)]
pub struct MemoryAddParams {
    pub content: String,
    pub layer: String,
    #[serde(default)]
    pub metadata: serde_json::Map<String, Value>,
    #[serde(rename = "tenantContext")]
    pub tenant_context: Option<TenantContext>,
}

#[derive(Serialize, Deserialize, JsonSchema, Validate)]
pub struct MemorySearchParams {
    pub query: String,
    pub limit: Option<usize>,
    pub threshold: Option<f32>,
    #[serde(default)]
    pub filters: serde_json::Map<String, Value>,
    #[serde(rename = "tenantContext")]
    pub tenant_context: Option<TenantContext>,
}

#[derive(Serialize, Deserialize, JsonSchema, Validate)]
pub struct MemoryDeleteParams {
    #[serde(rename = "memoryId")]
    pub memory_id: String,
    pub layer: String,
    #[serde(rename = "tenantContext")]
    pub tenant_context: Option<TenantContext>,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug)]
#[serde(rename_all = "camelCase")]
pub enum CloseTarget {
    Session,
    Agent,
}

#[derive(Serialize, Deserialize, JsonSchema, Validate)]
pub struct MemoryCloseParams {
    pub id: String,
    pub target: CloseTarget,
    #[serde(rename = "tenantContext")]
    pub tenant_context: Option<TenantContext>,
}

#[async_trait]
impl Tool for MemoryAddTool {
    fn name(&self) -> &str {
        "memory_add"
    }

    fn description(&self) -> &str {
        "Store a piece of information in memory for future reference."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "content": { "type": "string" },
                "layer": { "type": "string" },
                "metadata": { "type": "object" },
                "tenantContext": { "$ref": "#/definitions/TenantContext" }
            },
            "required": ["content", "layer"]
        })
    }

    async fn call(&self, params: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let p: MemoryAddParams = serde_json::from_value(params)?;
        p.validate()?;

        let ctx = p.tenant_context.ok_or("Missing tenant context")?;

        let layer = match p.layer.to_lowercase().as_str() {
            "agent" => mk_core::types::MemoryLayer::Agent,
            "user" => mk_core::types::MemoryLayer::User,
            "session" => mk_core::types::MemoryLayer::Session,
            "project" => mk_core::types::MemoryLayer::Project,
            "team" => mk_core::types::MemoryLayer::Team,
            "org" => mk_core::types::MemoryLayer::Org,
            "company" => mk_core::types::MemoryLayer::Company,
            _ => return Err(format!("Unknown layer: {}", p.layer).into()),
        };
        let entry = mk_core::types::MemoryEntry {
            id: uuid::Uuid::new_v4().to_string(),
            content: p.content,
            embedding: None,
            layer,
            summaries: std::collections::HashMap::new(),
            context_vector: None,
            importance_score: None,
            metadata: p.metadata.into_iter().collect(),
            created_at: chrono::Utc::now().timestamp(),
            updated_at: chrono::Utc::now().timestamp(),
        };

        let id = self.memory_manager.add_to_layer(ctx, layer, entry).await?;
        Ok(json!({ "success": true, "memoryId": id }))
    }
}

pub struct MemorySearchTool {
    memory_manager: Arc<MemoryManager>,
}

impl MemorySearchTool {
    pub fn new(memory_manager: Arc<MemoryManager>) -> Self {
        Self { memory_manager }
    }
}

#[async_trait]
impl Tool for MemorySearchTool {
    fn name(&self) -> &str {
        "memory_search"
    }

    fn description(&self) -> &str {
        "Search for memories across layers."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string" },
                "limit": { "type": "integer" },
                "threshold": { "type": "number" },
                "filters": { "type": "object" },
                "tenantContext": { "$ref": "#/definitions/TenantContext" }
            },
            "required": ["query"]
        })
    }

    async fn call(&self, params: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let p: MemorySearchParams = serde_json::from_value(params)?;
        p.validate()?;

        let ctx = p.tenant_context.ok_or("Missing tenant context")?;

        let limit = p.limit.unwrap_or(10);
        let threshold = p.threshold.unwrap_or(0.0);
        let filters: std::collections::HashMap<String, Value> = p.filters.into_iter().collect();

        let results = self
            .memory_manager
            .search_text_with_threshold(ctx, &p.query, limit, threshold, filters)
            .await?;

        Ok(json!({
            "success": true,
            "results": results,
            "totalCount": results.len()
        }))
    }
}

pub struct MemoryDeleteTool {
    memory_manager: Arc<MemoryManager>,
}

impl MemoryDeleteTool {
    pub fn new(memory_manager: Arc<MemoryManager>) -> Self {
        Self { memory_manager }
    }
}

#[async_trait]
impl Tool for MemoryDeleteTool {
    fn name(&self) -> &str {
        "memory_delete"
    }

    fn description(&self) -> &str {
        "Delete a memory from a specific layer."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "memoryId": { "type": "string" },
                "layer": { "type": "string" },
                "tenantContext": { "$ref": "#/definitions/TenantContext" }
            },
            "required": ["memoryId", "layer"]
        })
    }

    async fn call(&self, params: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let p: MemoryDeleteParams = serde_json::from_value(params)?;
        p.validate()?;

        let ctx = p.tenant_context.ok_or("Missing tenant context")?;

        let layer = match p.layer.to_lowercase().as_str() {
            "agent" => mk_core::types::MemoryLayer::Agent,
            "user" => mk_core::types::MemoryLayer::User,
            "session" => mk_core::types::MemoryLayer::Session,
            "project" => mk_core::types::MemoryLayer::Project,
            "team" => mk_core::types::MemoryLayer::Team,
            "org" => mk_core::types::MemoryLayer::Org,
            "company" => mk_core::types::MemoryLayer::Company,
            _ => return Err(format!("Unknown layer: {}", p.layer).into()),
        };

        self.memory_manager
            .delete_from_layer(ctx, layer, &p.memory_id)
            .await?;

        Ok(json!({
            "success": true,
            "message": "Memory deleted successfully"
        }))
    }
}

#[derive(Serialize, Deserialize, JsonSchema, Validate)]
pub struct MemoryReasonParams {
    pub query: String,
    #[serde(rename = "contextSummary")]
    pub context_summary: Option<String>,
}

pub struct MemoryReasonTool {
    reasoner: Arc<dyn memory::reasoning::ReflectiveReasoner>,
}

impl MemoryReasonTool {
    pub fn new(reasoner: Arc<dyn memory::reasoning::ReflectiveReasoner>) -> Self {
        Self { reasoner }
    }
}

#[async_trait]
impl Tool for MemoryReasonTool {
    fn name(&self) -> &str {
        "memory_reason"
    }

    fn description(&self) -> &str {
        "Perform reflective reasoning on a query to determine the best retrieval strategy."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string" },
                "contextSummary": { "type": "string" }
            },
            "required": ["query"]
        })
    }

    async fn call(&self, params: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let p: MemoryReasonParams = serde_json::from_value(params)?;
        p.validate()?;

        let trace = self
            .reasoner
            .reason(&p.query, p.context_summary.as_deref())
            .await?;

        Ok(serde_json::to_value(trace)?)
    }
}

pub struct MemoryCloseTool {
    memory_manager: Arc<MemoryManager>,
}

impl MemoryCloseTool {
    pub fn new(memory_manager: Arc<MemoryManager>) -> Self {
        Self { memory_manager }
    }
}

#[async_trait]
impl Tool for MemoryCloseTool {
    fn name(&self) -> &str {
        "memory_close"
    }

    fn description(&self) -> &str {
        "Close a session or agent memory context."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "id": { "type": "string" },
                "target": { "type": "string", "enum": ["session", "agent"] },
                "tenantContext": { "$ref": "#/definitions/TenantContext" }
            },
            "required": ["id", "target"]
        })
    }

    async fn call(&self, params: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let p: MemoryCloseParams = serde_json::from_value(params)?;
        p.validate()?;

        let ctx = p.tenant_context.ok_or("Missing tenant context")?;

        match p.target {
            CloseTarget::Session => self.memory_manager.close_session(ctx, &p.id).await?,
            CloseTarget::Agent => self.memory_manager.close_agent(ctx, &p.id).await?,
        }

        Ok(json!({
            "success": true,
            "message": format!("{:?} context closed successfully", p.target)
        }))
    }
}

#[derive(Serialize, Deserialize, JsonSchema, Validate)]
pub struct MemoryFeedbackParams {
    #[serde(rename = "memoryId")]
    pub memory_id: String,
    pub layer: String,
    #[serde(rename = "rewardType")]
    pub reward_type: String,
    pub score: f32,
    pub reasoning: Option<String>,
    #[serde(rename = "tenantContext")]
    pub tenant_context: Option<TenantContext>,
}

pub struct MemoryFeedbackTool {
    memory_manager: Arc<MemoryManager>,
}

impl MemoryFeedbackTool {
    pub fn new(memory_manager: Arc<MemoryManager>) -> Self {
        Self { memory_manager }
    }
}

#[async_trait]
impl Tool for MemoryFeedbackTool {
    fn name(&self) -> &str {
        "memory_feedback"
    }

    fn description(&self) -> &str {
        "Submit a reward signal for a retrieved memory."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "memoryId": { "type": "string" },
                "layer": { "type": "string" },
                "rewardType": { "type": "string", "enum": ["helpful", "irrelevant", "outdated", "inaccurate", "duplicate"] },
                "score": { "type": "number", "minimum": -1.0, "maximum": 1.0 },
                "reasoning": { "type": "string" },
                "tenantContext": { "$ref": "#/definitions/TenantContext" }
            },
            "required": ["memoryId", "layer", "rewardType", "score"]
        })
    }

    async fn call(&self, params: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let p: MemoryFeedbackParams = serde_json::from_value(params)?;
        p.validate()?;

        let ctx = p.tenant_context.ok_or("Missing tenant context")?;

        let layer = match p.layer.to_lowercase().as_str() {
            "agent" => mk_core::types::MemoryLayer::Agent,
            "user" => mk_core::types::MemoryLayer::User,
            "session" => mk_core::types::MemoryLayer::Session,
            "project" => mk_core::types::MemoryLayer::Project,
            "team" => mk_core::types::MemoryLayer::Team,
            "org" => mk_core::types::MemoryLayer::Org,
            "company" => mk_core::types::MemoryLayer::Company,
            _ => return Err(format!("Unknown layer: {}", p.layer).into()),
        };

        let reward_type = match p.reward_type.to_lowercase().as_str() {
            "helpful" => mk_core::types::RewardType::Helpful,
            "irrelevant" => mk_core::types::RewardType::Irrelevant,
            "outdated" => mk_core::types::RewardType::Outdated,
            "inaccurate" => mk_core::types::RewardType::Inaccurate,
            "duplicate" => mk_core::types::RewardType::Duplicate,
            _ => return Err(format!("Unknown reward type: {}", p.reward_type).into()),
        };

        let reward = mk_core::types::RewardSignal {
            reward_type,
            score: p.score,
            reasoning: p.reasoning,
            agent_id: ctx.agent_id.clone(),
            timestamp: chrono::Utc::now().timestamp(),
        };

        self.memory_manager
            .record_reward(ctx, layer, &p.memory_id, reward)
            .await?;

        Ok(json!({ "success": true, "message": "Reward recorded successfully" }))
    }
}

#[derive(Serialize, Deserialize, JsonSchema, Validate)]
pub struct MemoryOptimizeParams {
    pub layer: String,
    #[serde(rename = "tenantContext")]
    pub tenant_context: Option<TenantContext>,
}

pub struct MemoryOptimizeTool {
    memory_manager: Arc<MemoryManager>,
}

impl MemoryOptimizeTool {
    pub fn new(memory_manager: Arc<MemoryManager>) -> Self {
        Self { memory_manager }
    }
}

#[async_trait]
impl Tool for MemoryOptimizeTool {
    fn name(&self) -> &str {
        "memory_optimize"
    }

    fn description(&self) -> &str {
        "Manually trigger a pruning/compression cycle for a specific memory layer."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "layer": { "type": "string", "enum": ["agent", "user", "session", "project", "team", "org", "company"] },
                "tenantContext": { "$ref": "#/definitions/TenantContext" }
            },
            "required": ["layer"]
        })
    }

    async fn call(&self, params: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let p: MemoryOptimizeParams = serde_json::from_value(params)?;
        p.validate()?;

        let ctx = p.tenant_context.ok_or("Missing tenant context")?;

        let layer = match p.layer.to_lowercase().as_str() {
            "agent" => mk_core::types::MemoryLayer::Agent,
            "user" => mk_core::types::MemoryLayer::User,
            "session" => mk_core::types::MemoryLayer::Session,
            "project" => mk_core::types::MemoryLayer::Project,
            "team" => mk_core::types::MemoryLayer::Team,
            "org" => mk_core::types::MemoryLayer::Org,
            "company" => mk_core::types::MemoryLayer::Company,
            _ => return Err(format!("Unknown layer: {}", p.layer).into()),
        };

        self.memory_manager.optimize_layer(ctx, layer).await?;

        Ok(json!({ "success": true, "message": "Memory optimization complete" }))
    }
}

#[derive(Serialize, Deserialize, JsonSchema, Validate)]
pub struct GraphQueryParams {
    pub query: String,
    #[serde(rename = "tenantContext")]
    pub tenant_context: Option<TenantContext>,
}

pub struct GraphQueryTool {
    memory_manager: Arc<MemoryManager>,
}

impl GraphQueryTool {
    pub fn new(memory_manager: Arc<MemoryManager>) -> Self {
        Self { memory_manager }
    }
}

#[async_trait]
impl Tool for GraphQueryTool {
    fn name(&self) -> &str {
        "graph_query"
    }

    fn description(&self) -> &str {
        "Query the knowledge graph using natural language."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string" },
                "tenantContext": { "$ref": "#/definitions/TenantContext" }
            },
            "required": ["query"]
        })
    }

    async fn call(&self, params: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let p: GraphQueryParams = serde_json::from_value(params)?;
        p.validate()?;

        let _ctx = p.tenant_context.ok_or("Missing tenant context")?;

        Ok(json!({
            "success": true,
            "message": "Graph query not yet implemented",
            "query": p.query
        }))
    }
}

#[derive(Serialize, Deserialize, JsonSchema, Validate)]
pub struct GraphNeighborsParams {
    #[serde(rename = "entityId")]
    pub entity_id: String,
    pub depth: Option<usize>,
    #[serde(rename = "tenantContext")]
    pub tenant_context: Option<TenantContext>,
}

pub struct GraphNeighborsTool {
    memory_manager: Arc<MemoryManager>,
}

impl GraphNeighborsTool {
    pub fn new(memory_manager: Arc<MemoryManager>) -> Self {
        Self { memory_manager }
    }
}

#[async_trait]
impl Tool for GraphNeighborsTool {
    fn name(&self) -> &str {
        "graph_neighbors"
    }

    fn description(&self) -> &str {
        "Find neighboring entities in the knowledge graph."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "entityId": { "type": "string" },
                "depth": { "type": "integer", "minimum": 1, "maximum": 5 },
                "tenantContext": { "$ref": "#/definitions/TenantContext" }
            },
            "required": ["entityId"]
        })
    }

    async fn call(&self, params: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let p: GraphNeighborsParams = serde_json::from_value(params)?;
        p.validate()?;

        let _ctx = p.tenant_context.ok_or("Missing tenant context")?;

        Ok(json!({
            "success": true,
            "message": "Graph neighbors not yet implemented",
            "entityId": p.entity_id,
            "depth": p.depth.unwrap_or(1)
        }))
    }
}

#[derive(Serialize, Deserialize, JsonSchema, Validate)]
pub struct GraphPathParams {
    #[serde(rename = "sourceId")]
    pub source_id: String,
    #[serde(rename = "targetId")]
    pub target_id: String,
    #[serde(rename = "maxHops")]
    pub max_hops: Option<usize>,
    #[serde(rename = "tenantContext")]
    pub tenant_context: Option<TenantContext>,
}

pub struct GraphPathTool {
    memory_manager: Arc<MemoryManager>,
}

impl GraphPathTool {
    pub fn new(memory_manager: Arc<MemoryManager>) -> Self {
        Self { memory_manager }
    }
}

#[async_trait]
impl Tool for GraphPathTool {
    fn name(&self) -> &str {
        "graph_path"
    }

    fn description(&self) -> &str {
        "Find the shortest path between two entities in the knowledge graph."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "sourceId": { "type": "string" },
                "targetId": { "type": "string" },
                "maxHops": { "type": "integer", "minimum": 1, "maximum": 10 },
                "tenantContext": { "$ref": "#/definitions/TenantContext" }
            },
            "required": ["sourceId", "targetId"]
        })
    }

    async fn call(&self, params: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let p: GraphPathParams = serde_json::from_value(params)?;
        p.validate()?;

        let _ctx = p.tenant_context.ok_or("Missing tenant context")?;

        Ok(json!({
            "success": true,
            "message": "Graph path not yet implemented",
            "sourceId": p.source_id,
            "targetId": p.target_id,
            "maxHops": p.max_hops.unwrap_or(5)
        }))
    }
}
