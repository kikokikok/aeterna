use mk_core::types::{TenantContext, TenantId, UserId};
use storage::graph::{GraphEdge, GraphNode};
use storage::graph_duckdb::{DuckDbGraphConfig, DuckDbGraphStore, Entity, EntityEdge, GraphError};

fn create_test_context(tenant_id: &str) -> TenantContext {
    TenantContext::new(
        TenantId::new(tenant_id.to_string()).unwrap(),
        UserId::new("test-user".to_string()).unwrap(),
    )
}

#[test]
fn test_atomic_nodes_and_edges_success() {
    let store = DuckDbGraphStore::new(DuckDbGraphConfig::default()).unwrap();
    let ctx = create_test_context("tenant-1");
    let tenant_id = "tenant-1";

    let nodes = vec![
        GraphNode {
            id: "node-1".to_string(),
            label: "test".to_string(),
            properties: serde_json::json!({"key": "value1"}),
            tenant_id: tenant_id.to_string(),
        },
        GraphNode {
            id: "node-2".to_string(),
            label: "test".to_string(),
            properties: serde_json::json!({"key": "value2"}),
            tenant_id: tenant_id.to_string(),
        },
    ];

    let edges = vec![GraphEdge {
        id: "edge-1".to_string(),
        source_id: "node-1".to_string(),
        target_id: "node-2".to_string(),
        relation: "connects".to_string(),
        properties: serde_json::json!({}),
        tenant_id: tenant_id.to_string(),
    }];

    let result = store.add_nodes_and_edges_atomic(&ctx, tenant_id, nodes, edges);
    assert!(result.is_ok(), "Atomic insert should succeed: {:?}", result);

    let stats = store.get_stats(ctx.clone()).unwrap();
    assert_eq!(stats.node_count, 2);
    assert_eq!(stats.edge_count, 1);
}

#[test]
fn test_atomic_rollback_on_tenant_mismatch() {
    let store = DuckDbGraphStore::new(DuckDbGraphConfig::default()).unwrap();
    let ctx = create_test_context("tenant-1");
    let tenant_id = "tenant-1";

    let nodes = vec![
        GraphNode {
            id: "node-a".to_string(),
            label: "test".to_string(),
            properties: serde_json::json!({}),
            tenant_id: tenant_id.to_string(),
        },
        GraphNode {
            id: "node-b".to_string(),
            label: "test".to_string(),
            properties: serde_json::json!({}),
            tenant_id: "wrong-tenant".to_string(),
        },
    ];

    let result = store.add_nodes_and_edges_atomic(&ctx, tenant_id, nodes, vec![]);
    assert!(matches!(result, Err(GraphError::TenantViolation(_))));

    let stats = store.get_stats(ctx).unwrap();
    assert_eq!(
        stats.node_count, 0,
        "Rollback should have removed all nodes"
    );
}

#[test]
fn test_atomic_rollback_on_referential_integrity() {
    let store = DuckDbGraphStore::new(DuckDbGraphConfig::default()).unwrap();
    let ctx = create_test_context("tenant-1");
    let tenant_id = "tenant-1";

    let nodes = vec![GraphNode {
        id: "node-only".to_string(),
        label: "test".to_string(),
        properties: serde_json::json!({}),
        tenant_id: tenant_id.to_string(),
    }];

    let edges = vec![GraphEdge {
        id: "edge-bad".to_string(),
        source_id: "node-only".to_string(),
        target_id: "nonexistent-node".to_string(),
        relation: "broken".to_string(),
        properties: serde_json::json!({}),
        tenant_id: tenant_id.to_string(),
    }];

    let result = store.add_nodes_and_edges_atomic(&ctx, tenant_id, nodes, edges);
    assert!(
        matches!(result, Err(GraphError::ReferentialIntegrity(_))),
        "Should fail on missing target node: {:?}",
        result
    );

    let stats = store.get_stats(ctx).unwrap();
    assert_eq!(stats.node_count, 0, "Rollback should have removed the node");
    assert_eq!(stats.edge_count, 0);
}

#[test]
fn test_atomic_entities_success() {
    let store = DuckDbGraphStore::new(DuckDbGraphConfig::default()).unwrap();
    let ctx = create_test_context("tenant-1");
    let tenant_id = "tenant-1";

    let entities = vec![
        Entity {
            id: "entity-1".to_string(),
            name: "Entity One".to_string(),
            entity_type: "Person".to_string(),
            properties: serde_json::json!({}),
            tenant_id: tenant_id.to_string(),
            created_at: chrono::Utc::now(),
            deleted_at: None,
        },
        Entity {
            id: "entity-2".to_string(),
            name: "Entity Two".to_string(),
            entity_type: "Organization".to_string(),
            properties: serde_json::json!({}),
            tenant_id: tenant_id.to_string(),
            created_at: chrono::Utc::now(),
            deleted_at: None,
        },
    ];

    let entity_edges = vec![EntityEdge {
        id: "entity-edge-1".to_string(),
        source_entity_id: "entity-1".to_string(),
        target_entity_id: "entity-2".to_string(),
        relation: "works_for".to_string(),
        properties: serde_json::json!({}),
        tenant_id: tenant_id.to_string(),
        created_at: chrono::Utc::now(),
        deleted_at: None,
    }];

    let result = store.add_entities_atomic(&ctx, tenant_id, entities, entity_edges);
    assert!(
        result.is_ok(),
        "Atomic entity insert should succeed: {:?}",
        result
    );

    let stats = store.get_stats(ctx).unwrap();
    assert_eq!(stats.entity_count, 2);
    assert_eq!(stats.entity_edge_count, 1);
}

#[test]
fn test_atomic_entities_rollback_on_tenant_mismatch() {
    let store = DuckDbGraphStore::new(DuckDbGraphConfig::default()).unwrap();
    let ctx = create_test_context("tenant-1");
    let tenant_id = "tenant-1";

    let entities = vec![
        Entity {
            id: "entity-a".to_string(),
            name: "Good Entity".to_string(),
            entity_type: "Person".to_string(),
            properties: serde_json::json!({}),
            tenant_id: tenant_id.to_string(),
            created_at: chrono::Utc::now(),
            deleted_at: None,
        },
        Entity {
            id: "entity-b".to_string(),
            name: "Bad Entity".to_string(),
            entity_type: "Person".to_string(),
            properties: serde_json::json!({}),
            tenant_id: "wrong-tenant".to_string(),
            created_at: chrono::Utc::now(),
            deleted_at: None,
        },
    ];

    let result = store.add_entities_atomic(&ctx, tenant_id, entities, vec![]);
    assert!(matches!(result, Err(GraphError::TenantViolation(_))));

    let stats = store.get_stats(ctx).unwrap();
    assert_eq!(
        stats.entity_count, 0,
        "Rollback should have removed all entities"
    );
}

#[test]
fn test_with_transaction_commit() {
    let store = DuckDbGraphStore::new(DuckDbGraphConfig::default()).unwrap();

    let result = store.with_transaction(|conn| {
        conn.execute(
            "INSERT INTO memory_nodes (id, label, properties, tenant_id) VALUES (?, ?, ?, ?)",
            duckdb::params!["tx-node-1", "test", "{}", "tx-tenant"],
        )?;
        Ok(42)
    });

    assert_eq!(result.unwrap(), 42);
}

#[test]
fn test_with_transaction_rollback() {
    let store = DuckDbGraphStore::new(DuckDbGraphConfig::default()).unwrap();

    let result: Result<i32, GraphError> = store.with_transaction(|conn| {
        conn.execute(
            "INSERT INTO memory_nodes (id, label, properties, tenant_id) VALUES (?, ?, ?, ?)",
            duckdb::params!["tx-node-fail", "test", "{}", "tx-tenant"],
        )?;
        Err(GraphError::Serialization("Forced failure".to_string()))
    });

    assert!(result.is_err());

    let ctx = create_test_context("tx-tenant");
    let stats = store.get_stats(ctx).unwrap();
    assert_eq!(stats.node_count, 0, "Transaction should have rolled back");
}

#[test]
fn test_atomic_validates_tenant_id() {
    let store = DuckDbGraphStore::new(DuckDbGraphConfig::default()).unwrap();
    let ctx = create_test_context("tenant-1");

    let result = store.add_nodes_and_edges_atomic(&ctx, "", vec![], vec![]);
    assert!(matches!(result, Err(GraphError::InvalidTenantIdFormat(_))));

    let result = store.add_nodes_and_edges_atomic(&ctx, "tenant'; DROP TABLE", vec![], vec![]);
    assert!(matches!(result, Err(GraphError::InvalidTenantIdFormat(_))));
}

#[test]
fn test_atomic_empty_batch_succeeds() {
    let store = DuckDbGraphStore::new(DuckDbGraphConfig::default()).unwrap();
    let ctx = create_test_context("tenant-1");

    let result = store.add_nodes_and_edges_atomic(&ctx, "tenant-1", vec![], vec![]);
    assert!(result.is_ok(), "Empty batch should succeed");
}
