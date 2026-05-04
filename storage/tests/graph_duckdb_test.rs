use chrono::Utc;
use mk_core::types::{TenantContext, TenantId, UserId};
use serde_json::json;
use std::str::FromStr;
use storage::graph::{GraphEdge, GraphNode, GraphStore};
use storage::graph_duckdb::{DuckDbGraphConfig, DuckDbGraphStore, Entity, EntityEdge};
use uuid::Uuid;

fn explain_plan(conn: &duckdb::Connection, sql: &str) -> String {
    let mut stmt = conn.prepare(sql).expect("prepare explain");
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .expect("query explain")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect explain rows");
    rows.join("\n")
}

#[tokio::test]
async fn test_duckdb_schema_initialization() {
    let config = DuckDbGraphConfig {
        path: ":memory:".to_string(),
        ..Default::default()
    };

    let store = DuckDbGraphStore::new(config).expect("Failed to create DuckDbGraphStore");

    let version = store
        .get_current_schema_version()
        .expect("Failed to get schema version");
    assert!(version >= 1);

    let history = store
        .get_migration_history()
        .expect("Failed to get migration history");
    assert!(!history.is_empty());
    assert_eq!(history[0].version, 1);
}

#[tokio::test]
async fn test_duckdb_tenant_validation() {
    let config = DuckDbGraphConfig::default();
    let store = DuckDbGraphStore::new(config).expect("Failed to create DuckDbGraphStore");

    let tenant_id = TenantId::from_str("valid-tenant_123").unwrap();
    let user_id = UserId::from_str("user-1").unwrap();
    let ctx = TenantContext::new(tenant_id, user_id);
    let node = GraphNode {
        id: Uuid::new_v4().to_string(),
        label: "Test".to_string(),
        properties: json!({}),
        tenant_id: "valid-tenant_123".to_string(),
    };
    let _: () = store
        .add_node(ctx, node)
        .await
        .expect("Should add node for valid tenant");

    let tenant_invalid = TenantId::from_str("invalid_tenant").unwrap();
    let ctx_invalid = TenantContext::new(tenant_invalid, UserId::from_str("user-1").unwrap());
    let node_invalid = GraphNode {
        id: Uuid::new_v4().to_string(),
        label: "Test".to_string(),
        properties: json!({}),
        tenant_id: "invalid;tenant".to_string(),
    };
    let result: Result<(), _> = store.add_node(ctx_invalid, node_invalid).await;
    assert!(result.is_err());

    let ctx_sql = TenantContext::new(TenantId::default(), UserId::from_str("user-1").unwrap());
    let node_sql = GraphNode {
        id: Uuid::new_v4().to_string(),
        label: "Test".to_string(),
        properties: json!({}),
        tenant_id: "tenant' OR '1'='1".to_string(),
    };
    let result: Result<(), _> = store.add_node(ctx_sql, node_sql).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_duckdb_graph_operations() {
    let config = DuckDbGraphConfig::default();
    let store = DuckDbGraphStore::new(config).expect("Failed to create DuckDbGraphStore");
    let tenant_str = "test_tenant";
    let tenant_id = TenantId::from_str(tenant_str).unwrap();
    let ctx = TenantContext::new(tenant_id, UserId::from_str("user-1").unwrap());

    let node1_id = "node1";
    let node2_id = "node2";

    let _: () = store
        .add_node(
            ctx.clone(),
            GraphNode {
                id: node1_id.to_string(),
                label: "User".to_string(),
                properties: json!({"name": "Alice"}),
                tenant_id: tenant_str.to_string(),
            },
        )
        .await
        .unwrap();

    let _: () = store
        .add_node(
            ctx.clone(),
            GraphNode {
                id: node2_id.to_string(),
                label: "User".to_string(),
                properties: json!({"name": "Bob"}),
                tenant_id: tenant_str.to_string(),
            },
        )
        .await
        .unwrap();

    let _: () = store
        .add_edge(
            ctx.clone(),
            GraphEdge {
                id: "edge1".to_string(),
                source_id: node1_id.to_string(),
                target_id: node2_id.to_string(),
                relation: "FOLLOWS".to_string(),
                properties: json!({"since": "2023-01-01"}),
                tenant_id: tenant_str.to_string(),
            },
        )
        .await
        .unwrap();

    let neighbors: Vec<_> = store.get_neighbors(ctx.clone(), node1_id).await.unwrap();
    assert_eq!(neighbors.len(), 1);
    assert_eq!(neighbors[0].1.id, node2_id);

    let path: Vec<_> = store
        .find_path(ctx.clone(), node1_id, node2_id, 2)
        .await
        .unwrap();
    assert_eq!(path.len(), 1);
    assert_eq!(path[0].id, "edge1");
}

#[tokio::test]
async fn test_duckdb_records_monotonic_mutation_seq() {
    let store = DuckDbGraphStore::new(DuckDbGraphConfig::default()).expect("create store");
    let tenant = "test_tenant_seq";
    let ctx = TenantContext::new(
        TenantId::from_str(tenant).unwrap(),
        UserId::from_str("user-1").unwrap(),
    );

    store
        .add_node(
            ctx.clone(),
            GraphNode {
                id: "node-a".to_string(),
                label: "Person".to_string(),
                properties: json!({}),
                tenant_id: tenant.to_string(),
            },
        )
        .await
        .unwrap();
    store
        .add_node(
            ctx.clone(),
            GraphNode {
                id: "node-b".to_string(),
                label: "Person".to_string(),
                properties: json!({}),
                tenant_id: tenant.to_string(),
            },
        )
        .await
        .unwrap();
    store
        .add_edge(
            ctx,
            GraphEdge {
                id: "edge-ab".to_string(),
                source_id: "node-a".to_string(),
                target_id: "node-b".to_string(),
                relation: "KNOWS".to_string(),
                properties: json!({}),
                tenant_id: tenant.to_string(),
            },
        )
        .await
        .unwrap();

    let conn = store.db_handle();
    let conn = conn.lock();
    let node_a_seq: i64 = conn
        .query_row(
            "SELECT seq FROM memory_nodes WHERE id = 'node-a'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let node_b_seq: i64 = conn
        .query_row(
            "SELECT seq FROM memory_nodes WHERE id = 'node-b'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let edge_seq: i64 = conn
        .query_row(
            "SELECT seq FROM memory_edges WHERE id = 'edge-ab'",
            [],
            |row| row.get(0),
        )
        .unwrap();

    assert!(node_a_seq > 0);
    assert!(node_b_seq > node_a_seq);
    assert!(edge_seq > node_b_seq);
}

#[tokio::test]
async fn test_duckdb_snapshot_delta_exports_since_seq() {
    let store = DuckDbGraphStore::new(DuckDbGraphConfig::default()).expect("create store");
    let tenant = "test_tenant_delta";
    let ctx = TenantContext::new(
        TenantId::from_str(tenant).unwrap(),
        UserId::from_str("user-1").unwrap(),
    );

    store
        .add_node(
            ctx.clone(),
            GraphNode {
                id: "node-a".to_string(),
                label: "Alpha".to_string(),
                properties: json!({}),
                tenant_id: tenant.to_string(),
            },
        )
        .await
        .unwrap();
    store
        .add_node(
            ctx.clone(),
            GraphNode {
                id: "node-b".to_string(),
                label: "Beta".to_string(),
                properties: json!({}),
                tenant_id: tenant.to_string(),
            },
        )
        .await
        .unwrap();

    // Scoped block so the MutexGuard is dropped before the subsequent
    // `.await`s — silences `clippy::await_holding_lock` (denied by CI gate).
    let cutoff_seq: i64 = {
        let conn = store.db_handle();
        let conn = conn.lock();
        conn.query_row(
            "SELECT seq FROM memory_nodes WHERE id = 'node-a'",
            [],
            |row| row.get(0),
        )
        .unwrap()
    };

    store
        .add_edge(
            ctx,
            GraphEdge {
                id: "edge-ab".to_string(),
                source_id: "node-a".to_string(),
                target_id: "node-b".to_string(),
                relation: "KNOWS".to_string(),
                properties: json!({}),
                tenant_id: tenant.to_string(),
            },
        )
        .await
        .unwrap();

    let (_, rows_exported, to_seq) = store.export_delta_to_parquet(tenant, cutoff_seq).unwrap();
    assert_eq!(rows_exported, 2);
    assert!(to_seq > cutoff_seq);
}

#[test]
fn test_duckdb_explain_uses_index_scan_for_label_equality() {
    let store = DuckDbGraphStore::new(DuckDbGraphConfig::default()).expect("create store");
    let conn = store.db_handle();
    let conn = conn.lock();

    // Insert 100k rows so the optimizer strongly prefers index scan for a single-match query.
    conn.execute_batch(
        r#"
        INSERT INTO memory_nodes (id, label, properties, tenant_id, seq)
        SELECT 'node-' || i, CASE WHEN i = 42424 THEN 'Needle' ELSE 'Other' END, '{}', 'tenant_explain', i
        FROM range(100000) t(i);
        "#,
    )
    .unwrap();

    // Force DuckDB to prefer index scans at low selectivity.
    conn.execute_batch("SET index_scan_max_count = 2048; SET index_scan_percentage = 0.002;")
        .unwrap();

    // Query on only the indexed column to maximize index scan eligibility.
    let plan = explain_plan(
        &conn,
        "EXPLAIN ANALYZE SELECT id FROM memory_nodes WHERE label = 'Needle'",
    );
    // DuckDB may show "INDEX_SCAN" or "Index Scan" depending on version; check case-insensitively.
    let plan_lower = plan.to_lowercase();
    assert!(
        plan_lower.contains("index_scan") || plan_lower.contains("index scan"),
        "Expected index scan in plan, got:\n{plan}"
    );
}

#[test]
fn test_duckdb_explain_uses_index_scan_for_source_lookup() {
    let store = DuckDbGraphStore::new(DuckDbGraphConfig::default()).expect("create store");
    let conn = store.db_handle();
    let conn = conn.lock();

    conn.execute_batch(
        r#"
        INSERT INTO memory_edges (id, source_id, target_id, relation, properties, tenant_id, seq)
        SELECT 'edge-' || i, CASE WHEN i = 42424 THEN 'needle-source' ELSE 'other-source-' || i END, 'target-' || i, 'KNOWS', '{}', 'tenant_explain', i
        FROM range(100000) t(i);
        "#,
    )
    .unwrap();

    conn.execute_batch("SET index_scan_max_count = 2048; SET index_scan_percentage = 0.002;")
        .unwrap();

    let plan = explain_plan(
        &conn,
        "EXPLAIN ANALYZE SELECT id FROM memory_edges WHERE source_id = 'needle-source'",
    );
    let plan_lower = plan.to_lowercase();
    assert!(
        plan_lower.contains("index_scan") || plan_lower.contains("index scan"),
        "Expected index scan in plan, got:\n{plan}"
    );
}

#[tokio::test]
async fn test_duckdb_soft_delete_and_cleanup() {
    let config = DuckDbGraphConfig::default();
    let store = DuckDbGraphStore::new(config).expect("Failed to create DuckDbGraphStore");
    let tenant_str = "test_tenant";
    let tenant_id = TenantId::from_str(tenant_str).unwrap();
    let ctx = TenantContext::new(tenant_id, UserId::from_str("user-1").unwrap());

    let node_id = "node_to_delete";
    let _: () = store
        .add_node(
            ctx.clone(),
            GraphNode {
                id: node_id.to_string(),
                label: "Item".to_string(),
                properties: json!({}),
                tenant_id: tenant_str.to_string(),
            },
        )
        .await
        .unwrap();

    let _: () = store
        .add_edge(
            ctx.clone(),
            GraphEdge {
                id: "edge_to_delete".to_string(),
                source_id: node_id.to_string(),
                target_id: node_id.to_string(),
                relation: "SELF".to_string(),
                properties: json!({}),
                tenant_id: tenant_str.to_string(),
            },
        )
        .await
        .unwrap();

    store.soft_delete_node(ctx.clone(), node_id).unwrap();

    let neighbors: Vec<_> = store.get_neighbors(ctx.clone(), node_id).await.unwrap();
    assert!(neighbors.is_empty());

    let older_than = Utc::now() + chrono::Duration::hours(1);
    let deleted = store.cleanup_deleted(older_than).unwrap();
    assert!(deleted >= 2);
}

#[tokio::test]
async fn test_duckdb_atomic_operations() {
    let config = DuckDbGraphConfig::default();
    let store = DuckDbGraphStore::new(config).expect("Failed to create DuckDbGraphStore");
    let tenant_str = "test_tenant";
    let tenant_id = TenantId::from_str(tenant_str).unwrap();
    let ctx = TenantContext::new(tenant_id.clone(), UserId::from_str("user-1").unwrap());

    let nodes = vec![
        GraphNode {
            id: "a".to_string(),
            label: "Node".to_string(),
            properties: json!({}),
            tenant_id: tenant_str.to_string(),
        },
        GraphNode {
            id: "b".to_string(),
            label: "Node".to_string(),
            properties: json!({}),
            tenant_id: tenant_str.to_string(),
        },
    ];

    let edges = vec![GraphEdge {
        id: "e1".to_string(),
        source_id: "a".to_string(),
        target_id: "b".to_string(),
        relation: "CONNECTS".to_string(),
        properties: json!({}),
        tenant_id: tenant_str.to_string(),
    }];

    store
        .add_nodes_and_edges_atomic(&ctx, tenant_str, nodes, edges)
        .unwrap();

    let stats = store.get_stats(ctx).unwrap();
    assert_eq!(stats.node_count, 2);
    assert_eq!(stats.edge_count, 1);
}

#[tokio::test]
async fn test_duckdb_entities_atomic() {
    let config = DuckDbGraphConfig::default();
    let store = DuckDbGraphStore::new(config).expect("Failed to create DuckDbGraphStore");
    let tenant_id_str = "test-tenant";
    let tenant_id = TenantId::from_str(tenant_id_str).unwrap();
    let ctx = TenantContext::new(tenant_id, UserId::from_str("user-1").unwrap());

    let entities = vec![Entity {
        id: "e1".to_string(),
        name: "Entity 1".to_string(),
        entity_type: "Person".to_string(),
        properties: json!({}),
        tenant_id: tenant_id_str.to_string(),
        created_at: Utc::now(),
        deleted_at: None,
    }];

    let edges = vec![EntityEdge {
        id: "re1".to_string(),
        source_entity_id: "e1".to_string(),
        target_entity_id: "e1".to_string(),
        relation: "SELF".to_string(),
        properties: json!({}),
        tenant_id: tenant_id_str.to_string(),
        created_at: Utc::now(),
        deleted_at: None,
    }];

    store
        .add_entities_atomic(&ctx, tenant_id_str, entities, edges)
        .unwrap();

    let stats = store.get_stats(ctx).unwrap();
    assert_eq!(stats.entity_count, 1);
    assert_eq!(stats.entity_edge_count, 1);
}

#[tokio::test]
async fn test_duckdb_shortest_path() {
    let config = DuckDbGraphConfig::default();
    let store = DuckDbGraphStore::new(config).expect("Failed to create DuckDbGraphStore");
    let tenant_id = "test-tenant";
    let ctx = TenantContext::new(
        TenantId::from_str(tenant_id).unwrap(),
        UserId::from_str("user-1").unwrap(),
    );

    store
        .add_node(
            ctx.clone(),
            GraphNode {
                id: "A".to_string(),
                label: "L".to_string(),
                properties: json!({}),
                tenant_id: tenant_id.to_string(),
            },
        )
        .await
        .unwrap();
    store
        .add_node(
            ctx.clone(),
            GraphNode {
                id: "B".to_string(),
                label: "L".to_string(),
                properties: json!({}),
                tenant_id: tenant_id.to_string(),
            },
        )
        .await
        .unwrap();
    store
        .add_node(
            ctx.clone(),
            GraphNode {
                id: "C".to_string(),
                label: "L".to_string(),
                properties: json!({}),
                tenant_id: tenant_id.to_string(),
            },
        )
        .await
        .unwrap();

    store
        .add_edge(
            ctx.clone(),
            GraphEdge {
                id: "E1".to_string(),
                source_id: "A".to_string(),
                target_id: "B".to_string(),
                relation: "R".to_string(),
                properties: json!({}),
                tenant_id: tenant_id.to_string(),
            },
        )
        .await
        .unwrap();
    store
        .add_edge(
            ctx.clone(),
            GraphEdge {
                id: "E2".to_string(),
                source_id: "B".to_string(),
                target_id: "C".to_string(),
                relation: "R".to_string(),
                properties: json!({}),
                tenant_id: tenant_id.to_string(),
            },
        )
        .await
        .unwrap();

    let path = store.shortest_path(ctx, "A", "C", Some(5)).unwrap();
    assert_eq!(path.len(), 2);
    assert_eq!(path[0].id, "E1");
    assert_eq!(path[1].id, "E2");
}

#[tokio::test]
async fn test_duckdb_health_and_readiness() {
    let config = DuckDbGraphConfig::default();
    let store = DuckDbGraphStore::new(config).expect("Failed to create DuckDbGraphStore");

    let health = store.health_check();
    assert!(health.healthy);
    assert!(health.duckdb.is_healthy);

    let readiness = store.readiness_check();
    assert!(readiness.ready);
    assert!(readiness.duckdb_ready);
    assert!(readiness.schema_ready);
}

#[tokio::test]
async fn test_duckdb_cold_start_metrics() {
    let config = DuckDbGraphConfig::default();
    let store = DuckDbGraphStore::new(config).expect("Failed to create DuckDbGraphStore");
    let tenant_id = "test-tenant";

    store
        .record_partition_access(tenant_id, "part1", 10.5)
        .unwrap();
    let records = store.get_partition_access_records(tenant_id).unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].partition_key, "part1");
    assert_eq!(records[0].access_count, 1);

    let prewarm = store.get_prewarm_partitions(tenant_id).unwrap();
    assert_eq!(prewarm, vec!["part1"]);
}

#[tokio::test]
async fn test_duckdb_parquet_export_import() {
    let config = DuckDbGraphConfig::default();
    let store = DuckDbGraphStore::new(config).expect("Failed to create DuckDbGraphStore");
    let tenant_id = "test-tenant";
    let ctx = TenantContext::new(
        TenantId::from_str(tenant_id).unwrap(),
        UserId::from_str("user-1").unwrap(),
    );

    store
        .add_node(
            ctx.clone(),
            GraphNode {
                id: "A".to_string(),
                label: "L".to_string(),
                properties: json!({"k": "v"}),
                tenant_id: tenant_id.to_string(),
            },
        )
        .await
        .unwrap();

    let parquet_data = store.export_to_parquet(tenant_id).unwrap();
    assert!(!parquet_data.is_empty());

    let store2 = DuckDbGraphStore::new(DuckDbGraphConfig::default()).unwrap();
    store2
        .import_from_parquet(tenant_id, &parquet_data)
        .unwrap();

    let stats = store2.get_stats(ctx).unwrap();
    assert_eq!(stats.node_count, 1);
}

#[tokio::test]
async fn test_duckdb_community_detection() {
    let config = DuckDbGraphConfig::default();
    let store = DuckDbGraphStore::new(config).expect("Failed to create DuckDbGraphStore");
    let tenant_id = "test-tenant";
    let ctx = TenantContext::new(
        TenantId::from_str(tenant_id).unwrap(),
        UserId::from_str("user-1").unwrap(),
    );

    store
        .add_node(
            ctx.clone(),
            GraphNode {
                id: "A".to_string(),
                label: "L".to_string(),
                properties: json!({}),
                tenant_id: tenant_id.to_string(),
            },
        )
        .await
        .unwrap();
    store
        .add_node(
            ctx.clone(),
            GraphNode {
                id: "B".to_string(),
                label: "L".to_string(),
                properties: json!({}),
                tenant_id: tenant_id.to_string(),
            },
        )
        .await
        .unwrap();
    store
        .add_node(
            ctx.clone(),
            GraphNode {
                id: "C".to_string(),
                label: "L".to_string(),
                properties: json!({}),
                tenant_id: tenant_id.to_string(),
            },
        )
        .await
        .unwrap();

    store
        .add_edge(
            ctx.clone(),
            GraphEdge {
                id: "E1".to_string(),
                source_id: "A".to_string(),
                target_id: "B".to_string(),
                relation: "R".to_string(),
                properties: json!({}),
                tenant_id: tenant_id.to_string(),
            },
        )
        .await
        .unwrap();
    store
        .add_edge(
            ctx.clone(),
            GraphEdge {
                id: "E2".to_string(),
                source_id: "B".to_string(),
                target_id: "C".to_string(),
                relation: "R".to_string(),
                properties: json!({}),
                tenant_id: tenant_id.to_string(),
            },
        )
        .await
        .unwrap();
    store
        .add_edge(
            ctx.clone(),
            GraphEdge {
                id: "E3".to_string(),
                source_id: "C".to_string(),
                target_id: "A".to_string(),
                relation: "R".to_string(),
                properties: json!({}),
                tenant_id: tenant_id.to_string(),
            },
        )
        .await
        .unwrap();

    let communities = store.detect_communities(ctx, 2).unwrap();
    assert_eq!(communities.len(), 1);
    assert_eq!(communities[0].member_node_ids.len(), 3);
    assert!(communities[0].density >= 1.0);
}

#[tokio::test]
async fn test_duckdb_entity_crud_and_linking() {
    let config = DuckDbGraphConfig::default();
    let store = DuckDbGraphStore::new(config).expect("Failed to create DuckDbGraphStore");
    let tenant_id = "test-tenant";
    let ctx = TenantContext::new(
        TenantId::from_str(tenant_id).unwrap(),
        UserId::from_str("user-1").unwrap(),
    );

    let entity1 = Entity {
        id: "e1".to_string(),
        name: "Entity 1".to_string(),
        entity_type: "Concept".to_string(),
        properties: json!({"importance": "high"}),
        tenant_id: tenant_id.to_string(),
        created_at: Utc::now(),
        deleted_at: None,
    };

    let entity2 = Entity {
        id: "e2".to_string(),
        name: "Entity 2".to_string(),
        entity_type: "Concept".to_string(),
        properties: json!({"importance": "medium"}),
        tenant_id: tenant_id.to_string(),
        created_at: Utc::now(),
        deleted_at: None,
    };

    store.add_entity(ctx.clone(), entity1).unwrap();
    store.add_entity(ctx.clone(), entity2).unwrap();

    store
        .link_entities(ctx.clone(), "e1", "e2", "RELATES_TO", Some(json!({})))
        .unwrap();

    let stats = store.get_stats(ctx.clone()).unwrap();
    assert_eq!(stats.entity_count, 2);
    assert_eq!(stats.entity_edge_count, 1);
}

#[tokio::test]
async fn test_duckdb_referential_integrity_violation() {
    let config = DuckDbGraphConfig::default();
    let store = DuckDbGraphStore::new(config).expect("Failed to create DuckDbGraphStore");
    let tenant_str = "test_tenant";
    let tenant_id = TenantId::from_str(tenant_str).unwrap();
    let ctx = TenantContext::new(tenant_id, UserId::from_str("user-1").unwrap());

    let edge = GraphEdge {
        id: "edge1".to_string(),
        source_id: "non-existent-source".to_string(),
        target_id: "non-existent-target".to_string(),
        relation: "RELATES".to_string(),
        properties: json!({}),
        tenant_id: tenant_str.to_string(),
    };

    let result = store.add_edge(ctx.clone(), edge).await;
    assert!(result.is_err());
    match result {
        Err(err) => {
            if let Some(graph_err) = err.downcast_ref::<storage::graph_duckdb::GraphError>() {
                match graph_err {
                    storage::graph_duckdb::GraphError::ReferentialIntegrity(msg) => {
                        assert!(msg.contains("Source node"));
                    }
                    _ => panic!("Expected ReferentialIntegrity error, got {:?}", graph_err),
                }
            } else {
                panic!("Expected GraphError, got {:?}", err);
            }
        }
        Ok(_) => panic!("Expected error, got Ok"),
    }

    store
        .add_node(
            ctx.clone(),
            GraphNode {
                id: "source".to_string(),
                label: "L".to_string(),
                properties: json!({}),
                tenant_id: tenant_str.to_string(),
            },
        )
        .await
        .unwrap();

    let edge2 = GraphEdge {
        id: "edge2".to_string(),
        source_id: "source".to_string(),
        target_id: "non-existent-target".to_string(),
        relation: "RELATES".to_string(),
        properties: json!({}),
        tenant_id: tenant_str.to_string(),
    };

    let result2 = store.add_edge(ctx, edge2).await;
    assert!(result2.is_err());
    match result2 {
        Err(err) => {
            if let Some(graph_err) = err.downcast_ref::<storage::graph_duckdb::GraphError>() {
                match graph_err {
                    storage::graph_duckdb::GraphError::ReferentialIntegrity(msg) => {
                        assert!(msg.contains("Target node"));
                    }
                    _ => panic!("Expected ReferentialIntegrity error, got {:?}", graph_err),
                }
            } else {
                panic!("Expected GraphError, got {:?}", err);
            }
        }
        Ok(_) => panic!("Expected error, got Ok"),
    }
}

#[tokio::test]
async fn test_duckdb_search_edge_cases() {
    let config = DuckDbGraphConfig::default();
    let store = DuckDbGraphStore::new(config).expect("Failed to create DuckDbGraphStore");
    let tenant_str = "test_tenant";
    let tenant_id = TenantId::from_str(tenant_str).unwrap();
    let ctx = TenantContext::new(tenant_id, UserId::from_str("user-1").unwrap());

    store
        .add_node(
            ctx.clone(),
            GraphNode {
                id: "node1".to_string(),
                label: "Node".to_string(),
                properties: json!({"desc": "Special characters like % _ and ' quotes"}),
                tenant_id: tenant_str.to_string(),
            },
        )
        .await
        .unwrap();

    let results = store.search_nodes(ctx.clone(), "quotes", 10).await.unwrap();
    assert_eq!(results.len(), 1);

    let results = store
        .search_nodes(ctx.clone(), "non-existent", 10)
        .await
        .unwrap();
    assert!(results.is_empty());

    let results = store.search_nodes(ctx.clone(), "", 10).await.unwrap();
    assert_eq!(results.len(), 1);
}

#[tokio::test]
async fn test_duckdb_detect_communities_multi_cluster() {
    let config = DuckDbGraphConfig::default();
    let store = DuckDbGraphStore::new(config).expect("Failed to create DuckDbGraphStore");
    let tenant_id = "test-tenant";
    let ctx = TenantContext::new(
        TenantId::from_str(tenant_id).unwrap(),
        UserId::from_str("user-1").unwrap(),
    );

    store
        .add_node(
            ctx.clone(),
            GraphNode {
                id: "A1".to_string(),
                label: "L".to_string(),
                properties: json!({}),
                tenant_id: tenant_id.to_string(),
            },
        )
        .await
        .unwrap();
    store
        .add_node(
            ctx.clone(),
            GraphNode {
                id: "A2".to_string(),
                label: "L".to_string(),
                properties: json!({}),
                tenant_id: tenant_id.to_string(),
            },
        )
        .await
        .unwrap();
    store
        .add_edge(
            ctx.clone(),
            GraphEdge {
                id: "E1".to_string(),
                source_id: "A1".to_string(),
                target_id: "A2".to_string(),
                relation: "R".to_string(),
                properties: json!({}),
                tenant_id: tenant_id.to_string(),
            },
        )
        .await
        .unwrap();

    store
        .add_node(
            ctx.clone(),
            GraphNode {
                id: "B1".to_string(),
                label: "L".to_string(),
                properties: json!({}),
                tenant_id: tenant_id.to_string(),
            },
        )
        .await
        .unwrap();
    store
        .add_node(
            ctx.clone(),
            GraphNode {
                id: "B2".to_string(),
                label: "L".to_string(),
                properties: json!({}),
                tenant_id: tenant_id.to_string(),
            },
        )
        .await
        .unwrap();
    store
        .add_edge(
            ctx.clone(),
            GraphEdge {
                id: "E2".to_string(),
                source_id: "B1".to_string(),
                target_id: "B2".to_string(),
                relation: "R".to_string(),
                properties: json!({}),
                tenant_id: tenant_id.to_string(),
            },
        )
        .await
        .unwrap();

    let communities = store.detect_communities(ctx, 2).unwrap();
    assert!(communities.len() >= 2);
}

#[tokio::test]
async fn test_duckdb_tenant_isolation_atomic() {
    let config = DuckDbGraphConfig::default();
    let store = DuckDbGraphStore::new(config).expect("Failed to create DuckDbGraphStore");

    let tenant1 = "tenant1";
    let ctx1 = TenantContext::new(
        TenantId::from_str(tenant1).unwrap(),
        UserId::from_str("user-1").unwrap(),
    );

    let nodes1 = vec![GraphNode {
        id: "n1".to_string(),
        label: "L".to_string(),
        properties: json!({}),
        tenant_id: tenant1.to_string(),
    }];

    store
        .add_nodes_and_edges_atomic(&ctx1, tenant1, nodes1, vec![])
        .unwrap();

    let tenant2 = "tenant2";
    let ctx2 = TenantContext::new(
        TenantId::from_str(tenant2).unwrap(),
        UserId::from_str("user-1").unwrap(),
    );

    let nodes2 = vec![GraphNode {
        id: "n1".to_string(),
        label: "L".to_string(),
        properties: json!({}),
        tenant_id: tenant1.to_string(),
    }];

    let result = store.add_nodes_and_edges_atomic(&ctx2, tenant2, nodes2, vec![]);
    assert!(result.is_err());

    let stats2 = store.get_stats(ctx2).unwrap();
    assert_eq!(stats2.node_count, 0);
}

#[tokio::test]
async fn test_duckdb_soft_delete_node_not_found() {
    let config = DuckDbGraphConfig::default();
    let store = DuckDbGraphStore::new(config).expect("Failed to create DuckDbGraphStore");
    let tenant_id = "test-tenant";
    let ctx = TenantContext::new(
        TenantId::from_str(tenant_id).unwrap(),
        UserId::from_str("user-1").unwrap(),
    );

    let result = store.soft_delete_node(ctx, "non-existent");
    assert!(result.is_err());
    match result {
        Err(storage::graph_duckdb::GraphError::NodeNotFound(_)) => {}
        _ => panic!("Expected NodeNotFound, got {:?}", result),
    }
}

#[tokio::test]
async fn test_duckdb_find_path_max_depth_exceeded() {
    let config = DuckDbGraphConfig {
        max_path_depth: 1,
        ..Default::default()
    };
    let store = DuckDbGraphStore::new(config).expect("Failed to create DuckDbGraphStore");
    let tenant_id = "test-tenant";
    let ctx = TenantContext::new(
        TenantId::from_str(tenant_id).unwrap(),
        UserId::from_str("user-1").unwrap(),
    );

    let result = store.find_path(ctx.clone(), "A", "B", 5).await;
    assert!(result.is_err());
    match result {
        Err(err) => {
            if let Some(graph_err) = err.downcast_ref::<storage::graph_duckdb::GraphError>() {
                match graph_err {
                    storage::graph_duckdb::GraphError::MaxDepthExceeded(_) => {}
                    _ => panic!("Expected MaxDepthExceeded, got {:?}", graph_err),
                }
            } else {
                panic!("Expected GraphError, got {:?}", err);
            }
        }
        Ok(_) => panic!("Expected error, got Ok"),
    }

    let result_shortest = store.shortest_path(ctx, "A", "B", Some(5));
    assert!(result_shortest.is_err());
    match result_shortest {
        Err(storage::graph_duckdb::GraphError::MaxDepthExceeded(_)) => {}
        _ => panic!(
            "Expected MaxDepthExceeded for shortest_path, got {:?}",
            result_shortest
        ),
    }
}

#[tokio::test]
async fn test_duckdb_s3_not_configured() {
    let config = DuckDbGraphConfig::default(); // S3 is None by default
    let store = DuckDbGraphStore::new(config).expect("Failed to create DuckDbGraphStore");
    let tenant_id = "test-tenant";

    let result = store.persist_to_s3(tenant_id).await;
    assert!(result.is_err());
    match result {
        Err(storage::graph_duckdb::GraphError::S3(msg)) => {
            assert!(msg.contains("S3 bucket not configured"));
        }
        _ => panic!("Expected S3 error, got {:?}", result),
    }
}

#[tokio::test]
async fn test_duckdb_serialization_error() {
    let config = DuckDbGraphConfig::default();
    let store = DuckDbGraphStore::new(config).expect("Failed to create DuckDbGraphStore");
    let tenant_id = "test-tenant";
    let ctx = TenantContext::new(
        TenantId::from_str(tenant_id).unwrap(),
        UserId::from_str("user-1").unwrap(),
    );

    let node = GraphNode {
        id: "TRIGGER_SERIALIZATION_ERROR".to_string(),
        label: "L".to_string(),
        properties: json!({}),
        tenant_id: tenant_id.to_string(),
    };

    let result = store.add_node(ctx.clone(), node).await;
    assert!(result.is_err());
    match result {
        Err(err) => {
            if let Some(graph_err) = err.downcast_ref::<storage::graph_duckdb::GraphError>() {
                match graph_err {
                    storage::graph_duckdb::GraphError::Serialization(_) => {}
                    _ => panic!("Expected Serialization error, got {:?}", graph_err),
                }
            } else {
                panic!("Expected GraphError, got {:?}", err);
            }
        }
        Ok(_) => panic!("Expected error, got Ok"),
    }

    let entities = vec![Entity {
        id: "TRIGGER_SERIALIZATION_ERROR".to_string(),
        name: "E1".to_string(),
        entity_type: "T".to_string(),
        properties: json!({}),
        tenant_id: tenant_id.to_string(),
        created_at: Utc::now(),
        deleted_at: None,
    }];
    let result = store.add_entities_atomic(&ctx, tenant_id, entities, vec![]);
    assert!(result.is_err());
}

#[tokio::test]
async fn test_duckdb_checksum_mismatch() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    unsafe {
        std::env::set_var("AWS_ACCESS_KEY_ID", "test");
        std::env::set_var("AWS_SECRET_ACCESS_KEY", "test");
    }

    let server = MockServer::start().await;
    let bucket = "test-bucket";
    let tenant_id = "TRIGGER_CHECKSUM_ERROR";
    let snapshot_key = "snapshot.parquet";

    Mock::given(method("GET"))
        .and(path(format!("/{}/{}", bucket, snapshot_key)))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(vec![1, 2, 3])
                .insert_header("x-amz-meta-checksum", "valid_checksum"),
        )
        .mount(&server)
        .await;

    let config = DuckDbGraphConfig {
        s3_bucket: Some(bucket.to_string()),
        s3_endpoint: Some(server.uri()),
        s3_region: Some("us-east-1".to_string()),
        ..Default::default()
    };
    let store = DuckDbGraphStore::new(config).unwrap();

    let result = store.load_from_s3(tenant_id, snapshot_key).await;
    assert!(result.is_err());
    match result {
        Err(storage::graph_duckdb::GraphError::ChecksumMismatch { .. }) => {}
        _ => panic!("Expected ChecksumMismatch, got {:?}", result),
    }
}

#[tokio::test]
async fn test_duckdb_s3_atomicity() {
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    unsafe {
        std::env::set_var("AWS_ACCESS_KEY_ID", "test");
        std::env::set_var("AWS_SECRET_ACCESS_KEY", "test");
    }

    let server = MockServer::start().await;
    let bucket = "test-bucket";
    let tenant_id = "TRIGGER_S3_COMMIT_ERROR";

    Mock::given(method("PUT"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    Mock::given(method("DELETE"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let config = DuckDbGraphConfig {
        s3_bucket: Some(bucket.to_string()),
        s3_endpoint: Some(server.uri()),
        s3_region: Some("us-east-1".to_string()),
        ..Default::default()
    };
    let store = DuckDbGraphStore::new(config).unwrap();

    let result = store.persist_to_s3(tenant_id).await;
    assert!(result.is_err());
    match result {
        Err(storage::graph_duckdb::GraphError::S3(msg))
            if msg.contains("Induced commit failure") => {}
        _ => panic!("Expected S3 induced commit failure, got {:?}", result),
    }
}
#[tokio::test]
async fn test_duckdb_edge_not_found() {
    let config = DuckDbGraphConfig::default();
    let store = DuckDbGraphStore::new(config).expect("Failed to create DuckDbGraphStore");
    let tenant_id = "test-tenant";
    let ctx = TenantContext::new(
        TenantId::from_str(tenant_id).unwrap(),
        UserId::from_str("user-1").unwrap(),
    );

    let neighbors = store.get_neighbors(ctx, "non-existent").await.unwrap();
    assert!(neighbors.is_empty());
}
#[tokio::test]
async fn test_duckdb_invalid_tenant_context() {
    let config = DuckDbGraphConfig::default();
    let store = DuckDbGraphStore::new(config).expect("Failed to create DuckDbGraphStore");

    let tenant1 = "tenant1";
    let tenant2 = "tenant2";
    let ctx = TenantContext::new(
        TenantId::from_str(tenant1).unwrap(),
        UserId::from_str("user-1").unwrap(),
    );

    let node = GraphNode {
        id: "n1".to_string(),
        label: "L".to_string(),
        properties: json!({}),
        tenant_id: tenant2.to_string(),
    };

    let result = store.add_node(ctx, node).await;
    assert!(result.is_err());
    match result {
        Err(err) => {
            if let Some(graph_err) = err.downcast_ref::<storage::graph_duckdb::GraphError>() {
                match graph_err {
                    storage::graph_duckdb::GraphError::TenantViolation(_) => {}
                    _ => panic!("Expected TenantViolation, got {:?}", graph_err),
                }
            } else {
                panic!("Expected GraphError, got {:?}", err);
            }
        }
        Ok(_) => panic!("Expected error, got Ok"),
    }
}

#[tokio::test]
async fn test_duckdb_tenant_validation_comprehensive() {
    let _store = DuckDbGraphStore::new(DuckDbGraphConfig::default()).unwrap();

    let long_tid = "a".repeat(129);
    let invalid_tenants = vec![
        "tenant' OR '1'='1",
        "tenant; DROP TABLE memory_nodes",
        "tenant--",
        "tenant/*",
        "tenant\0",
        &long_tid,
        "",
        "tenant$name",
    ];

    for tid in invalid_tenants {
        let result = DuckDbGraphStore::validate_tenant_id_format(tid);
        assert!(result.is_err(), "Tenant ID '{}' should be invalid", tid);
    }

    let long_valid_tid = "a".repeat(128);
    let valid_tenants = vec![
        "tenant123",
        "tenant_name",
        "tenant-name",
        "T123-ABC_xyz",
        "a",
        &long_valid_tid,
    ];

    for tid in valid_tenants {
        let result = DuckDbGraphStore::validate_tenant_id_format(tid);
        assert!(result.is_ok(), "Tenant ID '{}' should be valid", tid);
    }
}

#[test]
fn test_duckdb_tenant_validation_unicode_and_boundaries() {
    let _store = DuckDbGraphStore::new(DuckDbGraphConfig::default()).unwrap();

    // Test exact boundary cases
    let exactly_128_chars = "a".repeat(128);
    let exactly_127_chars = "a".repeat(127);
    let exactly_129_chars = "a".repeat(129);

    assert!(
        DuckDbGraphStore::validate_tenant_id_format(&exactly_128_chars).is_ok(),
        "Exactly 128 characters should be valid"
    );
    assert!(
        DuckDbGraphStore::validate_tenant_id_format(&exactly_127_chars).is_ok(),
        "Exactly 127 characters should be valid"
    );
    assert!(
        DuckDbGraphStore::validate_tenant_id_format(&exactly_129_chars).is_err(),
        "Exactly 129 characters should be invalid"
    );

    // Test Unicode characters
    // Unicode letters (e.g., café, αβγ, 北京) are allowed by is_alphanumeric()
    // Non-letter Unicode (e.g., emojis 🎉😀) are not allowed
    let unicode_test_cases = vec![
        ("tenant- café", false),     // space should fail
        ("tenant- café-123", false), // space should fail
        ("café", true),              // Unicode letters should pass
        ("tenant-🎉", false),        // emoji should fail
        ("tenant-😀", false),        // emoji should fail
        ("tenant-αβγ", true),        // Greek letters should pass
        ("tenant-北京", true),       // Chinese characters should pass
    ];

    for (tid, should_pass) in unicode_test_cases {
        let result = DuckDbGraphStore::validate_tenant_id_format(tid);
        if should_pass {
            assert!(
                result.is_ok(),
                "Unicode tenant ID '{}' should be valid (contains only letters)",
                tid
            );
        } else {
            assert!(result.is_err(), "Tenant ID '{}' should be invalid", tid);
        }
    }

    // Test mixed valid characters at boundaries
    let mixed_valid_128 = format!("{}{}", "a".repeat(120), "-_123ABC");
    assert!(
        DuckDbGraphStore::validate_tenant_id_format(&mixed_valid_128).is_ok(),
        "Mixed valid characters at 128 length should be valid"
    );

    // Test edge cases with special characters at boundaries
    let edge_cases = vec![
        "-tenant",  // Starting with hyphen
        "_tenant",  // Starting with underscore
        "tenant-",  // Ending with hyphen
        "tenant_",  // Ending with underscore
        "ten__ant", // Double underscore
        "ten-_ant", // Mixed hyphen-underscore
        "ten_-ant", // Mixed underscore-hyphen
    ];

    for tid in edge_cases {
        let result = DuckDbGraphStore::validate_tenant_id_format(tid);
        assert!(
            result.is_ok(),
            "Edge case tenant ID '{}' should be valid",
            tid
        );
    }

    // Test invalid edge cases (SQL injection patterns)
    let invalid_edge_cases = vec![
        "ten--ant", // Double hyphen (SQL comment)
        "ten..ant", // Double dot
        "ten.-ant", // Dot-hyphen
        "ten-.ant", // Hyphen-dot
        "ten.ant.", // Ending with dot
        ".tenant",  // Starting with dot
    ];

    for tid in invalid_edge_cases {
        let result = DuckDbGraphStore::validate_tenant_id_format(tid);
        assert!(
            result.is_err(),
            "Invalid edge case tenant ID '{}' should be invalid",
            tid
        );
    }
}

#[tokio::test]
async fn test_duckdb_lazy_load_partition_error() {
    let config = DuckDbGraphConfig {
        s3_bucket: Some("test-bucket".to_string()),
        ..Default::default()
    };
    let store = DuckDbGraphStore::new(config).expect("Failed to create DuckDbGraphStore");
    let tenant_id = "TRIGGER_S3_PARTITION_ERROR";

    store
        .record_partition_access(tenant_id, "part1", 10.5)
        .unwrap();

    let result = store
        .lazy_load_partitions(tenant_id, &["part1".to_string()])
        .await
        .unwrap();
    assert_eq!(result.partitions_loaded, 0);
    assert_eq!(result.deferred_partitions.len(), 1);
    assert_eq!(result.deferred_partitions[0], "part1");
}

#[tokio::test]
async fn test_duckdb_detect_communities_empty_graph() {
    let config = DuckDbGraphConfig::default();
    let store = DuckDbGraphStore::new(config).expect("Failed to create DuckDbGraphStore");
    let tenant_id = "test-tenant-empty";
    let ctx = TenantContext::new(
        TenantId::from_str(tenant_id).unwrap(),
        UserId::from_str("user-1").unwrap(),
    );

    // Test with empty graph (no nodes)
    let communities = store.detect_communities(ctx.clone(), 2).unwrap();
    assert!(
        communities.is_empty(),
        "Empty graph should have no communities"
    );

    // Test with single node (no edges)
    store
        .add_node(
            ctx.clone(),
            GraphNode {
                id: "single-node".to_string(),
                label: "L".to_string(),
                properties: json!({}),
                tenant_id: tenant_id.to_string(),
            },
        )
        .await
        .unwrap();

    let communities = store.detect_communities(ctx.clone(), 2).unwrap();
    assert!(
        communities.is_empty(),
        "Single node with min_community_size=2 should not form a community"
    );

    // Test with min_community_size=1 to verify single nodes can form communities
    let communities_single = store.detect_communities(ctx, 1).unwrap();
    assert_eq!(
        communities_single.len(),
        1,
        "Single node with min_community_size=1 should form its own community"
    );
    assert_eq!(communities_single[0].member_node_ids.len(), 1);
    assert_eq!(communities_single[0].member_node_ids[0], "single-node");
    assert_eq!(
        communities_single[0].density, 0.0,
        "Single node community should have zero density"
    );
}

#[tokio::test]
async fn test_duckdb_concurrent_readers_plus_writer_qps_scaling() {
    use std::sync::Arc;
    use std::time::Instant;

    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("concurrent_test.duckdb");

    let config = DuckDbGraphConfig {
        path: db_path.to_str().unwrap().to_string(),
        reader_pool_size: Some(4),
        ..Default::default()
    };
    let store = Arc::new(DuckDbGraphStore::new(config).expect("create file-backed store"));

    let tenant = "tenant-concurrent-rw";
    let ctx = TenantContext::new(
        TenantId::from_str(tenant).unwrap(),
        UserId::from_str("user-1").unwrap(),
    );

    for i in 0..50 {
        store
            .add_node(
                ctx.clone(),
                GraphNode {
                    id: format!("seed-{i}"),
                    label: "Seed".to_string(),
                    properties: json!({"i": i}),
                    tenant_id: tenant.to_string(),
                },
            )
            .await
            .unwrap();
    }

    let reader_count = 4u32;
    let reads_per_reader = 50u32;
    let writes = 20u32;

    let store_w = Arc::clone(&store);
    let writer_handle = tokio::spawn(async move {
        let wctx = TenantContext::new(
            TenantId::from_str("tenant-concurrent-rw").unwrap(),
            UserId::from_str("user-1").unwrap(),
        );
        for i in 0..writes {
            store_w
                .add_node(
                    wctx.clone(),
                    GraphNode {
                        id: format!("write-{i}"),
                        label: "Written".to_string(),
                        properties: json!({}),
                        tenant_id: "tenant-concurrent-rw".to_string(),
                    },
                )
                .await
                .unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    });

    let start = Instant::now();
    let mut reader_handles = Vec::new();
    for _ in 0..reader_count {
        let s = Arc::clone(&store);
        let rctx = ctx.clone();
        reader_handles.push(tokio::spawn(async move {
            for _ in 0..reads_per_reader {
                let _ = s.get_stats(rctx.clone()).unwrap();
            }
        }));
    }

    for h in reader_handles {
        h.await.unwrap();
    }
    let read_elapsed = start.elapsed();
    writer_handle.await.unwrap();

    let total_reads = reader_count * reads_per_reader;
    let reads_per_sec = total_reads as f64 / read_elapsed.as_secs_f64();

    // After writer completes, a fresh writer-path read sees all nodes.
    let final_count = {
        let conn = store.db_handle();
        let conn = conn.lock();
        conn.query_row(
            "SELECT COUNT(*) FROM memory_nodes WHERE tenant_id = 'tenant-concurrent-rw' AND deleted_at IS NULL",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap()
    };
    assert_eq!(final_count, 70);

    assert!(
        reads_per_sec > 10.0,
        "Reader QPS too low: {reads_per_sec:.0} ops/sec with {reader_count} readers"
    );
}

#[tokio::test]
async fn test_duckdb_round_trip_full_plus_deltas_restore() {
    let store = DuckDbGraphStore::new(DuckDbGraphConfig::default()).expect("create store");
    let tenant = "tenant-roundtrip";
    let ctx = TenantContext::new(
        TenantId::from_str(tenant).unwrap(),
        UserId::from_str("user-1").unwrap(),
    );

    store
        .add_node(
            ctx.clone(),
            GraphNode {
                id: "node-a".to_string(),
                label: "Alpha".to_string(),
                properties: json!({"v": 1}),
                tenant_id: tenant.to_string(),
            },
        )
        .await
        .unwrap();
    store
        .add_node(
            ctx.clone(),
            GraphNode {
                id: "node-b".to_string(),
                label: "Beta".to_string(),
                properties: json!({"v": 2}),
                tenant_id: tenant.to_string(),
            },
        )
        .await
        .unwrap();

    let full_data = store.export_to_parquet(tenant).unwrap();
    assert!(!full_data.is_empty());

    let conn = store.db_handle();
    let cutoff_seq: i64 = conn
        .lock()
        .query_row(
            "SELECT MAX(seq) FROM memory_nodes WHERE tenant_id = ?",
            [tenant],
            |row| row.get(0),
        )
        .unwrap();

    store
        .add_node(
            ctx.clone(),
            GraphNode {
                id: "node-c".to_string(),
                label: "Gamma".to_string(),
                properties: json!({"v": 3}),
                tenant_id: tenant.to_string(),
            },
        )
        .await
        .unwrap();
    store
        .add_edge(
            ctx.clone(),
            GraphEdge {
                id: "edge-ac".to_string(),
                source_id: "node-a".to_string(),
                target_id: "node-c".to_string(),
                relation: "LINKS".to_string(),
                properties: json!({}),
                tenant_id: tenant.to_string(),
            },
        )
        .await
        .unwrap();

    let (delta_data, delta_rows, delta_to_seq) =
        store.export_delta_to_parquet(tenant, cutoff_seq).unwrap();
    assert_eq!(delta_rows, 2);
    assert!(delta_to_seq > cutoff_seq);

    let store2 = DuckDbGraphStore::new(DuckDbGraphConfig::default()).unwrap();
    store2.import_from_parquet(tenant, &full_data).unwrap();

    let ctx2 = TenantContext::new(
        TenantId::from_str(tenant).unwrap(),
        UserId::from_str("user-1").unwrap(),
    );
    let stats_after_full = store2.get_stats(ctx2.clone()).unwrap();
    assert_eq!(stats_after_full.node_count, 2);
    assert_eq!(stats_after_full.edge_count, 0);

    store2
        .import_delta_from_parquet(tenant, &delta_data)
        .unwrap();

    let stats_after_delta = store2.get_stats(ctx2.clone()).unwrap();
    assert_eq!(stats_after_delta.node_count, 3);
    assert_eq!(stats_after_delta.edge_count, 1);

    let neighbors = store2.get_neighbors(ctx2, "node-a").await.unwrap();
    assert_eq!(neighbors.len(), 1);
    assert_eq!(neighbors[0].1.id, "node-c");
}

#[tokio::test]
async fn test_duckdb_delta_with_no_events_is_noop() {
    let store = DuckDbGraphStore::new(DuckDbGraphConfig::default()).expect("create store");
    let tenant = "tenant-noop";
    let ctx = TenantContext::new(
        TenantId::from_str(tenant).unwrap(),
        UserId::from_str("user-1").unwrap(),
    );

    store
        .add_node(
            ctx.clone(),
            GraphNode {
                id: "node-x".to_string(),
                label: "Only".to_string(),
                properties: json!({}),
                tenant_id: tenant.to_string(),
            },
        )
        .await
        .unwrap();

    let conn = store.db_handle();
    let max_seq: i64 = conn
        .lock()
        .query_row(
            "SELECT MAX(seq) FROM memory_nodes WHERE tenant_id = ?",
            [tenant],
            |row| row.get(0),
        )
        .unwrap();

    let (delta_data, delta_rows, to_seq) = store.export_delta_to_parquet(tenant, max_seq).unwrap();
    assert_eq!(delta_rows, 0);
    assert_eq!(to_seq, max_seq);

    let store2 = DuckDbGraphStore::new(DuckDbGraphConfig::default()).unwrap();
    store2
        .import_from_parquet(tenant, &store.export_to_parquet(tenant).unwrap())
        .unwrap();
    store2
        .import_delta_from_parquet(tenant, &delta_data)
        .unwrap();

    let ctx2 = TenantContext::new(
        TenantId::from_str(tenant).unwrap(),
        UserId::from_str("user-1").unwrap(),
    );
    let stats = store2.get_stats(ctx2).unwrap();
    assert_eq!(stats.node_count, 1);
}
