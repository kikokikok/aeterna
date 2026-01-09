use crate::tools::Tool;
use async_trait::async_trait;
use knowledge::repository::GitRepository;
use mk_core::traits::KnowledgeRepository;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::Arc;
use validator::Validate;

pub struct KnowledgeGetTool {
    repo: Arc<GitRepository>
}

impl KnowledgeGetTool {
    pub fn new(repo: Arc<GitRepository>) -> Self {
        Self { repo }
    }
}

#[derive(Serialize, Deserialize, JsonSchema, Validate)]
pub struct KnowledgeGetParams {
    pub path: String,
    pub layer: String
}

#[async_trait]
impl Tool for KnowledgeGetTool {
    fn name(&self) -> &str {
        "knowledge_get"
    }

    fn description(&self) -> &str {
        "Retrieve a specific knowledge entry by path and layer."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" },
                "layer": { "type": "string" }
            },
            "required": ["path", "layer"]
        })
    }

    async fn call(&self, params: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let p: KnowledgeGetParams = serde_json::from_value(params)?;
        p.validate()?;

        let layer = match p.layer.to_lowercase().as_str() {
            "company" => mk_core::types::KnowledgeLayer::Company,
            "org" => mk_core::types::KnowledgeLayer::Org,
            "team" => mk_core::types::KnowledgeLayer::Team,
            "project" => mk_core::types::KnowledgeLayer::Project,
            _ => return Err(format!("Unknown layer: {}", p.layer).into())
        };

        let entry = self.repo.get(layer, &p.path).await?;
        Ok(json!({ "success": true, "entry": entry }))
    }
}

pub struct KnowledgeListTool {
    repo: Arc<GitRepository>
}

impl KnowledgeListTool {
    pub fn new(repo: Arc<GitRepository>) -> Self {
        Self { repo }
    }
}

#[derive(Serialize, Deserialize, JsonSchema, Validate)]
pub struct KnowledgeListParams {
    pub layer: String,
    #[serde(default)]
    pub prefix: String
}

#[async_trait]
impl Tool for KnowledgeListTool {
    fn name(&self) -> &str {
        "knowledge_list"
    }

    fn description(&self) -> &str {
        "List knowledge entries in a specific layer, optionally filtered by prefix."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "layer": { "type": "string" },
                "prefix": { "type": "string" }
            },
            "required": ["layer"]
        })
    }

    async fn call(&self, params: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let p: KnowledgeListParams = serde_json::from_value(params)?;
        p.validate()?;

        let layer = match p.layer.to_lowercase().as_str() {
            "company" => mk_core::types::KnowledgeLayer::Company,
            "org" => mk_core::types::KnowledgeLayer::Org,
            "team" => mk_core::types::KnowledgeLayer::Team,
            "project" => mk_core::types::KnowledgeLayer::Project,
            _ => return Err(format!("Unknown layer: {}", p.layer).into())
        };

        let entries = self.repo.list(layer, &p.prefix).await?;
        Ok(json!({ "success": true, "entries": entries }))
    }
}

pub struct KnowledgeQueryTool {
    repo: Arc<GitRepository>
}

impl KnowledgeQueryTool {
    pub fn new(repo: Arc<GitRepository>) -> Self {
        Self { repo }
    }
}

#[derive(Serialize, Deserialize, JsonSchema, Validate)]
pub struct KnowledgeQueryParams {
    pub query: String,
    #[serde(default)]
    pub layers: Vec<String>,
    pub limit: Option<usize>
}

#[async_trait]
impl Tool for KnowledgeQueryTool {
    fn name(&self) -> &str {
        "knowledge_query"
    }

    fn description(&self) -> &str {
        "Search for knowledge entries across layers."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string" },
                "layers": { "type": "array", "items": { "type": "string" } },
                "limit": { "type": "integer" }
            },
            "required": ["query"]
        })
    }

    async fn call(&self, params: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let p: KnowledgeQueryParams = serde_json::from_value(params)?;
        p.validate()?;

        let mut layers = Vec::new();
        if p.layers.is_empty() {
            layers = vec![
                mk_core::types::KnowledgeLayer::Company,
                mk_core::types::KnowledgeLayer::Org,
                mk_core::types::KnowledgeLayer::Team,
                mk_core::types::KnowledgeLayer::Project,
            ];
        } else {
            for l in &p.layers {
                let layer = match l.to_lowercase().as_str() {
                    "company" => mk_core::types::KnowledgeLayer::Company,
                    "org" => mk_core::types::KnowledgeLayer::Org,
                    "team" => mk_core::types::KnowledgeLayer::Team,
                    "project" => mk_core::types::KnowledgeLayer::Project,
                    _ => continue
                };
                layers.push(layer);
            }
        }

        let results = self
            .repo
            .search(&p.query, layers, p.limit.unwrap_or(10))
            .await?;
        Ok(json!({ "success": true, "results": results }))
    }
}
