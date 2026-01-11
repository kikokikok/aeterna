//! Core traits for memory-knowledge system

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Storage backend trait for extensible storage implementations
#[async_trait]
pub trait StorageBackend: Send + Sync {
    type Error;

    async fn store(
        &self,
        ctx: crate::types::TenantContext,
        key: &str,
        value: &[u8]
    ) -> Result<(), Self::Error>;

    async fn retrieve(
        &self,
        ctx: crate::types::TenantContext,
        key: &str
    ) -> Result<Option<Vec<u8>>, Self::Error>;

    async fn delete(&self, ctx: crate::types::TenantContext, key: &str) -> Result<(), Self::Error>;

    async fn exists(
        &self,
        ctx: crate::types::TenantContext,
        key: &str
    ) -> Result<bool, Self::Error>;
}

/// Health check capability for service monitoring
pub trait HealthCheck: Send + Sync {
    fn health_check(&self) -> Result<HealthStatus, Box<dyn std::error::Error + Send + Sync>>;
}

/// Health status for service monitoring
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy
}

#[async_trait]
pub trait MemoryProviderAdapter: Send + Sync {
    type Error;

    async fn add(
        &self,
        ctx: crate::types::TenantContext,
        entry: crate::types::MemoryEntry
    ) -> Result<String, Self::Error>;

    async fn search(
        &self,
        ctx: crate::types::TenantContext,
        query_vector: Vec<f32>,
        limit: usize,
        filters: std::collections::HashMap<String, serde_json::Value>
    ) -> Result<Vec<crate::types::MemoryEntry>, Self::Error>;

    async fn get(
        &self,
        ctx: crate::types::TenantContext,
        id: &str
    ) -> Result<Option<crate::types::MemoryEntry>, Self::Error>;

    async fn update(
        &self,
        ctx: crate::types::TenantContext,
        entry: crate::types::MemoryEntry
    ) -> Result<(), Self::Error>;

    async fn delete(&self, ctx: crate::types::TenantContext, id: &str) -> Result<(), Self::Error>;

    async fn list(
        &self,
        ctx: crate::types::TenantContext,
        layer: crate::types::MemoryLayer,
        limit: usize,
        cursor: Option<String>
    ) -> Result<(Vec<crate::types::MemoryEntry>, Option<String>), Self::Error>;
}

#[async_trait]
pub trait KnowledgeRepository: Send + Sync {
    type Error;

    async fn get(
        &self,
        ctx: crate::types::TenantContext,
        layer: crate::types::KnowledgeLayer,
        path: &str
    ) -> Result<Option<crate::types::KnowledgeEntry>, Self::Error>;

    async fn store(
        &self,
        ctx: crate::types::TenantContext,
        entry: crate::types::KnowledgeEntry,
        message: &str
    ) -> Result<String, Self::Error>;

    async fn list(
        &self,
        ctx: crate::types::TenantContext,
        layer: crate::types::KnowledgeLayer,
        prefix: &str
    ) -> Result<Vec<crate::types::KnowledgeEntry>, Self::Error>;

    async fn delete(
        &self,
        ctx: crate::types::TenantContext,
        layer: crate::types::KnowledgeLayer,
        path: &str,
        message: &str
    ) -> Result<String, Self::Error>;

    async fn get_head_commit(
        &self,
        ctx: crate::types::TenantContext
    ) -> Result<Option<String>, Self::Error>;

    async fn get_affected_items(
        &self,
        ctx: crate::types::TenantContext,
        since_commit: &str
    ) -> Result<Vec<(crate::types::KnowledgeLayer, String)>, Self::Error>;

    async fn search(
        &self,
        ctx: crate::types::TenantContext,
        query: &str,
        layers: Vec<crate::types::KnowledgeLayer>,
        limit: usize
    ) -> Result<Vec<crate::types::KnowledgeEntry>, Self::Error>;

    fn root_path(&self) -> Option<std::path::PathBuf>;
}

#[async_trait]
pub trait ContextHooks: Send + Sync {
    async fn on_session_start(
        &self,
        ctx: crate::types::TenantContext,
        session_id: &str
    ) -> anyhow::Result<()>;
    async fn on_session_end(
        &self,
        ctx: crate::types::TenantContext,
        session_id: &str
    ) -> anyhow::Result<()>;
    async fn on_message(
        &self,
        ctx: crate::types::TenantContext,
        session_id: &str,
        message: &str
    ) -> anyhow::Result<()>;
    async fn on_tool_use(
        &self,
        ctx: crate::types::TenantContext,
        session_id: &str,
        tool_name: &str,
        params: serde_json::Value
    ) -> anyhow::Result<()>;
}

#[async_trait]
pub trait EmbeddingService: Send + Sync {
    type Error;

    async fn embed(&self, text: &str) -> Result<Vec<f32>, Self::Error>;

    fn dimension(&self) -> usize;

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, Self::Error> {
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            results.push(self.embed(text).await?);
        }
        Ok(results)
    }
}
