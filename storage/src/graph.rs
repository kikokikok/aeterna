use async_trait::async_trait;
use mk_core::types::TenantContext;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    pub properties: serde_json::Value,
    pub tenant_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub id: String,
    pub source_id: String,
    pub target_id: String,
    pub relation: String,
    pub properties: serde_json::Value,
    pub tenant_id: String,
}

#[async_trait]
pub trait GraphStore: Send + Sync {
    type Error;

    async fn add_node(&self, ctx: TenantContext, node: GraphNode) -> Result<(), Self::Error>;
    async fn add_edge(&self, ctx: TenantContext, edge: GraphEdge) -> Result<(), Self::Error>;
    async fn get_neighbors(
        &self,
        ctx: TenantContext,
        node_id: &str,
    ) -> Result<Vec<(GraphEdge, GraphNode)>, Self::Error>;
    async fn find_path(
        &self,
        ctx: TenantContext,
        start_id: &str,
        end_id: &str,
        max_depth: usize,
    ) -> Result<Vec<GraphEdge>, Self::Error>;
    async fn search_nodes(
        &self,
        ctx: TenantContext,
        query: &str,
        limit: usize,
    ) -> Result<Vec<GraphNode>, Self::Error>;
}
