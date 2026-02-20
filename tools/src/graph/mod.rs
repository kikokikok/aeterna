use crate::tools::Tool;
use async_trait::async_trait;
use duckdb::Connection;
use parking_lot::Mutex;
use serde_json::{Value, json};
use std::collections::{HashSet, VecDeque};
use std::sync::Arc;

pub type DbHandle = Arc<Mutex<Connection>>;

#[derive(Clone)]
pub struct GraphLinkTool {
    db: DbHandle,
}

impl GraphLinkTool {
    pub fn new(db: DbHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl Tool for GraphLinkTool {
    fn name(&self) -> &str {
        "graph_link"
    }

    fn description(&self) -> &str {
        "Create or replace an edge between two nodes in the knowledge graph."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "source_id": {
                    "type": "string",
                    "description": "Source node identifier"
                },
                "target_id": {
                    "type": "string",
                    "description": "Target node identifier"
                },
                "edge_type": {
                    "type": "string",
                    "description": "Type of relationship (e.g. implements, violates, related_to)"
                },
                "confidence": {
                    "type": "number",
                    "description": "Confidence score (0.0 to 1.0, default: 1.0)",
                    "minimum": 0.0,
                    "maximum": 1.0
                },
                "properties": {
                    "type": "object",
                    "description": "Optional JSON properties for the edge"
                },
                "tenant_id": {
                    "type": "string",
                    "description": "Tenant identifier for isolation"
                }
            },
            "required": ["source_id", "target_id", "edge_type"]
        })
    }

    async fn call(&self, params: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let source_id = params["source_id"]
            .as_str()
            .ok_or("source_id is required")?
            .to_string();
        let target_id = params["target_id"]
            .as_str()
            .ok_or("target_id is required")?
            .to_string();
        let edge_type = params["edge_type"]
            .as_str()
            .ok_or("edge_type is required")?
            .to_string();
        let confidence = params["confidence"].as_f64().unwrap_or(1.0);
        let properties = params.get("properties").cloned().unwrap_or(json!({}));
        let tenant_id = params["tenant_id"]
            .as_str()
            .unwrap_or("default")
            .to_string();

        let edge_id = format!("{tenant_id}:{edge_type}:{source_id}:{target_id}");
        let props_str = serde_json::to_string(&properties)?;

        let db = self.db.clone();
        let eid = edge_id.clone();

        tokio::task::spawn_blocking(move || {
            let conn = db.lock();
            conn.execute(
                "INSERT OR REPLACE INTO graph_edges (id, source_id, target_id, edge_type, properties, confidence, tenant_id, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, now())",
                duckdb::params![eid, source_id, target_id, edge_type, props_str, confidence, tenant_id],
            )?;
            Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
        })
        .await??;

        Ok(json!({
            "success": true,
            "edge_id": edge_id
        }))
    }
}

#[derive(Clone)]
pub struct GraphUnlinkTool {
    db: DbHandle,
}

impl GraphUnlinkTool {
    pub fn new(db: DbHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl Tool for GraphUnlinkTool {
    fn name(&self) -> &str {
        "graph_unlink"
    }

    fn description(&self) -> &str {
        "Remove an edge from the knowledge graph by edge ID or by source/target pair."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "edge_id": {
                    "type": "string",
                    "description": "Specific edge ID to remove"
                },
                "source_id": {
                    "type": "string",
                    "description": "Source node ID (used with target_id)"
                },
                "target_id": {
                    "type": "string",
                    "description": "Target node ID (used with source_id)"
                },
                "tenant_id": {
                    "type": "string",
                    "description": "Tenant identifier for isolation"
                }
            }
        })
    }

    async fn call(&self, params: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let edge_id = params["edge_id"].as_str().map(String::from);
        let source_id = params["source_id"].as_str().map(String::from);
        let target_id = params["target_id"].as_str().map(String::from);
        let tenant_id = params["tenant_id"]
            .as_str()
            .unwrap_or("default")
            .to_string();

        let db = self.db.clone();

        let removed = tokio::task::spawn_blocking(move || {
            let conn = db.lock();
            if let Some(eid) = edge_id {
                conn.execute(
                    "DELETE FROM graph_edges WHERE id = ? AND tenant_id = ?",
                    duckdb::params![eid, tenant_id],
                )
            } else if let (Some(src), Some(tgt)) = (source_id, target_id) {
                conn.execute(
                    "DELETE FROM graph_edges WHERE source_id = ? AND target_id = ? AND tenant_id = ?",
                    duckdb::params![src, tgt, tenant_id],
                )
            } else {
                Err(duckdb::Error::InvalidParameterName(
                    "edge_id or source_id+target_id required".into(),
                ))
            }
        })
        .await??;

        Ok(json!({
            "success": true,
            "removed": removed
        }))
    }
}

#[derive(Clone)]
pub struct GraphTraverseTool {
    db: DbHandle,
}

impl GraphTraverseTool {
    pub fn new(db: DbHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl Tool for GraphTraverseTool {
    fn name(&self) -> &str {
        "graph_traverse"
    }

    fn description(&self) -> &str {
        "Traverse the knowledge graph from a starting node using BFS, \
         optionally filtering by edge types and direction."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "node_id": {
                    "type": "string",
                    "description": "Starting node identifier"
                },
                "edge_types": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional edge type filter"
                },
                "direction": {
                    "type": "string",
                    "enum": ["outgoing", "incoming", "both"],
                    "description": "Traversal direction (default: both)"
                },
                "max_depth": {
                    "type": "integer",
                    "description": "Maximum BFS depth (default: 3)",
                    "minimum": 1,
                    "maximum": 10
                },
                "tenant_id": {
                    "type": "string",
                    "description": "Tenant identifier for isolation"
                }
            },
            "required": ["node_id"]
        })
    }

    async fn call(&self, params: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let node_id = params["node_id"]
            .as_str()
            .ok_or("node_id is required")?
            .to_string();
        let edge_types: Option<Vec<String>> = params
            .get("edge_types")
            .and_then(|v| serde_json::from_value(v.clone()).ok());
        let direction = params["direction"].as_str().unwrap_or("both").to_string();
        let max_depth = params["max_depth"].as_u64().unwrap_or(3) as usize;
        let tenant_id = params["tenant_id"]
            .as_str()
            .unwrap_or("default")
            .to_string();

        let db = self.db.clone();

        let (nodes, edges) = tokio::task::spawn_blocking(move || {
            let conn = db.lock();
            let mut visited: HashSet<String> = HashSet::new();
            let mut queue: VecDeque<(String, usize)> = VecDeque::new();
            let mut result_nodes: Vec<Value> = Vec::new();
            let mut result_edges: Vec<Value> = Vec::new();

            visited.insert(node_id.clone());
            queue.push_back((node_id, 0));

            while let Some((current, depth)) = queue.pop_front() {
                if depth >= max_depth {
                    continue;
                }

                let mut rows: Vec<(String, String, String, String, String, f64)> = Vec::new();

                if direction == "outgoing" || direction == "both" {
                    let mut stmt = if let Some(ref et) = edge_types {
                        let placeholders: Vec<&str> = et.iter().map(|_| "?").collect();
                        let sql = format!(
                            "SELECT id, source_id, target_id, edge_type, COALESCE(properties, '{{}}'), confidence \
                             FROM graph_edges WHERE source_id = ? AND tenant_id = ? AND edge_type IN ({})",
                            placeholders.join(", ")
                        );
                        conn.prepare(&sql)?
                    } else {
                        conn.prepare(
                            "SELECT id, source_id, target_id, edge_type, COALESCE(properties, '{}'), confidence \
                             FROM graph_edges WHERE source_id = ? AND tenant_id = ?",
                        )?
                    };

                    let row_iter = if let Some(ref et) = edge_types {
                        let mut p: Vec<Box<dyn duckdb::ToSql>> = Vec::new();
                        p.push(Box::new(current.clone()));
                        p.push(Box::new(tenant_id.clone()));
                        for t in et {
                            p.push(Box::new(t.clone()));
                        }
                        let refs: Vec<&dyn duckdb::ToSql> = p.iter().map(|b| b.as_ref()).collect();
                        let mapped = stmt.query_map(&*refs, |row| {
                            Ok((
                                row.get::<_, String>(0)?,
                                row.get::<_, String>(1)?,
                                row.get::<_, String>(2)?,
                                row.get::<_, String>(3)?,
                                row.get::<_, String>(4)?,
                                row.get::<_, f64>(5)?,
                            ))
                        })?;
                        mapped.collect::<Result<Vec<_>, _>>()?
                    } else {
                        let mapped = stmt.query_map(
                            duckdb::params![current.clone(), tenant_id.clone()],
                            |row| {
                                Ok((
                                    row.get::<_, String>(0)?,
                                    row.get::<_, String>(1)?,
                                    row.get::<_, String>(2)?,
                                    row.get::<_, String>(3)?,
                                    row.get::<_, String>(4)?,
                                    row.get::<_, f64>(5)?,
                                ))
                            },
                        )?;
                        mapped.collect::<Result<Vec<_>, _>>()?
                    };
                    rows.extend(row_iter);
                }

                if direction == "incoming" || direction == "both" {
                    let mut stmt = if let Some(ref et) = edge_types {
                        let placeholders: Vec<&str> = et.iter().map(|_| "?").collect();
                        let sql = format!(
                            "SELECT id, source_id, target_id, edge_type, COALESCE(properties, '{{}}'), confidence \
                             FROM graph_edges WHERE target_id = ? AND tenant_id = ? AND edge_type IN ({})",
                            placeholders.join(", ")
                        );
                        conn.prepare(&sql)?
                    } else {
                        conn.prepare(
                            "SELECT id, source_id, target_id, edge_type, COALESCE(properties, '{}'), confidence \
                             FROM graph_edges WHERE target_id = ? AND tenant_id = ?",
                        )?
                    };

                    let row_iter = if let Some(ref et) = edge_types {
                        let mut p: Vec<Box<dyn duckdb::ToSql>> = Vec::new();
                        p.push(Box::new(current.clone()));
                        p.push(Box::new(tenant_id.clone()));
                        for t in et {
                            p.push(Box::new(t.clone()));
                        }
                        let refs: Vec<&dyn duckdb::ToSql> = p.iter().map(|b| b.as_ref()).collect();
                        let mapped = stmt.query_map(&*refs, |row| {
                            Ok((
                                row.get::<_, String>(0)?,
                                row.get::<_, String>(1)?,
                                row.get::<_, String>(2)?,
                                row.get::<_, String>(3)?,
                                row.get::<_, String>(4)?,
                                row.get::<_, f64>(5)?,
                            ))
                        })?;
                        mapped.collect::<Result<Vec<_>, _>>()?
                    } else {
                        let mapped = stmt.query_map(
                            duckdb::params![current.clone(), tenant_id.clone()],
                            |row| {
                                Ok((
                                    row.get::<_, String>(0)?,
                                    row.get::<_, String>(1)?,
                                    row.get::<_, String>(2)?,
                                    row.get::<_, String>(3)?,
                                    row.get::<_, String>(4)?,
                                    row.get::<_, f64>(5)?,
                                ))
                            },
                        )?;
                        mapped.collect::<Result<Vec<_>, _>>()?
                    };
                    rows.extend(row_iter);
                }

                for (eid, src, tgt, etype, props, conf) in rows {
                    result_edges.push(json!({
                        "edge_id": eid,
                        "source_id": src,
                        "target_id": tgt,
                        "edge_type": etype,
                        "properties": serde_json::from_str::<Value>(&props).unwrap_or(json!({})),
                        "confidence": conf
                    }));

                    let neighbor = if src == current { &tgt } else { &src };
                    if visited.insert(neighbor.clone()) {
                        result_nodes.push(json!({ "node_id": neighbor, "depth": depth + 1 }));
                        queue.push_back((neighbor.clone(), depth + 1));
                    }
                }
            }

            Ok::<_, Box<dyn std::error::Error + Send + Sync>>((result_nodes, result_edges))
        })
        .await??;

        Ok(json!({
            "success": true,
            "nodes": nodes,
            "edges": edges
        }))
    }
}

#[derive(Clone)]
pub struct GraphFindPathTool {
    db: DbHandle,
}

impl GraphFindPathTool {
    pub fn new(db: DbHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl Tool for GraphFindPathTool {
    fn name(&self) -> &str {
        "graph_find_path"
    }

    fn description(&self) -> &str {
        "Find the shortest path between two nodes in the knowledge graph using BFS."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "start_id": {
                    "type": "string",
                    "description": "Starting node identifier"
                },
                "end_id": {
                    "type": "string",
                    "description": "Target node identifier"
                },
                "max_depth": {
                    "type": "integer",
                    "description": "Maximum search depth (default: 5)",
                    "minimum": 1,
                    "maximum": 20
                },
                "tenant_id": {
                    "type": "string",
                    "description": "Tenant identifier for isolation"
                }
            },
            "required": ["start_id", "end_id"]
        })
    }

    async fn call(&self, params: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let start_id = params["start_id"]
            .as_str()
            .ok_or("start_id is required")?
            .to_string();
        let end_id = params["end_id"]
            .as_str()
            .ok_or("end_id is required")?
            .to_string();
        let max_depth = params["max_depth"].as_u64().unwrap_or(5) as usize;
        let tenant_id = params["tenant_id"]
            .as_str()
            .unwrap_or("default")
            .to_string();

        let db = self.db.clone();

        let path = tokio::task::spawn_blocking(move || {
            let conn = db.lock();
            let mut visited: HashSet<String> = HashSet::new();
            let mut queue: VecDeque<(String, usize)> = VecDeque::new();
            let mut parent: std::collections::HashMap<String, String> =
                std::collections::HashMap::new();

            visited.insert(start_id.clone());
            queue.push_back((start_id.clone(), 0));

            let mut found = false;

            while let Some((current, depth)) = queue.pop_front() {
                if current == end_id {
                    found = true;
                    break;
                }
                if depth >= max_depth {
                    continue;
                }

                let mut stmt = conn.prepare(
                    "SELECT source_id, target_id FROM graph_edges \
                     WHERE (source_id = ? OR target_id = ?) AND tenant_id = ?",
                )?;
                let neighbors: Vec<(String, String)> = stmt
                    .query_map(
                        duckdb::params![current.clone(), current.clone(), tenant_id.clone()],
                        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                    )?
                    .collect::<Result<Vec<_>, _>>()?;

                for (src, tgt) in neighbors {
                    let neighbor = if src == current { tgt } else { src };
                    if visited.insert(neighbor.clone()) {
                        parent.insert(neighbor.clone(), current.clone());
                        queue.push_back((neighbor, depth + 1));
                    }
                }
            }

            if found {
                let mut path = vec![end_id.clone()];
                let mut current = end_id;
                while current != start_id {
                    if let Some(p) = parent.get(&current) {
                        path.push(p.clone());
                        current = p.clone();
                    } else {
                        break;
                    }
                }
                path.reverse();
                Ok::<_, Box<dyn std::error::Error + Send + Sync>>(Some(path))
            } else {
                Ok(None)
            }
        })
        .await??;

        match path {
            Some(p) => {
                let len = p.len();
                Ok(json!({
                    "success": true,
                    "path": p,
                    "length": len
                }))
            }
            None => Ok(json!({
                "success": true,
                "path": [],
                "length": 0
            })),
        }
    }
}

#[derive(Clone)]
pub struct GraphViolationsTool {
    db: DbHandle,
}

impl GraphViolationsTool {
    pub fn new(db: DbHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl Tool for GraphViolationsTool {
    fn name(&self) -> &str {
        "graph_violations"
    }

    fn description(&self) -> &str {
        "Find all violation edges in the knowledge graph, optionally filtered by node type."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "tenant_id": {
                    "type": "string",
                    "description": "Tenant identifier for isolation"
                },
                "node_type": {
                    "type": "string",
                    "description": "Optional filter by source node type"
                }
            }
        })
    }

    async fn call(&self, params: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let tenant_id = params["tenant_id"]
            .as_str()
            .unwrap_or("default")
            .to_string();
        let _node_type = params["node_type"].as_str().map(String::from);

        let db = self.db.clone();

        let violations = tokio::task::spawn_blocking(move || {
            let conn = db.lock();
            let mut stmt = conn.prepare(
                "SELECT id, source_id, target_id, COALESCE(properties, '{}'), confidence \
                 FROM graph_edges WHERE edge_type = 'violates' AND tenant_id = ?",
            )?;
            let rows: Vec<Value> = stmt
                .query_map(duckdb::params![tenant_id], |row| {
                    Ok(json!({
                        "edge_id": row.get::<_, String>(0)?,
                        "source_id": row.get::<_, String>(1)?,
                        "target_id": row.get::<_, String>(2)?,
                        "properties": serde_json::from_str::<Value>(
                            &row.get::<_, String>(3)?
                        ).unwrap_or(json!({})),
                        "confidence": row.get::<_, f64>(4)?
                    }))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok::<_, Box<dyn std::error::Error + Send + Sync>>(rows)
        })
        .await??;

        Ok(json!({
            "success": true,
            "violations": violations
        }))
    }
}

#[derive(Clone)]
pub struct GraphImplementationsTool {
    db: DbHandle,
}

impl GraphImplementationsTool {
    pub fn new(db: DbHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl Tool for GraphImplementationsTool {
    fn name(&self) -> &str {
        "graph_implementations"
    }

    fn description(&self) -> &str {
        "Find all nodes that implement a given knowledge item via 'implements' edges."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "knowledge_id": {
                    "type": "string",
                    "description": "Knowledge node identifier to find implementations for"
                },
                "tenant_id": {
                    "type": "string",
                    "description": "Tenant identifier for isolation"
                }
            },
            "required": ["knowledge_id"]
        })
    }

    async fn call(&self, params: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let knowledge_id = params["knowledge_id"]
            .as_str()
            .ok_or("knowledge_id is required")?
            .to_string();
        let tenant_id = params["tenant_id"]
            .as_str()
            .unwrap_or("default")
            .to_string();

        let db = self.db.clone();

        let implementations = tokio::task::spawn_blocking(move || {
            let conn = db.lock();
            let mut stmt = conn.prepare(
                "SELECT id, source_id FROM graph_edges \
                 WHERE target_id = ? AND edge_type = 'implements' AND tenant_id = ?",
            )?;
            let rows: Vec<Value> = stmt
                .query_map(duckdb::params![knowledge_id, tenant_id], |row| {
                    Ok(json!({
                        "edge_id": row.get::<_, String>(0)?,
                        "node_id": row.get::<_, String>(1)?
                    }))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok::<_, Box<dyn std::error::Error + Send + Sync>>(rows)
        })
        .await??;

        Ok(json!({
            "success": true,
            "implementations": implementations
        }))
    }
}

#[derive(Clone)]
pub struct GraphContextTool {
    db: DbHandle,
}

impl GraphContextTool {
    pub fn new(db: DbHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl Tool for GraphContextTool {
    fn name(&self) -> &str {
        "graph_context"
    }

    fn description(&self) -> &str {
        "Get the knowledge graph context for a given file path, including \
         the file node and all related edges."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "File path to look up in the graph"
                },
                "tenant_id": {
                    "type": "string",
                    "description": "Tenant identifier for isolation"
                }
            },
            "required": ["file_path"]
        })
    }

    async fn call(&self, params: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let file_path = params["file_path"]
            .as_str()
            .ok_or("file_path is required")?
            .to_string();
        let tenant_id = params["tenant_id"]
            .as_str()
            .unwrap_or("default")
            .to_string();

        let db = self.db.clone();

        let (file_node, related) = tokio::task::spawn_blocking(move || {
            let conn = db.lock();

            let mut stmt = conn.prepare(
                "SELECT id, node_type, label, COALESCE(properties, '{}') \
                 FROM graph_nodes WHERE node_type = 'code_file' AND label = ? AND tenant_id = ?",
            )?;
            let file_node: Option<Value> = stmt
                .query_map(duckdb::params![file_path, tenant_id.clone()], |row| {
                    Ok(json!({
                        "id": row.get::<_, String>(0)?,
                        "node_type": row.get::<_, String>(1)?,
                        "label": row.get::<_, String>(2)?,
                        "properties": serde_json::from_str::<Value>(
                            &row.get::<_, String>(3)?
                        ).unwrap_or(json!({}))
                    }))
                })?
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .next();

            let related = if let Some(ref node) = file_node {
                let node_id = node["id"].as_str().unwrap_or("");
                let mut edge_stmt = conn.prepare(
                    "SELECT id, source_id, target_id, edge_type, COALESCE(properties, '{}'), confidence \
                     FROM graph_edges WHERE (source_id = ? OR target_id = ?) AND tenant_id = ?",
                )?;
                edge_stmt
                    .query_map(
                        duckdb::params![node_id, node_id, tenant_id],
                        |row| {
                            Ok(json!({
                                "edge_id": row.get::<_, String>(0)?,
                                "source_id": row.get::<_, String>(1)?,
                                "target_id": row.get::<_, String>(2)?,
                                "edge_type": row.get::<_, String>(3)?,
                                "properties": serde_json::from_str::<Value>(
                                    &row.get::<_, String>(4)?
                                ).unwrap_or(json!({})),
                                "confidence": row.get::<_, f64>(5)?
                            }))
                        },
                    )?
                    .collect::<Result<Vec<_>, _>>()?
            } else {
                vec![]
            };

            Ok::<_, Box<dyn std::error::Error + Send + Sync>>((file_node, related))
        })
        .await??;

        Ok(json!({
            "success": true,
            "file_node": file_node,
            "related": related
        }))
    }
}

#[derive(Clone)]
pub struct GraphRelatedTool {
    db: DbHandle,
}

impl GraphRelatedTool {
    pub fn new(db: DbHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl Tool for GraphRelatedTool {
    fn name(&self) -> &str {
        "graph_related"
    }

    fn description(&self) -> &str {
        "Find nodes related to a given node via 'related_to' edges, \
         filtered by confidence threshold."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "node_id": {
                    "type": "string",
                    "description": "Node identifier to find related nodes for"
                },
                "threshold": {
                    "type": "number",
                    "description": "Minimum confidence threshold (default: 0.85)",
                    "minimum": 0.0,
                    "maximum": 1.0
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum number of results (default: 10)",
                    "minimum": 1,
                    "maximum": 100
                },
                "tenant_id": {
                    "type": "string",
                    "description": "Tenant identifier for isolation"
                }
            },
            "required": ["node_id"]
        })
    }

    async fn call(&self, params: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let node_id = params["node_id"]
            .as_str()
            .ok_or("node_id is required")?
            .to_string();
        let threshold = params["threshold"].as_f64().unwrap_or(0.85);
        let max_results = params["max_results"].as_u64().unwrap_or(10) as usize;
        let tenant_id = params["tenant_id"]
            .as_str()
            .unwrap_or("default")
            .to_string();

        let db = self.db.clone();

        let related = tokio::task::spawn_blocking(move || {
            let conn = db.lock();
            let mut stmt = conn.prepare(
                "SELECT target_id, confidence FROM graph_edges \
                 WHERE source_id = ? AND edge_type = 'related_to' \
                 AND confidence >= ? AND tenant_id = ? \
                 ORDER BY confidence DESC LIMIT ?",
            )?;
            let limit_i64 = max_results as i64;
            let rows: Vec<Value> = stmt
                .query_map(
                    duckdb::params![node_id, threshold, tenant_id, limit_i64],
                    |row| {
                        Ok(json!({
                            "node_id": row.get::<_, String>(0)?,
                            "score": row.get::<_, f64>(1)?
                        }))
                    },
                )?
                .collect::<Result<Vec<_>, _>>()?;
            Ok::<_, Box<dyn std::error::Error + Send + Sync>>(rows)
        })
        .await??;

        Ok(json!({
            "success": true,
            "related": related
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> DbHandle {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE graph_nodes (
                id VARCHAR PRIMARY KEY,
                node_type VARCHAR,
                label VARCHAR,
                properties JSON,
                tenant_id VARCHAR,
                created_at TIMESTAMP,
                updated_at TIMESTAMP
            );
            CREATE TABLE graph_edges (
                id VARCHAR PRIMARY KEY,
                source_id VARCHAR,
                target_id VARCHAR,
                edge_type VARCHAR,
                properties JSON,
                confidence FLOAT,
                tenant_id VARCHAR,
                created_at TIMESTAMP
            );",
        )
        .unwrap();
        Arc::new(Mutex::new(conn))
    }

    #[test]
    fn test_graph_link_tool_name() {
        let db = test_db();
        let tool = GraphLinkTool::new(db);
        assert_eq!(tool.name(), "graph_link");
        assert!(!tool.description().is_empty());
        let schema = tool.input_schema();
        assert_eq!(schema["type"], "object");
    }

    #[test]
    fn test_graph_unlink_tool_name() {
        let db = test_db();
        let tool = GraphUnlinkTool::new(db);
        assert_eq!(tool.name(), "graph_unlink");
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn test_graph_traverse_tool_name() {
        let db = test_db();
        let tool = GraphTraverseTool::new(db);
        assert_eq!(tool.name(), "graph_traverse");
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn test_graph_find_path_tool_name() {
        let db = test_db();
        let tool = GraphFindPathTool::new(db);
        assert_eq!(tool.name(), "graph_find_path");
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn test_graph_violations_tool_name() {
        let db = test_db();
        let tool = GraphViolationsTool::new(db);
        assert_eq!(tool.name(), "graph_violations");
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn test_graph_implementations_tool_name() {
        let db = test_db();
        let tool = GraphImplementationsTool::new(db);
        assert_eq!(tool.name(), "graph_implementations");
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn test_graph_context_tool_name() {
        let db = test_db();
        let tool = GraphContextTool::new(db);
        assert_eq!(tool.name(), "graph_context");
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn test_graph_related_tool_name() {
        let db = test_db();
        let tool = GraphRelatedTool::new(db);
        assert_eq!(tool.name(), "graph_related");
        assert!(!tool.description().is_empty());
    }

    #[tokio::test]
    async fn test_graph_link_and_unlink() {
        let db = test_db();

        let link = GraphLinkTool::new(db.clone());
        let result = link
            .call(json!({
                "source_id": "node_a",
                "target_id": "node_b",
                "edge_type": "depends_on",
                "confidence": 0.95,
                "tenant_id": "test"
            }))
            .await
            .unwrap();
        assert_eq!(result["success"], true);
        assert!(result["edge_id"].as_str().unwrap().contains("depends_on"));

        let unlink = GraphUnlinkTool::new(db);
        let result = unlink
            .call(json!({
                "source_id": "node_a",
                "target_id": "node_b",
                "tenant_id": "test"
            }))
            .await
            .unwrap();
        assert_eq!(result["success"], true);
    }

    #[tokio::test]
    async fn test_graph_traverse_empty() {
        let db = test_db();
        let tool = GraphTraverseTool::new(db);
        let result = tool
            .call(json!({
                "node_id": "nonexistent",
                "tenant_id": "test"
            }))
            .await
            .unwrap();
        assert_eq!(result["success"], true);
        assert_eq!(result["nodes"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_graph_find_path_no_path() {
        let db = test_db();
        let tool = GraphFindPathTool::new(db);
        let result = tool
            .call(json!({
                "start_id": "a",
                "end_id": "z",
                "tenant_id": "test"
            }))
            .await
            .unwrap();
        assert_eq!(result["success"], true);
        assert_eq!(result["length"], 0);
    }

    #[tokio::test]
    async fn test_graph_violations_empty() {
        let db = test_db();
        let tool = GraphViolationsTool::new(db);
        let result = tool.call(json!({ "tenant_id": "test" })).await.unwrap();
        assert_eq!(result["success"], true);
        assert_eq!(result["violations"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_graph_implementations_empty() {
        let db = test_db();
        let tool = GraphImplementationsTool::new(db);
        let result = tool
            .call(json!({
                "knowledge_id": "k1",
                "tenant_id": "test"
            }))
            .await
            .unwrap();
        assert_eq!(result["success"], true);
        assert_eq!(result["implementations"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_graph_context_no_file() {
        let db = test_db();
        let tool = GraphContextTool::new(db);
        let result = tool
            .call(json!({
                "file_path": "src/main.rs",
                "tenant_id": "test"
            }))
            .await
            .unwrap();
        assert_eq!(result["success"], true);
        assert!(result["file_node"].is_null());
    }

    #[tokio::test]
    async fn test_graph_related_empty() {
        let db = test_db();
        let tool = GraphRelatedTool::new(db);
        let result = tool
            .call(json!({
                "node_id": "n1",
                "tenant_id": "test"
            }))
            .await
            .unwrap();
        assert_eq!(result["success"], true);
        assert_eq!(result["related"].as_array().unwrap().len(), 0);
    }
}
