use duckdb::{Connection, params};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    Knowledge,
    Memory,
    CodeFile,
    CodeSymbol,
    CodeChunk,
}

impl std::fmt::Display for NodeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = serde_json::to_value(self).unwrap_or_default();
        write!(f, "{}", s.as_str().unwrap_or("unknown"))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum EdgeType {
    Implements,
    References,
    Violates,
    DerivedFrom,
    Calls,
    RelatedTo,
}

impl std::fmt::Display for EdgeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = serde_json::to_value(self).unwrap_or_default();
        write!(f, "{}", s.as_str().unwrap_or("unknown"))
    }
}

#[derive(Debug, Clone)]
pub struct CodeChunkNode {
    pub id: String,
    pub file_path: String,
    pub content: String,
    pub score: f32,
    pub language: Option<String>,
}

pub struct UnifiedGraphSyncer;

pub fn initialize_unified_graph_schema(conn: &Connection) -> Result<(), duckdb::Error> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS graph_nodes (
            id VARCHAR PRIMARY KEY,
            node_type VARCHAR NOT NULL,
            label VARCHAR NOT NULL,
            properties JSON,
            tenant_id VARCHAR NOT NULL,
            created_at TIMESTAMP DEFAULT now(),
            updated_at TIMESTAMP DEFAULT now()
        );
        CREATE TABLE IF NOT EXISTS graph_edges (
            id VARCHAR PRIMARY KEY,
            source_id VARCHAR NOT NULL,
            target_id VARCHAR NOT NULL,
            edge_type VARCHAR NOT NULL,
            properties JSON,
            confidence FLOAT DEFAULT 1.0,
            tenant_id VARCHAR NOT NULL,
            created_at TIMESTAMP DEFAULT now()
        );
        CREATE INDEX IF NOT EXISTS idx_graph_nodes_tenant ON graph_nodes(tenant_id);
        CREATE INDEX IF NOT EXISTS idx_graph_nodes_type ON graph_nodes(node_type);
        CREATE INDEX IF NOT EXISTS idx_graph_edges_source ON graph_edges(source_id);
        CREATE INDEX IF NOT EXISTS idx_graph_edges_target ON graph_edges(target_id);
        CREATE INDEX IF NOT EXISTS idx_graph_edges_type ON graph_edges(edge_type);
        CREATE INDEX IF NOT EXISTS idx_graph_edges_tenant ON graph_edges(tenant_id);
        ",
    )?;
    Ok(())
}

impl UnifiedGraphSyncer {
    pub fn sync_code_chunk(
        conn: &Connection,
        tenant_id: &str,
        chunk: &CodeChunkNode,
    ) -> Result<(), duckdb::Error> {
        let props = serde_json::json!({
            "file_path": chunk.file_path,
            "score": chunk.score,
            "language": chunk.language,
        });
        let props_str = serde_json::to_string(&props).unwrap_or_else(|_| "{}".to_string());

        conn.execute(
            r#"
            INSERT INTO graph_nodes (id, node_type, label, properties, tenant_id)
            VALUES (?, 'code_chunk', ?, ?, ?)
            ON CONFLICT (id) DO UPDATE SET
                label = EXCLUDED.label,
                properties = EXCLUDED.properties,
                updated_at = now()
            "#,
            params![chunk.id, chunk.content, props_str, tenant_id],
        )?;
        Ok(())
    }

    pub fn sync_call_edge(
        conn: &Connection,
        tenant_id: &str,
        caller_id: &str,
        callee_id: &str,
        confidence: f32,
    ) -> Result<(), duckdb::Error> {
        let edge_id = format!("{tenant_id}:calls:{caller_id}:{callee_id}");

        conn.execute(
            r#"
            INSERT INTO graph_edges (id, source_id, target_id, edge_type, properties, confidence, tenant_id)
            VALUES (?, ?, ?, 'calls', '{}', ?, ?)
            ON CONFLICT (id) DO UPDATE SET
                confidence = EXCLUDED.confidence,
                properties = EXCLUDED.properties
            "#,
            params![edge_id, caller_id, callee_id, confidence, tenant_id],
        )?;
        Ok(())
    }

    pub fn sync_knowledge_node(
        conn: &Connection,
        tenant_id: &str,
        id: &str,
        label: &str,
        props: &serde_json::Value,
    ) -> Result<(), duckdb::Error> {
        let props_str = serde_json::to_string(props).unwrap_or_else(|_| "{}".to_string());

        conn.execute(
            r#"
            INSERT INTO graph_nodes (id, node_type, label, properties, tenant_id)
            VALUES (?, 'knowledge', ?, ?, ?)
            ON CONFLICT (id) DO UPDATE SET
                label = EXCLUDED.label,
                properties = EXCLUDED.properties,
                updated_at = now()
            "#,
            params![id, label, props_str, tenant_id],
        )?;
        Ok(())
    }

    pub fn sync_memory_node(
        conn: &Connection,
        tenant_id: &str,
        id: &str,
        label: &str,
        props: &serde_json::Value,
    ) -> Result<(), duckdb::Error> {
        let props_str = serde_json::to_string(props).unwrap_or_else(|_| "{}".to_string());

        conn.execute(
            r#"
            INSERT INTO graph_nodes (id, node_type, label, properties, tenant_id)
            VALUES (?, 'memory', ?, ?, ?)
            ON CONFLICT (id) DO UPDATE SET
                label = EXCLUDED.label,
                properties = EXCLUDED.properties,
                updated_at = now()
            "#,
            params![id, label, props_str, tenant_id],
        )?;
        Ok(())
    }
}

fn query_three_strings(
    stmt: &mut duckdb::Statement<'_>,
    p: &[&dyn duckdb::types::ToSql],
) -> Result<Vec<(String, String, String)>, duckdb::Error> {
    let rows = stmt.query_map(p, |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    rows.collect()
}

pub fn export_to_dot(
    conn: &Connection,
    tenant_id: &str,
    node_type_filter: Option<&str>,
) -> Result<String, duckdb::Error> {
    let mut dot = String::from("digraph G {\n");

    let node_sql = match node_type_filter {
        Some(_) => {
            "SELECT id, label, node_type FROM graph_nodes WHERE tenant_id = ? AND node_type = ?"
        }
        None => "SELECT id, label, node_type FROM graph_nodes WHERE tenant_id = ?",
    };

    {
        let mut stmt = conn.prepare(node_sql)?;
        let node_rows = match node_type_filter {
            Some(nt) => query_three_strings(&mut stmt, &[&tenant_id, &nt])?,
            None => query_three_strings(&mut stmt, &[&tenant_id])?,
        };

        for (id, label, ntype) in node_rows {
            let safe_label = label.replace('"', r#"\""#);
            dot.push_str(&format!(
                "  \"{}\" [label=\"{}\" type=\"{}\"];\n",
                id, safe_label, ntype
            ));
        }
    }

    let edge_sql = match node_type_filter {
        Some(_) => {
            "SELECT e.source_id, e.target_id, e.edge_type \
             FROM graph_edges e \
             JOIN graph_nodes n ON n.id = e.source_id AND n.tenant_id = e.tenant_id \
             WHERE e.tenant_id = ? AND n.node_type = ?"
        }
        None => "SELECT source_id, target_id, edge_type FROM graph_edges WHERE tenant_id = ?",
    };

    {
        let mut stmt = conn.prepare(edge_sql)?;
        let edge_rows = match node_type_filter {
            Some(nt) => query_three_strings(&mut stmt, &[&tenant_id, &nt])?,
            None => query_three_strings(&mut stmt, &[&tenant_id])?,
        };

        for (src, tgt, etype) in edge_rows {
            dot.push_str(&format!(
                "  \"{}\" -> \"{}\" [label=\"{}\"];\n",
                src, tgt, etype
            ));
        }
    }

    dot.push_str("}\n");
    Ok(dot)
}

pub fn export_to_json(
    conn: &Connection,
    tenant_id: &str,
) -> Result<serde_json::Value, duckdb::Error> {
    let mut nodes = Vec::new();
    {
        let mut stmt = conn.prepare(
            "SELECT id, node_type, label, properties FROM graph_nodes WHERE tenant_id = ?",
        )?;
        let rows = stmt.query_map(params![tenant_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;

        for r in rows {
            let (id, ntype, label, props_str) = r?;
            let props: serde_json::Value =
                serde_json::from_str(&props_str).unwrap_or(serde_json::Value::Null);
            nodes.push(serde_json::json!({
                "id": id,
                "node_type": ntype,
                "label": label,
                "properties": props,
            }));
        }
    }

    let mut edges = Vec::new();
    {
        let mut stmt = conn.prepare(
            "SELECT id, source_id, target_id, edge_type, confidence \
             FROM graph_edges WHERE tenant_id = ?",
        )?;
        let rows = stmt.query_map(params![tenant_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, f64>(4)?,
            ))
        })?;

        for r in rows {
            let (id, src, tgt, etype, conf) = r?;
            edges.push(serde_json::json!({
                "id": id,
                "source_id": src,
                "target_id": tgt,
                "edge_type": etype,
                "confidence": conf,
            }));
        }
    }

    Ok(serde_json::json!({ "nodes": nodes, "edges": edges }))
}

/// Placeholder — returns `Ok(vec![])` until embedding-based similarity is wired up.
pub fn detect_semantic_links(
    _conn: &Connection,
    _tenant_id: &str,
    _threshold: f64,
) -> Result<Vec<(String, String, f64)>, duckdb::Error> {
    Ok(vec![])
}

/// Multiply `confidence` by `decay_factor` on all `related_to` edges for `tenant_id`.
/// Returns the number of rows affected.
pub fn decay_link_confidence(
    conn: &Connection,
    tenant_id: &str,
    decay_factor: f64,
) -> Result<usize, duckdb::Error> {
    let affected = conn.execute(
        "UPDATE graph_edges SET confidence = confidence * ? WHERE edge_type = 'related_to' AND tenant_id = ?",
        params![decay_factor, tenant_id],
    )?;
    Ok(affected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use duckdb::Connection;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        initialize_unified_graph_schema(&conn).unwrap();
        conn
    }

    #[test]
    fn test_node_type_display() {
        assert_eq!(NodeType::CodeChunk.to_string(), "code_chunk");
        assert_eq!(NodeType::Knowledge.to_string(), "knowledge");
        assert_eq!(NodeType::Memory.to_string(), "memory");
        assert_eq!(NodeType::CodeFile.to_string(), "code_file");
        assert_eq!(NodeType::CodeSymbol.to_string(), "code_symbol");
    }

    #[test]
    fn test_edge_type_display() {
        assert_eq!(EdgeType::Calls.to_string(), "calls");
        assert_eq!(EdgeType::Implements.to_string(), "implements");
        assert_eq!(EdgeType::References.to_string(), "references");
        assert_eq!(EdgeType::Violates.to_string(), "violates");
        assert_eq!(EdgeType::DerivedFrom.to_string(), "derived_from");
        assert_eq!(EdgeType::RelatedTo.to_string(), "related_to");
    }

    #[test]
    fn test_node_type_serde_roundtrip() {
        let nt = NodeType::CodeChunk;
        let json = serde_json::to_string(&nt).unwrap();
        assert_eq!(json, r#""code_chunk""#);
        let back: NodeType = serde_json::from_str(&json).unwrap();
        assert_eq!(back, nt);
    }

    #[test]
    fn test_sync_code_chunk() {
        let conn = setup_db();
        let chunk = CodeChunkNode {
            id: "chunk-1".into(),
            file_path: "src/main.rs".into(),
            content: "fn main() {}".into(),
            score: 0.95,
            language: Some("rust".into()),
        };

        UnifiedGraphSyncer::sync_code_chunk(&conn, "t1", &chunk).unwrap();

        let (label, ntype): (String, String) = conn
            .query_row(
                "SELECT label, node_type FROM graph_nodes WHERE id = ? AND tenant_id = ?",
                params!["chunk-1", "t1"],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(ntype, "code_chunk");
        assert_eq!(label, "fn main() {}");

        let chunk2 = CodeChunkNode {
            id: "chunk-1".into(),
            file_path: "src/main.rs".into(),
            content: "fn main() { println!(\"hi\"); }".into(),
            score: 0.99,
            language: Some("rust".into()),
        };
        UnifiedGraphSyncer::sync_code_chunk(&conn, "t1", &chunk2).unwrap();

        let label2: String = conn
            .query_row(
                "SELECT label FROM graph_nodes WHERE id = ?",
                params!["chunk-1"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(label2, "fn main() { println!(\"hi\"); }");
    }

    #[test]
    fn test_sync_call_edge() {
        let conn = setup_db();

        let c1 = CodeChunkNode {
            id: "fn-a".into(),
            file_path: "a.rs".into(),
            content: "fn a()".into(),
            score: 1.0,
            language: None,
        };
        let c2 = CodeChunkNode {
            id: "fn-b".into(),
            file_path: "b.rs".into(),
            content: "fn b()".into(),
            score: 1.0,
            language: None,
        };
        UnifiedGraphSyncer::sync_code_chunk(&conn, "t1", &c1).unwrap();
        UnifiedGraphSyncer::sync_code_chunk(&conn, "t1", &c2).unwrap();

        UnifiedGraphSyncer::sync_call_edge(&conn, "t1", "fn-a", "fn-b", 0.8).unwrap();

        let (etype, conf): (String, f64) = conn
            .query_row(
                "SELECT edge_type, confidence FROM graph_edges WHERE tenant_id = ?",
                params!["t1"],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(etype, "calls");
        assert!((conf - 0.8).abs() < 1e-6);

        UnifiedGraphSyncer::sync_call_edge(&conn, "t1", "fn-a", "fn-b", 0.5).unwrap();
        let conf2: f64 = conn
            .query_row(
                "SELECT confidence FROM graph_edges WHERE tenant_id = ?",
                params!["t1"],
                |row| row.get(0),
            )
            .unwrap();
        assert!((conf2 - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_sync_knowledge_node() {
        let conn = setup_db();
        let props = serde_json::json!({"source": "adr-042"});
        UnifiedGraphSyncer::sync_knowledge_node(&conn, "t1", "k1", "Use Postgres", &props).unwrap();

        let ntype: String = conn
            .query_row(
                "SELECT node_type FROM graph_nodes WHERE id = ?",
                params!["k1"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(ntype, "knowledge");
    }

    #[test]
    fn test_sync_memory_node() {
        let conn = setup_db();
        let props = serde_json::json!({"layer": "project"});
        UnifiedGraphSyncer::sync_memory_node(&conn, "t1", "m1", "Remember X", &props).unwrap();

        let ntype: String = conn
            .query_row(
                "SELECT node_type FROM graph_nodes WHERE id = ?",
                params!["m1"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(ntype, "memory");
    }

    #[test]
    fn test_export_to_dot() {
        let conn = setup_db();

        let c1 = CodeChunkNode {
            id: "n1".into(),
            file_path: "a.rs".into(),
            content: "alpha".into(),
            score: 1.0,
            language: None,
        };
        let c2 = CodeChunkNode {
            id: "n2".into(),
            file_path: "b.rs".into(),
            content: "beta".into(),
            score: 1.0,
            language: None,
        };
        UnifiedGraphSyncer::sync_code_chunk(&conn, "t1", &c1).unwrap();
        UnifiedGraphSyncer::sync_code_chunk(&conn, "t1", &c2).unwrap();
        UnifiedGraphSyncer::sync_call_edge(&conn, "t1", "n1", "n2", 0.9).unwrap();

        let dot = export_to_dot(&conn, "t1", None).unwrap();
        assert!(dot.starts_with("digraph G {"));
        assert!(dot.contains("\"n1\""));
        assert!(dot.contains("\"n2\""));
        assert!(dot.contains("\"n1\" -> \"n2\""));
        assert!(dot.ends_with("}\n"));

        let props = serde_json::json!({"x": 1});
        UnifiedGraphSyncer::sync_knowledge_node(&conn, "t1", "k1", "KB", &props).unwrap();
        let dot_filtered = export_to_dot(&conn, "t1", Some("knowledge")).unwrap();
        assert!(dot_filtered.contains("\"k1\""));
        assert!(!dot_filtered.contains("\"n1\""));
    }

    #[test]
    fn test_export_to_json() {
        let conn = setup_db();

        let c1 = CodeChunkNode {
            id: "j1".into(),
            file_path: "x.rs".into(),
            content: "fn x()".into(),
            score: 0.5,
            language: Some("rust".into()),
        };
        UnifiedGraphSyncer::sync_code_chunk(&conn, "t1", &c1).unwrap();

        let json = export_to_json(&conn, "t1").unwrap();
        let nodes = json["nodes"].as_array().unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0]["id"], "j1");
        assert_eq!(nodes[0]["node_type"], "code_chunk");

        let edges = json["edges"].as_array().unwrap();
        assert!(edges.is_empty());
    }

    #[test]
    fn test_detect_semantic_links_placeholder() {
        let conn = setup_db();
        let links = detect_semantic_links(&conn, "t1", 0.5).unwrap();
        assert!(links.is_empty());
    }

    #[test]
    fn test_decay_link_confidence() {
        let conn = setup_db();

        let props = serde_json::json!({});
        UnifiedGraphSyncer::sync_knowledge_node(&conn, "t1", "a", "A", &props).unwrap();
        UnifiedGraphSyncer::sync_knowledge_node(&conn, "t1", "b", "B", &props).unwrap();

        conn.execute(
            "INSERT INTO graph_edges (id, source_id, target_id, edge_type, confidence, tenant_id) \
             VALUES (?, ?, ?, 'related_to', 1.0, ?)",
            params!["e1", "a", "b", "t1"],
        )
        .unwrap();

        let affected = decay_link_confidence(&conn, "t1", 0.5).unwrap();
        assert_eq!(affected, 1);

        let conf: f64 = conn
            .query_row(
                "SELECT confidence FROM graph_edges WHERE id = ?",
                params!["e1"],
                |row| row.get(0),
            )
            .unwrap();
        assert!((conf - 0.5).abs() < f64::EPSILON);

        decay_link_confidence(&conn, "t1", 0.5).unwrap();
        let conf2: f64 = conn
            .query_row(
                "SELECT confidence FROM graph_edges WHERE id = ?",
                params!["e1"],
                |row| row.get(0),
            )
            .unwrap();
        assert!((conf2 - 0.25).abs() < f64::EPSILON);
    }

    #[test]
    fn test_decay_only_affects_related_to_edges() {
        let conn = setup_db();

        let props = serde_json::json!({});
        UnifiedGraphSyncer::sync_knowledge_node(&conn, "t1", "x", "X", &props).unwrap();
        UnifiedGraphSyncer::sync_knowledge_node(&conn, "t1", "y", "Y", &props).unwrap();

        UnifiedGraphSyncer::sync_call_edge(&conn, "t1", "x", "y", 1.0).unwrap();

        let affected = decay_link_confidence(&conn, "t1", 0.1).unwrap();
        assert_eq!(affected, 0);

        let conf: f64 = conn
            .query_row(
                "SELECT confidence FROM graph_edges WHERE tenant_id = ?",
                params!["t1"],
                |row| row.get(0),
            )
            .unwrap();
        assert!((conf - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_tenant_isolation() {
        let conn = setup_db();

        let props = serde_json::json!({});
        UnifiedGraphSyncer::sync_knowledge_node(&conn, "t1", "shared-id", "T1 node", &props)
            .unwrap();
        UnifiedGraphSyncer::sync_knowledge_node(&conn, "t2", "t2-node", "T2 node", &props).unwrap();

        let json = export_to_json(&conn, "t1").unwrap();
        let nodes = json["nodes"].as_array().unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0]["id"], "shared-id");
    }
}
