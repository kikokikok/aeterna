//! Performance benchmark tests for DuckDbGraphStore
//!
//! These tests verify that query performance meets expectations
//! when using composite and single-column indexes.
//!
//! Note: Thresholds are set for debug builds. Release builds would be faster.

use mk_core::types::{TenantContext, TenantId, UserId};
use serde_json::json;
use std::time::Instant;
use storage::graph::{GraphEdge, GraphNode};
use storage::graph_duckdb::{DuckDbGraphConfig, DuckDbGraphStore};

const DEBUG_MULTIPLIER: f64 = 50.0;

fn create_store() -> DuckDbGraphStore {
    DuckDbGraphStore::new(DuckDbGraphConfig::default()).unwrap()
}

fn create_tenant_context(tenant_id: &str) -> TenantContext {
    TenantContext::new(
        TenantId::new(tenant_id.to_string()).unwrap(),
        UserId::new("benchmark-user".to_string()).unwrap(),
    )
}

fn create_test_node(id: &str, tenant_id: &str) -> GraphNode {
    GraphNode {
        id: id.to_string(),
        label: format!("node-{}", id),
        properties: json!({"key": "value", "index": id}),
        tenant_id: tenant_id.to_string(),
    }
}

fn create_test_edge(id: &str, source: &str, target: &str, tenant_id: &str) -> GraphEdge {
    GraphEdge {
        id: id.to_string(),
        source_id: source.to_string(),
        target_id: target.to_string(),
        relation: "related_to".to_string(),
        properties: json!({}),
        tenant_id: tenant_id.to_string(),
    }
}

#[test]
fn bench_batch_node_insert() {
    let store = create_store();
    let ctx = create_tenant_context("tenant-batch");

    let batch_size = 100;
    let nodes: Vec<GraphNode> = (0..batch_size)
        .map(|i| create_test_node(&format!("batch-node-{}", i), "tenant-batch"))
        .collect();
    let edges: Vec<GraphEdge> = vec![];

    let start = Instant::now();
    store
        .add_nodes_and_edges_atomic(&ctx, "tenant-batch", nodes, edges)
        .unwrap();
    let elapsed = start.elapsed();

    let avg_ms = elapsed.as_millis() as f64 / batch_size as f64;
    println!(
        "Batch insert {} nodes in {:?} (avg: {:.3}ms/node)",
        batch_size, elapsed, avg_ms
    );

    assert!(
        avg_ms < 5.0 * DEBUG_MULTIPLIER,
        "Batch insert should be reasonable for debug build"
    );
}

#[test]
fn bench_find_related_small_graph() {
    let store = create_store();
    let ctx = create_tenant_context("tenant-small");

    let nodes: Vec<GraphNode> = (0..50)
        .map(|i| create_test_node(&format!("s-node-{}", i), "tenant-small"))
        .collect();

    let edges: Vec<GraphEdge> = (0..49)
        .map(|i| {
            create_test_edge(
                &format!("s-edge-{}", i),
                &format!("s-node-{}", i),
                &format!("s-node-{}", i + 1),
                "tenant-small",
            )
        })
        .collect();

    store
        .add_nodes_and_edges_atomic(&ctx, "tenant-small", nodes, edges)
        .unwrap();

    let iterations = 20;
    let start = Instant::now();

    for _ in 0..iterations {
        let _ = store.find_related(ctx.clone(), "s-node-0", 2).unwrap();
    }

    let elapsed = start.elapsed();
    let avg_ms = elapsed.as_millis() as f64 / iterations as f64;

    println!(
        "find_related (50 nodes, depth=2): {} iterations in {:?} (avg: {:.3}ms/query)",
        iterations, elapsed, avg_ms
    );

    assert!(
        avg_ms < 50.0 * DEBUG_MULTIPLIER,
        "find_related on small graph should be reasonable for debug build"
    );
}

#[test]
fn bench_find_related_medium_graph() {
    let store = create_store();
    let ctx = create_tenant_context("tenant-medium");

    let node_count = 200;
    let nodes: Vec<GraphNode> = (0..node_count)
        .map(|i| create_test_node(&format!("m-node-{}", i), "tenant-medium"))
        .collect();

    let edges: Vec<GraphEdge> = (0..(node_count - 1))
        .map(|i| {
            create_test_edge(
                &format!("m-edge-{}", i),
                &format!("m-node-{}", i),
                &format!("m-node-{}", i + 1),
                "tenant-medium",
            )
        })
        .collect();

    store
        .add_nodes_and_edges_atomic(&ctx, "tenant-medium", nodes, edges)
        .unwrap();

    let iterations = 10;
    let start = Instant::now();

    for _ in 0..iterations {
        let _ = store.find_related(ctx.clone(), "m-node-50", 3).unwrap();
    }

    let elapsed = start.elapsed();
    let avg_ms = elapsed.as_millis() as f64 / iterations as f64;

    println!(
        "find_related (200 nodes, depth=3): {} iterations in {:?} (avg: {:.3}ms/query)",
        iterations, elapsed, avg_ms
    );

    assert!(
        avg_ms < 200.0 * DEBUG_MULTIPLIER,
        "find_related on medium graph should be reasonable for debug build"
    );
}

#[test]
fn bench_shortest_path() {
    let store = create_store();
    let ctx = create_tenant_context("tenant-path");

    let node_count = 100;
    let nodes: Vec<GraphNode> = (0..node_count)
        .map(|i| create_test_node(&format!("p-node-{}", i), "tenant-path"))
        .collect();

    let edges: Vec<GraphEdge> = (0..(node_count - 1))
        .map(|i| {
            create_test_edge(
                &format!("p-edge-{}", i),
                &format!("p-node-{}", i),
                &format!("p-node-{}", i + 1),
                "tenant-path",
            )
        })
        .collect();

    store
        .add_nodes_and_edges_atomic(&ctx, "tenant-path", nodes, edges)
        .unwrap();

    let iterations = 10;
    let start = Instant::now();

    for _ in 0..iterations {
        let _ = store
            .shortest_path(ctx.clone(), "p-node-0", "p-node-50", None)
            .unwrap();
    }

    let elapsed = start.elapsed();
    let avg_ms = elapsed.as_millis() as f64 / iterations as f64;

    println!(
        "shortest_path (100 nodes, 50 hops): {} iterations in {:?} (avg: {:.3}ms/query)",
        iterations, elapsed, avg_ms
    );

    assert!(
        avg_ms < 100.0 * DEBUG_MULTIPLIER,
        "shortest_path on linear graph should be reasonable for debug build"
    );
}

#[test]
fn bench_get_stats() {
    let store = create_store();
    let ctx = create_tenant_context("tenant-stats");

    let node_count = 500;
    let nodes: Vec<GraphNode> = (0..node_count)
        .map(|i| create_test_node(&format!("st-node-{}", i), "tenant-stats"))
        .collect();

    let edges: Vec<GraphEdge> = (0..(node_count - 1))
        .map(|i| {
            create_test_edge(
                &format!("st-edge-{}", i),
                &format!("st-node-{}", i),
                &format!("st-node-{}", i + 1),
                "tenant-stats",
            )
        })
        .collect();

    store
        .add_nodes_and_edges_atomic(&ctx, "tenant-stats", nodes, edges)
        .unwrap();

    let iterations = 50;
    let start = Instant::now();

    for _ in 0..iterations {
        let _ = store.get_stats(ctx.clone()).unwrap();
    }

    let elapsed = start.elapsed();
    let avg_ms = elapsed.as_millis() as f64 / iterations as f64;

    println!(
        "get_stats (500 nodes, 499 edges): {} iterations in {:?} (avg: {:.3}ms/query)",
        iterations, elapsed, avg_ms
    );

    assert!(
        avg_ms < 20.0 * DEBUG_MULTIPLIER,
        "get_stats should be reasonable for debug build"
    );
}

#[test]
fn bench_tenant_isolation_query() {
    let store = create_store();

    for tenant_num in 0..5 {
        let tenant_id = format!("tenant-iso-{}", tenant_num);
        let ctx = create_tenant_context(&tenant_id);

        let nodes: Vec<GraphNode> = (0..100)
            .map(|i| create_test_node(&format!("{}-node-{}", tenant_id, i), &tenant_id))
            .collect();

        store
            .add_nodes_and_edges_atomic(&ctx, &tenant_id, nodes, vec![])
            .unwrap();
    }

    let iterations = 20;
    let start = Instant::now();

    for _ in 0..iterations {
        let ctx = create_tenant_context("tenant-iso-2");
        let _ = store.get_stats(ctx).unwrap();
    }

    let elapsed = start.elapsed();
    let avg_ms = elapsed.as_millis() as f64 / iterations as f64;

    println!(
        "Tenant isolated query (5 tenants, 100 nodes each): {} iterations in {:?} (avg: \
         {:.3}ms/query)",
        iterations, elapsed, avg_ms
    );

    assert!(
        avg_ms < 30.0 * DEBUG_MULTIPLIER,
        "Tenant isolated query should be reasonable for debug build"
    );
}

#[test]
fn bench_index_usage_verification() {
    let store = create_store();
    let ctx = create_tenant_context("tenant-idx");

    let nodes: Vec<GraphNode> = (0..1000)
        .map(|i| create_test_node(&format!("idx-node-{}", i), "tenant-idx"))
        .collect();

    store
        .add_nodes_and_edges_atomic(&ctx, "tenant-idx", nodes, vec![])
        .unwrap();

    let start = Instant::now();
    let stats = store.get_stats(ctx.clone()).unwrap();
    let indexed_query_time = start.elapsed();

    assert_eq!(stats.node_count, 1000);

    println!(
        "Index verification: 1000 nodes queried in {:?}",
        indexed_query_time
    );

    assert!(
        indexed_query_time.as_millis() < 50 * DEBUG_MULTIPLIER as u128,
        "Indexed query on 1000 nodes should be reasonable for debug build"
    );
}

#[test]
fn bench_concurrent_reads() {
    use std::sync::Arc;
    use std::thread;

    let store = Arc::new(create_store());
    let tenant_id = "tenant-concurrent";
    let ctx = create_tenant_context(tenant_id);

    let nodes: Vec<GraphNode> = (0..200)
        .map(|i| create_test_node(&format!("conc-node-{}", i), tenant_id))
        .collect();

    store
        .add_nodes_and_edges_atomic(&ctx, tenant_id, nodes, vec![])
        .unwrap();

    let thread_count = 4;
    let iterations_per_thread = 25;

    let start = Instant::now();
    let handles: Vec<_> = (0..thread_count)
        .map(|_| {
            let store = Arc::clone(&store);
            let tid = tenant_id.to_string();
            thread::spawn(move || {
                for _ in 0..iterations_per_thread {
                    let ctx = create_tenant_context(&tid);
                    let _ = store.get_stats(ctx).unwrap();
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let elapsed = start.elapsed();
    let total_ops = thread_count * iterations_per_thread;
    let ops_per_sec = total_ops as f64 / elapsed.as_secs_f64();

    println!(
        "Concurrent reads: {} threads x {} ops = {} total in {:?} ({:.0} ops/sec)",
        thread_count, iterations_per_thread, total_ops, elapsed, ops_per_sec
    );

    assert!(
        ops_per_sec > 1.0,
        "Should achieve concurrent operations in debug build"
    );
}

#[test]
fn bench_health_check_latency() {
    let store = create_store();

    let iterations = 100;
    let start = Instant::now();

    for _ in 0..iterations {
        let _ = store.health_check();
    }

    let elapsed = start.elapsed();
    let avg_ms = elapsed.as_millis() as f64 / iterations as f64;

    println!(
        "health_check: {} iterations in {:?} (avg: {:.3}ms/check)",
        iterations, elapsed, avg_ms
    );

    assert!(
        avg_ms < 5.0 * DEBUG_MULTIPLIER,
        "health_check should be reasonable for debug build"
    );
}

#[test]
fn bench_readiness_check_latency() {
    let store = create_store();

    let iterations = 100;
    let start = Instant::now();

    for _ in 0..iterations {
        let _ = store.readiness_check();
    }

    let elapsed = start.elapsed();
    let avg_ms = elapsed.as_millis() as f64 / iterations as f64;

    println!(
        "readiness_check: {} iterations in {:?} (avg: {:.3}ms/check)",
        iterations, elapsed, avg_ms
    );

    assert!(
        avg_ms < 5.0 * DEBUG_MULTIPLIER,
        "readiness_check should be reasonable for debug build"
    );
}

#[test]
fn bench_atomic_transaction() {
    let store = create_store();
    let ctx = create_tenant_context("tenant-atomic");

    let nodes: Vec<GraphNode> = (0..50)
        .map(|i| create_test_node(&format!("atomic-node-{}", i), "tenant-atomic"))
        .collect();

    let edges: Vec<GraphEdge> = (0..49)
        .map(|i| {
            create_test_edge(
                &format!("atomic-edge-{}", i),
                &format!("atomic-node-{}", i),
                &format!("atomic-node-{}", i + 1),
                "tenant-atomic",
            )
        })
        .collect();

    let iterations = 10;
    let start = Instant::now();

    for batch in 0..iterations {
        let batch_nodes: Vec<GraphNode> = nodes
            .iter()
            .map(|n| GraphNode {
                id: format!("{}-batch-{}", n.id, batch),
                ..n.clone()
            })
            .collect();

        let batch_edges: Vec<GraphEdge> = edges
            .iter()
            .enumerate()
            .map(|(i, e)| GraphEdge {
                id: format!("{}-batch-{}", e.id, batch),
                source_id: format!("atomic-node-{}-batch-{}", i, batch),
                target_id: format!("atomic-node-{}-batch-{}", i + 1, batch),
                ..e.clone()
            })
            .collect();

        store
            .add_nodes_and_edges_atomic(&ctx, "tenant-atomic", batch_nodes, batch_edges)
            .unwrap();
    }

    let elapsed = start.elapsed();
    let avg_ms = elapsed.as_millis() as f64 / iterations as f64;

    println!(
        "Atomic transaction (50 nodes + 49 edges): {} iterations in {:?} (avg: {:.3}ms/tx)",
        iterations, elapsed, avg_ms
    );

    let stats = store.get_stats(ctx).unwrap();
    assert_eq!(stats.node_count, 50 * iterations);
    assert_eq!(stats.edge_count, 49 * iterations);

    assert!(
        avg_ms < 100.0 * DEBUG_MULTIPLIER,
        "Atomic transaction should be reasonable for debug build"
    );
}

#[test]
fn bench_soft_removal_performance() {
    let store = create_store();
    let ctx = create_tenant_context("tenantrem");

    let nodes: Vec<GraphNode> = (0..100)
        .map(|i| {
            let mut node = create_test_node(&format!("rem-node-{}", i), "tenantrem");
            node.properties = json!({"source_memory_id": format!("memory{}", i % 10)});
            node
        })
        .collect();

    store
        .add_nodes_and_edges_atomic(&ctx, "tenantrem", nodes, vec![])
        .unwrap();

    let iterations = 10;
    let start = Instant::now();

    for i in 0..iterations {
        let _ = store.soft_delete_node(ctx.clone(), &format!("rem-node-{}", i));
    }

    let elapsed = start.elapsed();
    let avg_ms = elapsed.as_millis() as f64 / iterations as f64;

    println!(
        "soft_delete: {} iterations in {:?} (avg: {:.3}ms/delete)",
        iterations, elapsed, avg_ms
    );

    assert!(
        avg_ms < 20.0 * DEBUG_MULTIPLIER,
        "soft_delete should be reasonable for debug build"
    );
}

#[test]
fn bench_large_graph_traversal() {
    let store = create_store();
    let ctx = create_tenant_context("tenant-large");

    let node_count = 500;
    let nodes: Vec<GraphNode> = (0..node_count)
        .map(|i| create_test_node(&format!("lg-node-{}", i), "tenant-large"))
        .collect();

    let mut edges: Vec<GraphEdge> = (0..(node_count - 1))
        .map(|i| {
            create_test_edge(
                &format!("lg-edge-{}", i),
                &format!("lg-node-{}", i),
                &format!("lg-node-{}", i + 1),
                "tenant-large",
            )
        })
        .collect();

    for i in (0..node_count - 20).step_by(10) {
        edges.push(create_test_edge(
            &format!("lg-cross-{}", i),
            &format!("lg-node-{}", i),
            &format!("lg-node-{}", i + 20),
            "tenant-large",
        ));
    }

    store
        .add_nodes_and_edges_atomic(&ctx, "tenant-large", nodes, edges)
        .unwrap();

    let iterations = 5;
    let start = Instant::now();

    for _ in 0..iterations {
        let _ = store.find_related(ctx.clone(), "lg-node-0", 4).unwrap();
    }

    let elapsed = start.elapsed();
    let avg_ms = elapsed.as_millis() as f64 / iterations as f64;

    println!(
        "Large graph traversal (500 nodes, depth=4): {} iterations in {:?} (avg: {:.3}ms/query)",
        iterations, elapsed, avg_ms
    );

    assert!(
        avg_ms < 500.0 * DEBUG_MULTIPLIER,
        "Large graph traversal should be reasonable for debug build"
    );
}
