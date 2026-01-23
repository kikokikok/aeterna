use crate::governance::GovernanceEngine;
use crate::repository::{GitRepository, RepositoryError};
use mk_core::traits::KnowledgeRepository;
use mk_core::types::{KnowledgeEntry, KnowledgeLayer, TenantContext, ValidationResult};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum KnowledgeManagerError {
    #[error("Repository error: {0}")]
    Repository(#[from] RepositoryError),
    #[error("Governance violation: {0}")]
    Governance(String),
    #[error("Governance error: {0}")]
    GovernanceInternal(#[from] crate::governance::GovernanceError),
    #[error("Configuration error: {0}")]
    Config(String),
}

pub struct KnowledgeManager {
    repository: Arc<GitRepository>,
    governance: Arc<GovernanceEngine>,
}

impl KnowledgeManager {
    pub fn new(repository: Arc<GitRepository>, governance: Arc<GovernanceEngine>) -> Self {
        Self {
            repository,
            governance,
        }
    }

    pub async fn add(
        &self,
        ctx: TenantContext,
        entry: KnowledgeEntry,
        message: &str,
    ) -> Result<String, KnowledgeManagerError> {
        let mut context = HashMap::new();
        context.insert("path".to_string(), serde_json::json!(entry.path));
        context.insert("content".to_string(), serde_json::json!(entry.content));
        context.insert("layer".to_string(), serde_json::json!(entry.layer));

        let validation = self
            .governance
            .validate_with_context(entry.layer, &context, Some(&ctx))
            .await?;

        if !validation.is_valid {
            let errors: Vec<String> = validation
                .violations
                .iter()
                .map(|v| v.message.clone())
                .collect();
            return Err(KnowledgeManagerError::Governance(errors.join(", ")));
        }

        let commit_hash = self.repository.store(ctx, entry, message).await?;
        Ok(commit_hash)
    }

    pub async fn query(
        &self,
        ctx: TenantContext,
        query: &str,
        layers: Vec<KnowledgeLayer>,
        limit: usize,
    ) -> Result<Vec<KnowledgeEntry>, KnowledgeManagerError> {
        let results = self.repository.search(ctx, query, layers, limit).await?;
        Ok(results)
    }

    pub async fn check_constraints(
        &self,
        ctx: TenantContext,
        layer: KnowledgeLayer,
        context: HashMap<String, serde_json::Value>,
    ) -> Result<ValidationResult, KnowledgeManagerError> {
        let result = self
            .governance
            .validate_with_context(layer, &context, Some(&ctx))
            .await?;
        Ok(result)
    }

    pub async fn list(
        &self,
        ctx: TenantContext,
        layer: KnowledgeLayer,
        prefix: &str,
    ) -> Result<Vec<KnowledgeEntry>, KnowledgeManagerError> {
        let entries = self.repository.list(ctx, layer, prefix).await?;
        Ok(entries)
    }

    pub async fn get(
        &self,
        ctx: TenantContext,
        layer: KnowledgeLayer,
        path: &str,
    ) -> Result<Option<KnowledgeEntry>, KnowledgeManagerError> {
        let entry = self.repository.get(ctx, layer, path).await?;
        Ok(entry)
    }

    pub async fn get_head_commit(
        &self,
        ctx: TenantContext,
    ) -> Result<Option<String>, KnowledgeManagerError> {
        Ok(self.repository.get_head_commit(ctx).await?)
    }

    pub async fn get_affected_items(
        &self,
        ctx: TenantContext,
        since_commit: &str,
    ) -> Result<Vec<(KnowledgeLayer, String)>, KnowledgeManagerError> {
        Ok(self
            .repository
            .get_affected_items(ctx, since_commit)
            .await?)
    }

    pub fn root_path(&self) -> Option<std::path::PathBuf> {
        Some(self.repository.root_path().to_path_buf())
    }
}
