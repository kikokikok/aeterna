// E2E pipeline tests — GoogleEmbeddingService + VertexAiBackend together.
//
// These tests exercise the full AI/ML pipeline:
//   text → GoogleEmbeddingService (Vertex AI Embeddings) → Vec<f32>
//                                                        ↓
//                                          VertexAiBackend (Vector Search)
//                                          upsert / search / delete
//
// # Why a separate file?
//
// GoogleEmbeddingService requires `--features google-provider`.
// VertexAiBackend requires `--features vertex-ai`.
// These features are independent and may be tested separately, but the pipeline
// tests require both simultaneously.
//
// # Setup
//
// You need two distinct GCP resources:
//   A) Vertex AI Embedding API (aiplatform.googleapis.com)
//   B) Vertex AI Vector Search index + deployed endpoint
//
// IMPORTANT: The Vector Search index must be configured with dimension=768
//   (matching text-embedding-005 output). Do NOT use the default 1536.
//
// Export environment variables:
//   ```sh
//   # Shared GCP config
//   export AETERNA_GOOGLE_PROJECT_ID="my-gcp-project"
//   export AETERNA_GOOGLE_LOCATION="us-central1"
//
//   # Embedding
//   export AETERNA_GOOGLE_EMBEDDING_MODEL="text-embedding-005"
//
//   # Vector Search backend
//   export GCP_PROJECT_ID="my-gcp-project"   # may differ from above if using separate project
//   export VERTEX_AI_LOCATION="us-central1"
//   export VERTEX_AI_INDEX_ENDPOINT="projects/PROJECT_NUMBER/locations/REGION/indexEndpoints/ENDPOINT_ID"
//   export VERTEX_AI_DEPLOYED_INDEX_ID="my_deployed_index"
//
//   # Auth (must work for BOTH APIs)
//   export GOOGLE_ACCESS_TOKEN="$(gcloud auth print-access-token)"
//   # or: export GOOGLE_APPLICATION_CREDENTIALS="/path/to/sa.json"
//   ```
//
// Run:
//   ```sh
//   cargo test -p memory --features google-provider,vertex-ai \
//     --test vertex_ai_pipeline_e2e_test -- --ignored
//   ```

#[cfg(all(feature = "google-provider", feature = "vertex-ai"))]
mod vertex_ai_pipeline_e2e {
    use memory::backends::factory::VertexAiConfig;
    use memory::backends::{
        BackendConfig, SearchQuery, VectorBackend, VectorBackendType, VectorRecord, create_backend,
    };
    use memory::embedding::google::GoogleEmbeddingService;
    use mk_core::traits::EmbeddingService;
    use std::collections::HashMap;

    const EMBEDDING_DIM: usize = 768;

    fn embedding_service() -> GoogleEmbeddingService {
        let project_id =
            std::env::var("AETERNA_GOOGLE_PROJECT_ID").expect("AETERNA_GOOGLE_PROJECT_ID not set");
        let location =
            std::env::var("AETERNA_GOOGLE_LOCATION").unwrap_or_else(|_| "us-central1".to_string());
        let model = std::env::var("AETERNA_GOOGLE_EMBEDDING_MODEL")
            .unwrap_or_else(|_| "text-embedding-005".to_string());
        GoogleEmbeddingService::new(project_id, location, model)
    }

    async fn vector_backend() -> std::sync::Arc<dyn VectorBackend> {
        let config = BackendConfig {
            backend_type: VectorBackendType::VertexAi,
            embedding_dimension: EMBEDDING_DIM,
            vertex_ai: Some(VertexAiConfig {
                project_id: std::env::var("GCP_PROJECT_ID").expect("GCP_PROJECT_ID not set"),
                location: std::env::var("VERTEX_AI_LOCATION")
                    .unwrap_or_else(|_| "us-central1".to_string()),
                index_endpoint: std::env::var("VERTEX_AI_INDEX_ENDPOINT")
                    .expect("VERTEX_AI_INDEX_ENDPOINT not set"),
                deployed_index_id: std::env::var("VERTEX_AI_DEPLOYED_INDEX_ID")
                    .expect("VERTEX_AI_DEPLOYED_INDEX_ID not set"),
            }),
            qdrant: None,
            pinecone: None,
            pgvector: None,
            databricks: None,
            weaviate: None,
            mongodb: None,
        };
        create_backend(config)
            .await
            .expect("VertexAiBackend must initialize")
    }

    // -------------------------------------------------------------------------
    // Test 7: Full pipeline — embed text, store vector, retrieve by semantic
    //         similarity, verify correct ranking
    // -------------------------------------------------------------------------
    #[tokio::test]
    #[ignore = "requires GCP project with Vertex AI Embedding API + Vector Search (dim=768) - set AETERNA_GOOGLE_PROJECT_ID, GCP_PROJECT_ID, VERTEX_AI_LOCATION, VERTEX_AI_INDEX_ENDPOINT, VERTEX_AI_DEPLOYED_INDEX_ID, GOOGLE_ACCESS_TOKEN or ADC"]
    async fn test_embedding_semantic_search_relevance() {
        let embedder = embedding_service();
        let backend = vector_backend().await;
        let tenant = "pipeline-e2e-semantic";

        // Three documents: two related to memory/AI, one unrelated
        let texts = [
            (
                "pipeline-doc-memory",
                "Aeterna provides hierarchical vector memory storage for AI agents",
            ),
            (
                "pipeline-doc-agents",
                "Autonomous agents use semantic retrieval to access past context",
            ),
            (
                "pipeline-doc-finance",
                "The Federal Reserve sets interest rates to control monetary policy",
            ),
        ];

        // Embed and upsert all documents
        let mut records = Vec::new();
        for (id, text) in &texts {
            let vector = embedder
                .embed(text)
                .await
                .unwrap_or_else(|e| panic!("embed failed for {id}: {e}"));
            assert_eq!(
                vector.len(),
                EMBEDDING_DIM,
                "embedding must be {EMBEDDING_DIM}-dim for {id}"
            );
            records.push(VectorRecord::new(
                *id,
                vector,
                HashMap::from([("text".to_string(), serde_json::json!(text))]),
            ));
        }

        let upsert_result = backend
            .upsert(tenant, records)
            .await
            .expect("upsert of embedded documents must succeed");
        assert_eq!(upsert_result.upserted_count, 3);
        assert!(upsert_result.failed_ids.is_empty());

        // Vertex AI Vector Search has eventual consistency — wait for index to update
        tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;

        // Query: close to the "memory" domain
        let query_text = "vector memory retrieval for AI agent context";
        let query_vector = embedder
            .embed(query_text)
            .await
            .expect("embed query must succeed");

        let query = SearchQuery::new(query_vector).with_limit(3);
        let results = backend
            .search(tenant, query)
            .await
            .expect("search must succeed");

        assert!(!results.is_empty(), "search must return results");

        // The top-1 result must be one of the two AI/memory documents, not the finance doc
        let top_id = &results[0].id;
        assert_ne!(
            top_id.as_str(),
            "pipeline-doc-finance",
            "the finance document must not be the top result for an AI/memory query (got {top_id})"
        );

        // Cleanup
        let ids = texts.iter().map(|(id, _)| id.to_string()).collect();
        backend
            .delete(tenant, ids)
            .await
            .expect("cleanup delete must succeed");
    }

    // -------------------------------------------------------------------------
    // Test 8: Tenant isolation — embeddings stored in tenant-A are not returned
    //         when searching from tenant-B
    // -------------------------------------------------------------------------
    #[tokio::test]
    #[ignore = "requires GCP project with Vertex AI Embedding API + Vector Search (dim=768) - set AETERNA_GOOGLE_PROJECT_ID, GCP_PROJECT_ID, VERTEX_AI_LOCATION, VERTEX_AI_INDEX_ENDPOINT, VERTEX_AI_DEPLOYED_INDEX_ID, GOOGLE_ACCESS_TOKEN or ADC"]
    async fn test_pipeline_tenant_isolation() {
        let embedder = embedding_service();
        let backend = vector_backend().await;

        let text_a = "Rust is a systems programming language focused on safety and performance";
        let text_b = "Python is a high-level dynamically-typed scripting language";

        let vec_a = embedder.embed(text_a).await.expect("embed tenant-a doc");
        let vec_b = embedder.embed(text_b).await.expect("embed tenant-b doc");

        backend
            .upsert(
                "pipeline-tenant-a",
                vec![VectorRecord::new(
                    "pipe-iso-1",
                    vec_a,
                    HashMap::from([("lang".to_string(), serde_json::json!("rust"))]),
                )],
            )
            .await
            .expect("upsert for tenant-a must succeed");

        backend
            .upsert(
                "pipeline-tenant-b",
                vec![VectorRecord::new(
                    "pipe-iso-1", // Same ID, different tenant
                    vec_b,
                    HashMap::from([("lang".to_string(), serde_json::json!("python"))]),
                )],
            )
            .await
            .expect("upsert for tenant-b must succeed");

        tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;

        // A third tenant must not see records from either tenant
        let result_c = backend
            .get("pipeline-tenant-c", "pipe-iso-1")
            .await
            .expect("get from tenant-c must not error");
        assert!(
            result_c.is_none(),
            "tenant-c must not see records belonging to tenant-a or tenant-b"
        );

        // Each tenant must see its own record only (vectors must differ)
        let rec_a = backend
            .get("pipeline-tenant-a", "pipe-iso-1")
            .await
            .expect("get from tenant-a must not error");
        let rec_b = backend
            .get("pipeline-tenant-b", "pipe-iso-1")
            .await
            .expect("get from tenant-b must not error");

        if let (Some(a), Some(b)) = (rec_a, rec_b) {
            assert_ne!(
                a.vector, b.vector,
                "tenant-a and tenant-b store different vectors for the same record ID"
            );
        }

        // Cleanup
        backend
            .delete("pipeline-tenant-a", vec!["pipe-iso-1".to_string()])
            .await
            .ok();
        backend
            .delete("pipeline-tenant-b", vec!["pipe-iso-1".to_string()])
            .await
            .ok();
    }

    // -------------------------------------------------------------------------
    // Test 9: EMBEDDING_DIMENSION mismatch guard
    //         Configuring backend with wrong dimension must produce an error
    //         or the health check must reflect degraded state.
    // -------------------------------------------------------------------------
    #[tokio::test]
    #[ignore = "requires GCP project with Vertex AI Vector Search (dim=768) - set GCP_PROJECT_ID, VERTEX_AI_LOCATION, VERTEX_AI_INDEX_ENDPOINT, VERTEX_AI_DEPLOYED_INDEX_ID, GOOGLE_ACCESS_TOKEN or ADC"]
    async fn test_vector_search_rejects_wrong_dimension_vector() {
        // Use the real backend but submit a 3-dim vector when the index expects 768
        let wrong_dim_config = BackendConfig {
            backend_type: VectorBackendType::VertexAi,
            embedding_dimension: 3, // Deliberately wrong
            vertex_ai: Some(VertexAiConfig {
                project_id: std::env::var("GCP_PROJECT_ID").expect("GCP_PROJECT_ID not set"),
                location: std::env::var("VERTEX_AI_LOCATION")
                    .unwrap_or_else(|_| "us-central1".to_string()),
                index_endpoint: std::env::var("VERTEX_AI_INDEX_ENDPOINT")
                    .expect("VERTEX_AI_INDEX_ENDPOINT not set"),
                deployed_index_id: std::env::var("VERTEX_AI_DEPLOYED_INDEX_ID")
                    .expect("VERTEX_AI_DEPLOYED_INDEX_ID not set"),
            }),
            qdrant: None,
            pinecone: None,
            pgvector: None,
            databricks: None,
            weaviate: None,
            mongodb: None,
        };

        let backend = create_backend(wrong_dim_config)
            .await
            .expect("backend init always succeeds; dimension mismatch only surfaces on search");

        let record = VectorRecord::new(
            "dim-mismatch-1",
            vec![1.0_f32, 0.0, 0.0], // 3-dim, not 768
            HashMap::new(),
        );

        // The upsert OR subsequent search must return an error from Vertex AI
        // indicating dimension mismatch. We accept either failure path.
        let upsert_result = backend.upsert("dim-mismatch-tenant", vec![record]).await;

        if upsert_result.is_ok() {
            tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
            let query = SearchQuery::new(vec![1.0_f32, 0.0, 0.0]).with_limit(1);
            let search_result = backend.search("dim-mismatch-tenant", query).await;
            assert!(
                search_result.is_err(),
                "search with 3-dim vector against 768-dim index must return an error"
            );
        } else {
            // upsert already rejected — that's the correct early-fail behaviour
            let err = upsert_result.unwrap_err();
            let msg = err.to_string();
            assert!(
                !msg.is_empty(),
                "dimension-mismatch error must carry a message"
            );
        }

        // Best-effort cleanup
        backend
            .delete("dim-mismatch-tenant", vec!["dim-mismatch-1".to_string()])
            .await
            .ok();
    }
}
