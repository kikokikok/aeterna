use crate::tools::Tool;
use async_trait::async_trait;
use mk_core::traits::KnowledgeRepository;
use mk_core::types::KnowledgeLayer;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;
use validator::Validate;

pub struct KnowledgeQueryTool {
    repository: Arc<dyn KnowledgeRepository<Error = knowledge::repository::RepositoryError>>
}

#[derive(Deserialize, JsonSchema, Validate)]
pub struct KnowledgeQueryArgs {
    pub layer: KnowledgeLayer,
    #[serde(default)]
    pub prefix: String
}

impl KnowledgeQueryTool {
    pub fn new(
        repository: Arc<dyn KnowledgeRepository<Error = knowledge::repository::RepositoryError>>
    ) -> Self {
        Self { repository }
    }
}

#[async_trait]
impl Tool for KnowledgeQueryTool {
    fn name(&self) -> &str {
        "knowledge_query"
    }

    fn description(&self) -> &str {
        "Search knowledge entries by layer and path prefix"
    }

    fn input_schema(&self) -> Value {
        let schema = schemars::schema_for!(KnowledgeQueryArgs);
        serde_json::to_value(schema).unwrap()
    }

    async fn call(&self, args: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let args: KnowledgeQueryArgs = serde_json::from_value(args)?;
        args.validate()?;
        let entries = self.repository.list(args.layer, &args.prefix).await?;

        Ok(json!({
            "success": true,
            "totalCount": entries.len(),
            "results": entries
        }))
    }
}

pub struct KnowledgeShowTool {
    repository: Arc<dyn KnowledgeRepository<Error = knowledge::repository::RepositoryError>>
}

#[derive(Deserialize, JsonSchema, Validate)]
pub struct KnowledgeShowArgs {
    pub layer: KnowledgeLayer,
    pub path: String
}

impl KnowledgeShowTool {
    pub fn new(
        repository: Arc<dyn KnowledgeRepository<Error = knowledge::repository::RepositoryError>>
    ) -> Self {
        Self { repository }
    }
}

#[async_trait]
impl Tool for KnowledgeShowTool {
    fn name(&self) -> &str {
        "knowledge_show"
    }

    fn description(&self) -> &str {
        "Retrieve full content and metadata for a specific knowledge entry"
    }

    fn input_schema(&self) -> Value {
        let schema = schemars::schema_for!(KnowledgeShowArgs);
        serde_json::to_value(schema).unwrap()
    }

    async fn call(&self, args: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let args: KnowledgeShowArgs = serde_json::from_value(args)?;
        args.validate()?;
        let entry = self.repository.get(args.layer, &args.path).await?;

        match entry {
            Some(e) => Ok(json!({
                "success": true,
                "entry": e
            })),
            None => Ok(json!({
                "success": false,
                "error": "Entry not found"
            }))
        }
    }
}

pub struct KnowledgeCheckTool;

#[derive(Deserialize, JsonSchema, Validate)]
pub struct KnowledgeCheckArgs {
    pub content: String,
    #[serde(default)]
    pub context: std::collections::HashMap<String, String>
}

impl KnowledgeCheckTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for KnowledgeCheckTool {
    fn name(&self) -> &str {
        "knowledge_check"
    }

    fn description(&self) -> &str {
        "Check content against organizational policies and knowledge constraints (Stub)"
    }

    fn input_schema(&self) -> Value {
        let schema = schemars::schema_for!(KnowledgeCheckArgs);
        serde_json::to_value(schema).unwrap()
    }

    async fn call(&self, args: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let args: KnowledgeCheckArgs = serde_json::from_value(args)?;
        args.validate()?;
        Ok(json!({
            "success": true,
            "isValid": true,
            "violations": []
        }))
    }
}
