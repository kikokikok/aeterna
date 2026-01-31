use memory::backends::{
    BackendConfig, SearchQuery, VectorBackend, VectorBackendType, VectorRecord, create_backend
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

struct BenchmarkResult {
    backend: String,
    operation: String,
    total_ops: usize,
    total_time: Duration,
    avg_latency_ms: f64,
    p50_latency_ms: f64,
    p95_latency_ms: f64,
    p99_latency_ms: f64,
    ops_per_sec: f64
}

impl BenchmarkResult {
    fn print(&self) {
        println!(
            "| {:<12} | {:<10} | {:>8} | {:>10.2}ms | {:>8.2}ms | {:>8.2}ms | {:>8.2}ms | \
             {:>10.1} |",
            self.backend,
            self.operation,
            self.total_ops,
            self.avg_latency_ms,
            self.p50_latency_ms,
            self.p95_latency_ms,
            self.p99_latency_ms,
            self.ops_per_sec
        );
    }
}

fn calculate_percentile(sorted_latencies: &[f64], percentile: f64) -> f64 {
    if sorted_latencies.is_empty() {
        return 0.0;
    }
    let idx = ((percentile / 100.0) * (sorted_latencies.len() - 1) as f64).round() as usize;
    sorted_latencies[idx.min(sorted_latencies.len() - 1)]
}

fn make_vectors(count: usize, dim: usize, prefix: &str) -> Vec<VectorRecord> {
    (0..count)
        .map(|i| {
            let vector: Vec<f32> = (0..dim).map(|j| ((i * dim + j) as f32).sin()).collect();
            let mut metadata = HashMap::new();
            metadata.insert("idx".to_string(), serde_json::json!(i));
            VectorRecord::new(format!("{}-{}", prefix, i), vector, metadata)
        })
        .collect()
}

async fn benchmark_upsert(
    backend: Arc<dyn VectorBackend>,
    tenant_id: &str,
    batch_size: usize,
    num_batches: usize,
    dim: usize
) -> BenchmarkResult {
    let mut latencies = Vec::with_capacity(num_batches);
    let start = Instant::now();

    for batch_idx in 0..num_batches {
        let vectors = make_vectors(batch_size, dim, &format!("bench-{}", batch_idx));
        let op_start = Instant::now();
        let _ = backend.upsert(tenant_id, vectors).await;
        latencies.push(op_start.elapsed().as_secs_f64() * 1000.0);
    }

    let total_time = start.elapsed();
    latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let total_ops = num_batches * batch_size;
    BenchmarkResult {
        backend: backend.backend_name().to_string(),
        operation: "upsert".to_string(),
        total_ops,
        total_time,
        avg_latency_ms: latencies.iter().sum::<f64>() / latencies.len() as f64,
        p50_latency_ms: calculate_percentile(&latencies, 50.0),
        p95_latency_ms: calculate_percentile(&latencies, 95.0),
        p99_latency_ms: calculate_percentile(&latencies, 99.0),
        ops_per_sec: total_ops as f64 / total_time.as_secs_f64()
    }
}

async fn benchmark_search(
    backend: Arc<dyn VectorBackend>,
    tenant_id: &str,
    num_queries: usize,
    dim: usize,
    limit: usize
) -> BenchmarkResult {
    let mut latencies = Vec::with_capacity(num_queries);
    let start = Instant::now();

    for i in 0..num_queries {
        let query_vector: Vec<f32> = (0..dim).map(|j| ((i * dim + j) as f32).cos()).collect();
        let query = SearchQuery::new(query_vector).with_limit(limit);

        let op_start = Instant::now();
        let _ = backend.search(tenant_id, query).await;
        latencies.push(op_start.elapsed().as_secs_f64() * 1000.0);
    }

    let total_time = start.elapsed();
    latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());

    BenchmarkResult {
        backend: backend.backend_name().to_string(),
        operation: "search".to_string(),
        total_ops: num_queries,
        total_time,
        avg_latency_ms: latencies.iter().sum::<f64>() / latencies.len() as f64,
        p50_latency_ms: calculate_percentile(&latencies, 50.0),
        p95_latency_ms: calculate_percentile(&latencies, 95.0),
        p99_latency_ms: calculate_percentile(&latencies, 99.0),
        ops_per_sec: num_queries as f64 / total_time.as_secs_f64()
    }
}

async fn benchmark_get(
    backend: Arc<dyn VectorBackend>,
    tenant_id: &str,
    ids: &[String]
) -> BenchmarkResult {
    let mut latencies = Vec::with_capacity(ids.len());
    let start = Instant::now();

    for id in ids {
        let op_start = Instant::now();
        let _ = backend.get(tenant_id, id).await;
        latencies.push(op_start.elapsed().as_secs_f64() * 1000.0);
    }

    let total_time = start.elapsed();
    latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());

    BenchmarkResult {
        backend: backend.backend_name().to_string(),
        operation: "get".to_string(),
        total_ops: ids.len(),
        total_time,
        avg_latency_ms: latencies.iter().sum::<f64>() / latencies.len() as f64,
        p50_latency_ms: calculate_percentile(&latencies, 50.0),
        p95_latency_ms: calculate_percentile(&latencies, 95.0),
        p99_latency_ms: calculate_percentile(&latencies, 99.0),
        ops_per_sec: ids.len() as f64 / total_time.as_secs_f64()
    }
}

async fn run_full_benchmark(backend: Arc<dyn VectorBackend>, dim: usize) {
    let tenant_id = format!("bench-{}", backend.backend_name());
    let batch_size = 100;
    let num_batches = 10;
    let num_queries = 100;
    let search_limit = 10;

    println!("\n### {} Benchmark Results", backend.backend_name());
    println!(
        "| {:<12} | {:<10} | {:>8} | {:>12} | {:>10} | {:>10} | {:>10} | {:>12} |",
        "Backend", "Operation", "Ops", "Avg Latency", "P50", "P95", "P99", "Ops/sec"
    );
    println!(
        "|{:-<14}|{:-<12}|{:-<10}|{:-<14}|{:-<12}|{:-<12}|{:-<12}|{:-<14}|",
        "", "", "", "", "", "", "", ""
    );

    let upsert_result =
        benchmark_upsert(backend.clone(), &tenant_id, batch_size, num_batches, dim).await;
    upsert_result.print();

    tokio::time::sleep(Duration::from_secs(1)).await;

    let search_result =
        benchmark_search(backend.clone(), &tenant_id, num_queries, dim, search_limit).await;
    search_result.print();

    let ids: Vec<String> = (0..100).map(|i| format!("bench-0-{}", i)).collect();
    let get_result = benchmark_get(backend.clone(), &tenant_id, &ids).await;
    get_result.print();

    let delete_ids: Vec<String> = (0..num_batches)
        .flat_map(|b| (0..batch_size).map(move |i| format!("bench-{}-{}", b, i)))
        .collect();
    let _ = backend.delete(&tenant_id, delete_ids).await;

    println!();
}

#[tokio::test]
#[ignore = "requires running Qdrant instance"]
async fn benchmark_qdrant() {
    let config = BackendConfig {
        backend_type: VectorBackendType::Qdrant,
        embedding_dimension: 1536,
        qdrant: Some(memory::backends::factory::QdrantConfig {
            url: std::env::var("QDRANT_URL").unwrap_or_else(|_| "http://localhost:6334".into()),
            api_key: std::env::var("QDRANT_API_KEY").ok(),
            collection_prefix: "benchmark".into()
        }),
        ..Default::default()
    };

    let backend = create_backend(config).await.unwrap();
    run_full_benchmark(backend, 1536).await;
}

#[tokio::test]
#[ignore = "requires running PostgreSQL with pgvector"]
async fn benchmark_pgvector() {
    let config = BackendConfig {
        backend_type: VectorBackendType::Pgvector,
        embedding_dimension: 1536,
        pgvector: Some(memory::backends::factory::PgvectorConfig {
            connection_string: std::env::var("PGVECTOR_URL")
                .unwrap_or_else(|_| "postgres://postgres:postgres@localhost/aeterna".into()),
            schema: "public".into(),
            table_name: "benchmark_vectors".into()
        }),
        ..Default::default()
    };

    let backend = create_backend(config).await.unwrap();
    run_full_benchmark(backend, 1536).await;
}

#[tokio::test]
#[ignore = "requires Pinecone API key"]
async fn benchmark_pinecone() {
    let api_key = std::env::var("PINECONE_API_KEY").expect("PINECONE_API_KEY required");
    let environment = std::env::var("PINECONE_ENVIRONMENT").expect("PINECONE_ENVIRONMENT required");

    let config = BackendConfig {
        backend_type: VectorBackendType::Pinecone,
        embedding_dimension: 1536,
        pinecone: Some(memory::backends::factory::PineconeConfig {
            api_key,
            environment,
            index_name: "benchmark".into()
        }),
        ..Default::default()
    };

    let backend = create_backend(config).await.unwrap();
    run_full_benchmark(backend, 1536).await;
}

#[tokio::test]
#[ignore = "requires running Weaviate instance"]
async fn benchmark_weaviate() {
    let config = BackendConfig {
        backend_type: VectorBackendType::Weaviate,
        embedding_dimension: 1536,
        weaviate: Some(memory::backends::factory::WeaviateConfig {
            url: std::env::var("WEAVIATE_URL").unwrap_or_else(|_| "http://localhost:8080".into()),
            api_key: std::env::var("WEAVIATE_API_KEY").ok(),
            class_name: "Benchmark".into()
        }),
        ..Default::default()
    };

    let backend = create_backend(config).await.unwrap();
    run_full_benchmark(backend, 1536).await;
}

#[tokio::test]
#[ignore = "requires MongoDB Atlas"]
async fn benchmark_mongodb() {
    let config = BackendConfig {
        backend_type: VectorBackendType::Mongodb,
        embedding_dimension: 1536,
        mongodb: Some(memory::backends::factory::MongodbConfig {
            connection_string: std::env::var("MONGODB_URI").expect("MONGODB_URI required"),
            database: "benchmark".into(),
            collection: "vectors".into(),
            index_name: "vector_index".into()
        }),
        ..Default::default()
    };

    let backend = create_backend(config).await.unwrap();
    run_full_benchmark(backend, 1536).await;
}

#[tokio::test]
#[ignore = "requires GCP with Vertex AI"]
async fn benchmark_vertex_ai() {
    let config = BackendConfig {
        backend_type: VectorBackendType::VertexAi,
        embedding_dimension: 1536,
        vertex_ai: Some(memory::backends::factory::VertexAiConfig {
            project_id: std::env::var("GCP_PROJECT_ID").expect("GCP_PROJECT_ID required"),
            location: std::env::var("VERTEX_AI_LOCATION").unwrap_or_else(|_| "us-central1".into()),
            index_endpoint: std::env::var("VERTEX_AI_INDEX_ENDPOINT")
                .expect("VERTEX_AI_INDEX_ENDPOINT required"),
            deployed_index_id: std::env::var("VERTEX_AI_DEPLOYED_INDEX_ID")
                .expect("VERTEX_AI_DEPLOYED_INDEX_ID required")
        }),
        ..Default::default()
    };

    let backend = create_backend(config).await.unwrap();
    run_full_benchmark(backend, 1536).await;
}

#[tokio::test]
#[ignore = "requires Databricks workspace"]
async fn benchmark_databricks() {
    let config = BackendConfig {
        backend_type: VectorBackendType::Databricks,
        embedding_dimension: 1536,
        databricks: Some(memory::backends::factory::DatabricksConfig {
            workspace_url: std::env::var("DATABRICKS_HOST").expect("DATABRICKS_HOST required"),
            token: std::env::var("DATABRICKS_TOKEN").expect("DATABRICKS_TOKEN required"),
            catalog: std::env::var("DATABRICKS_CATALOG").unwrap_or_else(|_| "main".into()),
            schema: std::env::var("DATABRICKS_SCHEMA").unwrap_or_else(|_| "benchmark".into())
        }),
        ..Default::default()
    };

    let backend = create_backend(config).await.unwrap();
    run_full_benchmark(backend, 1536).await;
}
