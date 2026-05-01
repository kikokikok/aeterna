use async_trait::async_trait;
use mk_core::types::TenantContext;
use serde::{Deserialize, Serialize};
use serde_json::Value;

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
    async fn soft_delete_nodes_by_source_memory_id(
        &self,
        ctx: TenantContext,
        source_memory_id: &str,
    ) -> Result<usize, Self::Error>;

    /// Append a graph mutation event to the event log.
    /// Returns the assigned sequence number.
    /// Default implementation panics — only `DuckDbGraphStore` with event sourcing implements this.
    async fn append_event(
        &self,
        _ctx: TenantContext,
        _kind: &str,
        _payload: Value,
    ) -> Result<i64, Self::Error> {
        unimplemented!("append_event is only available when event sourcing is enabled")
    }

    /// Return the last applied event sequence for a tenant on this pod.
    async fn last_applied_seq(&self, _ctx: TenantContext) -> Result<i64, Self::Error> {
        unimplemented!("last_applied_seq is only available when event sourcing is enabled")
    }
}
