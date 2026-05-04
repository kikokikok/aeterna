use criterion::{BatchSize, Criterion, black_box, criterion_group, criterion_main};
use mk_core::types::{TenantContext, TenantId, UserId};
use serde_json::json;
use std::sync::{Arc, Barrier};
use std::thread;
use storage::graph::{GraphNode, GraphStore};
use storage::graph_duckdb::{DuckDbGraphConfig, DuckDbGraphStore};
use tempfile::tempdir;
use tokio::runtime::Runtime;

fn tenant_context() -> TenantContext {
    TenantContext::new(
        TenantId::new("t".into()).unwrap(),
        UserId::new("u".into()).unwrap(),
    )
}

fn node(id: String, tenant_id: String) -> GraphNode {
    GraphNode {
        id: id.clone(),
        label: id,
        properties: json!({}),
        tenant_id,
    }
}

fn seed_nodes(
    rt: &Runtime,
    store: &DuckDbGraphStore,
    ctx: &TenantContext,
    tenant_id: &str,
    count: usize,
) {
    for i in 0..count {
        rt.block_on(store.add_node(
            ctx.clone(),
            node(format!("node-{i}"), tenant_id.to_string()),
        ))
        .unwrap();
    }
}

fn single_reader_get_stats(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let ctx = tenant_context();
    let store = DuckDbGraphStore::new(DuckDbGraphConfig::default()).unwrap();
    seed_nodes(&rt, &store, &ctx, "t", 500);

    c.bench_function("bench_single_reader_get_stats", |b| {
        b.iter(|| black_box(store.get_stats(ctx.clone()).unwrap()))
    });
}

fn concurrent_readers_get_stats(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let ctx = tenant_context();
    let _tempdir = tempdir().unwrap();
    let db_path = _tempdir.path().join("graph.duckdb");
    let config = DuckDbGraphConfig {
        path: db_path.to_string_lossy().into_owned(),
        reader_pool_size: Some(4),
        ..Default::default()
    };
    let store = Arc::new(DuckDbGraphStore::new(config).unwrap());
    seed_nodes(&rt, &store, &ctx, "t", 500);

    c.bench_function("bench_concurrent_readers_get_stats", |b| {
        b.iter(|| {
            let barrier = Arc::new(Barrier::new(4));
            let handles: Vec<_> = (0..4)
                .map(|_| {
                    let barrier = Arc::clone(&barrier);
                    let store = Arc::clone(&store);
                    let ctx = ctx.clone();
                    thread::spawn(move || {
                        barrier.wait();
                        black_box(store.get_stats(ctx).unwrap())
                    })
                })
                .collect();

            for handle in handles {
                black_box(handle.join().unwrap());
            }
        })
    });
}

fn writer_add_node(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let ctx = tenant_context();

    c.bench_function("bench_writer_add_node", |b| {
        b.iter_batched(
            || {
                let store = DuckDbGraphStore::new(DuckDbGraphConfig::default()).unwrap();
                let node = node("writer-node".to_string(), "t".to_string());
                (store, node)
            },
            |(store, node)| {
                rt.block_on(store.add_node(ctx.clone(), node)).unwrap();
            },
            BatchSize::PerIteration,
        )
    });
}

fn benches(c: &mut Criterion) {
    single_reader_get_stats(c);
    concurrent_readers_get_stats(c);
    writer_add_node(c);
}

criterion_group!(graph_pool_benches, benches);
criterion_main!(graph_pool_benches);
