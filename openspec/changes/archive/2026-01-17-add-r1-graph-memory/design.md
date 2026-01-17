# Design: Dynamic Knowledge Graph with DuckDB + DuckPGQ

## Context

Aeterna needs a graph layer for memory relationship traversal. The original proposal suggested PostgreSQL/Apache Age, but this requires managing a separate database server.

**Constraints:**
- Must run on AWS without managing database servers
- Should support S3 as storage backend for cost efficiency
- Zero idle cost preferred (serverless-friendly)
- Memory graphs are typically small-medium per tenant (< 1M nodes)
- Primary operations: 1-3 hop traversals, shortest path, community detection

## Goals / Non-Goals

### Goals
- Embed graph database within Aeterna (no external server)
- Support S3/Parquet as durable storage
- Use SQL/PGQ standard for portable graph queries
- Enable efficient neighbor traversal and shortest path
- Integrate with existing `MemoryManager` and `MemoryR1Trainer`

### Non-Goals
- Real-time concurrent graph writes at scale (not a social network)
- ACID transactions across distributed nodes
- Supporting graphs with billions of edges (use Neptune for that)

## Decision: DuckDB + DuckPGQ

### Why DuckDB

| Criterion | DuckDB | PostgreSQL + Age | Neptune |
|-----------|--------|------------------|---------|
| **Server required** | No (embedded) | Yes | Yes (managed) |
| **Idle cost** | $0 | ~$15/mo | ~$43/mo |
| **S3 native** | Yes (Parquet) | No | Partial |
| **Rust crate** | `duckdb-rs` | `tokio-postgres` | HTTP API |
| **Graph syntax** | SQL/PGQ (standard) | Cypher (proprietary) | Gremlin/SPARQL |
| **Deployment** | Single binary | Container + volume | VPC + NAT |

### Why DuckPGQ

DuckPGQ is a community extension that implements SQL/PGQ (SQL:2023 standard):

- **Pattern matching**: `MATCH (a)-[e]->(b)` syntax
- **Shortest path**: `SHORTEST path_var` built-in
- **Path enumeration**: All paths with cycle detection
- **Property graphs**: Define over relational tables (zero copy)

### Alternatives Considered

1. **PostgreSQL + Apache Age**
   - Pro: Mature, ACID, familiar
   - Con: Requires server, no S3 backend, Cypher not SQL standard
   - Rejected: Adds operational complexity

2. **Neptune Serverless**
   - Pro: Fully managed, scales automatically
   - Con: $43/mo minimum (no scale-to-zero), requires VPC
   - Rejected: Too expensive for small/idle tenants

3. **Pure Recursive CTEs (no extension)**
   - Pro: No extension dependency
   - Con: Verbose, no pattern matching syntax, harder to maintain
   - Rejected: DuckPGQ provides cleaner abstraction

4. **In-memory graph (petgraph)**
   - Pro: Fast, pure Rust
   - Con: No persistence, no query language, manual serialization
   - Rejected: Need durable storage and query capabilities

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                              AETERNA                                     │
│                                                                          │
│  ┌────────────────┐     ┌────────────────┐     ┌────────────────────┐  │
│  │ MemoryManager  │────▶│  GraphStore    │────▶│  MemoryR1Trainer   │  │
│  └────────────────┘     └───────┬────────┘     └────────────────────┘  │
│                                 │                                        │
│                    ┌────────────▼────────────┐                          │
│                    │      DuckDB Engine      │                          │
│                    │  ┌──────────────────┐   │                          │
│                    │  │    DuckPGQ       │   │                          │
│                    │  │  (SQL/PGQ ext)   │   │                          │
│                    │  └──────────────────┘   │                          │
│                    └────────────┬────────────┘                          │
│                                 │                                        │
│              ┌──────────────────┼──────────────────┐                    │
│              ▼                  ▼                  ▼                    │
│     ┌────────────────┐ ┌────────────────┐ ┌────────────────┐           │
│     │  Local File    │ │   S3 Parquet   │ │   In-Memory    │           │
│     │  (.duckdb)     │ │   (persistent) │ │   (ephemeral)  │           │
│     └────────────────┘ └────────────────┘ └────────────────┘           │
└─────────────────────────────────────────────────────────────────────────┘
```

## Data Model

### Schema

```sql
-- Nodes: Memory entries with embeddings
CREATE TABLE memory_nodes (
    id VARCHAR PRIMARY KEY,
    content TEXT NOT NULL,
    layer VARCHAR NOT NULL,  -- 'agent', 'user', 'session', 'project', 'team', 'org', 'company'
    tenant_id VARCHAR NOT NULL,
    embedding FLOAT[384],    -- Vector for similarity (optional, Qdrant primary)
    metadata JSON,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Edges: Relationships between memories
CREATE TABLE memory_edges (
    id VARCHAR PRIMARY KEY,
    source_id VARCHAR NOT NULL REFERENCES memory_nodes(id),
    target_id VARCHAR NOT NULL REFERENCES memory_nodes(id),
    relation_type VARCHAR NOT NULL,  -- 'references', 'contradicts', 'supersedes', 'related_to'
    weight FLOAT DEFAULT 1.0,        -- Strength of relationship
    metadata JSON,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Entity index: Extracted entities for lookup
CREATE TABLE entities (
    id VARCHAR PRIMARY KEY,
    name VARCHAR NOT NULL,
    entity_type VARCHAR NOT NULL,  -- 'project', 'person', 'technology', 'concept'
    memory_id VARCHAR NOT NULL REFERENCES memory_nodes(id),
    tenant_id VARCHAR NOT NULL
);

-- Entity-Entity edges (derived from memory relationships)
CREATE TABLE entity_edges (
    source_entity_id VARCHAR REFERENCES entities(id),
    target_entity_id VARCHAR REFERENCES entities(id),
    relation_type VARCHAR NOT NULL,
    memory_edge_id VARCHAR REFERENCES memory_edges(id),
    PRIMARY KEY (source_entity_id, target_entity_id, relation_type)
);
```

### Property Graph Definition

```sql
-- Define graph over relational tables (zero-copy view)
CREATE PROPERTY GRAPH memory_graph
VERTEX TABLES (
    memory_nodes 
        KEY (id)
        LABEL Memory
        PROPERTIES (id, content, layer, tenant_id, metadata)
)
EDGE TABLES (
    memory_edges
        KEY (id)
        SOURCE KEY (source_id) REFERENCES memory_nodes (id)
        DESTINATION KEY (target_id) REFERENCES memory_nodes (id)
        LABEL Relates
        PROPERTIES (relation_type, weight)
);

-- Separate graph for entity-level reasoning
CREATE PROPERTY GRAPH entity_graph
VERTEX TABLES (
    entities
        KEY (id)
        LABEL Entity
        PROPERTIES (id, name, entity_type)
)
EDGE TABLES (
    entity_edges
        SOURCE KEY (source_entity_id) REFERENCES entities (id)
        DESTINATION KEY (target_entity_id) REFERENCES entities (id)
        LABEL LinkedTo
        PROPERTIES (relation_type)
);
```

## Query Patterns

### 1. Find Related Memories (1-3 hops)

```sql
FROM GRAPH_TABLE (memory_graph
    MATCH (m1:Memory)-[r:Relates]->{1,3}(m2:Memory)
    WHERE m1.id = $memory_id
      AND m1.tenant_id = $tenant_id
    COLUMNS (
        m2.id AS related_id,
        m2.content AS related_content,
        m2.layer AS related_layer,
        element_id(r) AS path,
        path_length(r) AS distance
    )
)
ORDER BY distance, r.weight DESC
LIMIT 20;
```

### 2. Shortest Path Between Memories

```sql
FROM GRAPH_TABLE (memory_graph
    MATCH SHORTEST (m1:Memory)-[r:Relates]->+(m2:Memory)
    WHERE m1.id = $start_id AND m2.id = $end_id
    COLUMNS (
        m1.id AS start,
        m2.id AS end,
        path_length(r) AS hops,
        vertices(r) AS path_nodes
    )
);
```

### 3. Find Common Ancestors (Reasoning Path)

```sql
-- Find memories that connect two concepts
FROM GRAPH_TABLE (entity_graph
    MATCH (e1:Entity)-[r1:LinkedTo]->{1,3}(common:Entity)<-[r2:LinkedTo]-{1,3}(e2:Entity)
    WHERE e1.name = $concept_a AND e2.name = $concept_b
    COLUMNS (
        e1.name AS concept_a,
        e2.name AS concept_b,
        common.name AS connecting_concept,
        common.entity_type AS connector_type
    )
)
LIMIT 10;
```

### 4. Community Detection (Weakly Connected Components)

```sql
-- Using iterative label propagation via recursive CTE
WITH RECURSIVE components(node_id, component_id, iteration) AS (
    -- Initialize: each node is its own component
    SELECT id AS node_id, id AS component_id, 0 AS iteration
    FROM memory_nodes
    WHERE tenant_id = $tenant_id
    
    UNION ALL
    
    -- Propagate: adopt smallest neighbor component
    SELECT 
        c.node_id,
        MIN(COALESCE(neighbor_component, c.component_id)) AS component_id,
        c.iteration + 1
    FROM components c
    LEFT JOIN memory_edges e ON c.node_id = e.source_id OR c.node_id = e.target_id
    LEFT JOIN components nc ON nc.node_id = CASE 
        WHEN e.source_id = c.node_id THEN e.target_id 
        ELSE e.source_id 
    END AS neighbor_component
    WHERE c.iteration < 10  -- Max iterations
    GROUP BY c.node_id
)
SELECT node_id, component_id
FROM components
WHERE iteration = (SELECT MAX(iteration) FROM components);
```

## Rust API Design

```rust
// storage/src/graph.rs

use duckdb::{Connection, Result, params};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphConfig {
    /// Storage mode: "memory", "file", or "s3"
    pub storage_mode: StorageMode,
    /// Path for file mode, S3 URI for s3 mode
    pub storage_path: Option<String>,
    /// S3 credentials (if s3 mode)
    pub s3_config: Option<S3Config>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StorageMode {
    Memory,
    File,
    S3,
}

pub struct GraphStore {
    conn: Connection,
    config: GraphConfig,
}

impl GraphStore {
    pub fn new(config: GraphConfig) -> Result<Self> {
        let conn = match &config.storage_mode {
            StorageMode::Memory => Connection::open_in_memory()?,
            StorageMode::File => {
                let path = config.storage_path.as_ref()
                    .ok_or_else(|| duckdb::Error::InvalidParameterName("storage_path required".into()))?;
                Connection::open(path)?
            }
            StorageMode::S3 => {
                let conn = Connection::open_in_memory()?;
                // Configure S3 access
                if let Some(s3) = &config.s3_config {
                    conn.execute_batch(&format!(r#"
                        INSTALL httpfs; LOAD httpfs;
                        SET s3_region = '{}';
                        SET s3_access_key_id = '{}';
                        SET s3_secret_access_key = '{}';
                    "#, s3.region, s3.access_key, s3.secret_key))?;
                }
                conn
            }
        };
        
        // Load DuckPGQ extension
        conn.execute_batch("INSTALL duckpgq FROM community; LOAD duckpgq;")?;
        
        let store = Self { conn, config };
        store.init_schema()?;
        Ok(store)
    }
    
    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(include_str!("../sql/schema.sql"))?;
        Ok(())
    }
    
    /// Add a memory node to the graph
    pub fn add_node(&self, node: &MemoryNode) -> Result<()> {
        self.conn.execute(
            "INSERT INTO memory_nodes (id, content, layer, tenant_id, metadata) 
             VALUES (?, ?, ?, ?, ?)",
            params![node.id, node.content, node.layer, node.tenant_id, node.metadata],
        )?;
        Ok(())
    }
    
    /// Add an edge between memories
    pub fn add_edge(&self, edge: &MemoryEdge) -> Result<()> {
        self.conn.execute(
            "INSERT INTO memory_edges (id, source_id, target_id, relation_type, weight) 
             VALUES (?, ?, ?, ?, ?)",
            params![edge.id, edge.source_id, edge.target_id, edge.relation_type, edge.weight],
        )?;
        Ok(())
    }
    
    /// Find related memories within N hops
    pub fn find_related(&self, memory_id: &str, max_hops: u32, limit: u32) -> Result<Vec<RelatedMemory>> {
        let query = format!(r#"
            FROM GRAPH_TABLE (memory_graph
                MATCH (m1:Memory)-[r:Relates]->{{1,{}}}(m2:Memory)
                WHERE m1.id = ?
                COLUMNS (
                    m2.id AS id,
                    m2.content AS content,
                    m2.layer AS layer,
                    path_length(r) AS distance
                )
            )
            ORDER BY distance
            LIMIT {}
        "#, max_hops, limit);
        
        let mut stmt = self.conn.prepare(&query)?;
        let rows = stmt.query_map([memory_id], |row| {
            Ok(RelatedMemory {
                id: row.get(0)?,
                content: row.get(1)?,
                layer: row.get(2)?,
                distance: row.get(3)?,
            })
        })?;
        
        rows.collect()
    }
    
    /// Find shortest path between two memories
    pub fn shortest_path(&self, start_id: &str, end_id: &str) -> Result<Option<GraphPath>> {
        let query = r#"
            FROM GRAPH_TABLE (memory_graph
                MATCH SHORTEST (m1:Memory)-[r:Relates]->+(m2:Memory)
                WHERE m1.id = ? AND m2.id = ?
                COLUMNS (
                    path_length(r) AS hops,
                    vertices(r) AS path_nodes
                )
            )
        "#;
        
        let mut stmt = self.conn.prepare(query)?;
        let result = stmt.query_row([start_id, end_id], |row| {
            Ok(GraphPath {
                hops: row.get(0)?,
                nodes: row.get(1)?,
            })
        });
        
        match result {
            Ok(path) => Ok(Some(path)),
            Err(duckdb::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
    
    /// Persist to S3 (for S3 storage mode)
    pub fn persist_to_s3(&self) -> Result<()> {
        if let StorageMode::S3 = self.config.storage_mode {
            let base_path = self.config.storage_path.as_ref().unwrap();
            self.conn.execute_batch(&format!(r#"
                COPY memory_nodes TO '{}/memory_nodes.parquet' (FORMAT PARQUET);
                COPY memory_edges TO '{}/memory_edges.parquet' (FORMAT PARQUET);
                COPY entities TO '{}/entities.parquet' (FORMAT PARQUET);
                COPY entity_edges TO '{}/entity_edges.parquet' (FORMAT PARQUET);
            "#, base_path, base_path, base_path, base_path))?;
        }
        Ok(())
    }
    
    /// Load from S3 (for S3 storage mode)
    pub fn load_from_s3(&self) -> Result<()> {
        if let StorageMode::S3 = self.config.storage_mode {
            let base_path = self.config.storage_path.as_ref().unwrap();
            self.conn.execute_batch(&format!(r#"
                INSERT INTO memory_nodes SELECT * FROM '{}/memory_nodes.parquet';
                INSERT INTO memory_edges SELECT * FROM '{}/memory_edges.parquet';
                INSERT INTO entities SELECT * FROM '{}/entities.parquet';
                INSERT INTO entity_edges SELECT * FROM '{}/entity_edges.parquet';
            "#, base_path, base_path, base_path, base_path))?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct MemoryNode {
    pub id: String,
    pub content: String,
    pub layer: String,
    pub tenant_id: String,
    pub metadata: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MemoryEdge {
    pub id: String,
    pub source_id: String,
    pub target_id: String,
    pub relation_type: String,
    pub weight: f32,
}

#[derive(Debug, Clone)]
pub struct RelatedMemory {
    pub id: String,
    pub content: String,
    pub layer: String,
    pub distance: u32,
}

#[derive(Debug, Clone)]
pub struct GraphPath {
    pub hops: u32,
    pub nodes: Vec<String>,
}
```

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| **DuckPGQ is community extension** | Monitor upstream; fallback to recursive CTEs if abandoned |
| **No concurrent writes** | DuckDB supports single-writer; use write-ahead queue if needed |
| **Large graphs may OOM** | Partition by tenant; use S3 checkpointing for recovery |
| **Extension compatibility** | Pin DuckDB + DuckPGQ versions in Cargo.toml |

## Migration Plan

1. **Phase 1**: Implement `GraphStore` with in-memory mode (development)
2. **Phase 2**: Add file-based persistence for single-node deployments
3. **Phase 3**: Add S3 persistence for multi-node/serverless deployments
4. **Phase 4**: Integrate with `MemoryManager` for automatic graph updates

## Open Questions

1. **Graph partitioning**: Should we have one graph per tenant or partition within a single graph?
   - **Recommendation**: Start with tenant_id column filter; partition files by tenant for S3

2. **Entity extraction**: Use LLM or rule-based?
   - **Recommendation**: Start with LLM (via existing provider abstraction); add rule-based for common patterns

3. **Write frequency**: How often do we persist to S3?
   - **Recommendation**: On session end + periodic (every 5 minutes) + on explicit flush
