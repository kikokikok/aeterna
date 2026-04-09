// E2E tests for GoogleEmbeddingService against live Vertex AI.
//
// # Setup
//
// Requires a GCP project with Vertex AI Embedding API enabled.
//
// 1. Enable the Vertex AI API:
//    gcloud services enable aiplatform.googleapis.com --project=$GCP_PROJECT_ID
//
// 2. Grant the service account the `Vertex AI User` IAM role.
//
// 3. Export environment variables:
//    ```sh
//    export AETERNA_GOOGLE_PROJECT_ID="my-gcp-project"
//    export AETERNA_GOOGLE_LOCATION="us-central1"
//    export AETERNA_GOOGLE_EMBEDDING_MODEL="text-embedding-005"
//    # Authentication: ADC or manual token
//    export GOOGLE_APPLICATION_CREDENTIALS="/path/to/service-account.json"
//    # Or supply a bearer token directly:
//    export GOOGLE_ACCESS_TOKEN="$(gcloud auth print-access-token)"
//    ```
//
// 4. Run:
//    ```sh
//    cargo test -p memory --features google-provider \
//      --test embedding_google_e2e_test -- --ignored
//    ```

#[cfg(feature = "google-provider")]
mod google_embedding_e2e {
    use memory::embedding::google::GoogleEmbeddingService;
    use mk_core::traits::EmbeddingService;

    fn service_from_env() -> GoogleEmbeddingService {
        let project_id =
            std::env::var("AETERNA_GOOGLE_PROJECT_ID").expect("AETERNA_GOOGLE_PROJECT_ID not set");
        let location =
            std::env::var("AETERNA_GOOGLE_LOCATION").unwrap_or_else(|_| "us-central1".to_string());
        let model = std::env::var("AETERNA_GOOGLE_EMBEDDING_MODEL")
            .unwrap_or_else(|_| "text-embedding-005".to_string());

        GoogleEmbeddingService::new(project_id, location, model)
    }

    // -------------------------------------------------------------------------
    // Test 1: Live embedding call returns a 768-dimensional vector
    // -------------------------------------------------------------------------
    #[tokio::test]
    #[ignore = "requires GCP project + Vertex AI Embedding API - set AETERNA_GOOGLE_PROJECT_ID, AETERNA_GOOGLE_LOCATION, AETERNA_GOOGLE_EMBEDDING_MODEL, and GOOGLE_ACCESS_TOKEN or ADC"]
    async fn test_google_embed_generates_768_dim_vector() {
        let svc = service_from_env();

        let vector = svc
            .embed("Aeterna is a universal memory framework for enterprise AI agents")
            .await
            .expect("GoogleEmbeddingService::embed must succeed against live Vertex AI");

        assert_eq!(
            vector.len(),
            768,
            "text-embedding-005 must produce 768-dimensional vectors (got {})",
            vector.len()
        );

        // Sanity: values are finite and non-zero
        assert!(
            vector.iter().all(|v| v.is_finite()),
            "all embedding values must be finite"
        );
        assert!(
            vector.iter().any(|v| *v != 0.0),
            "embedding vector must not be all-zeros"
        );

        assert_eq!(svc.dimension(), 768, "dimension() must report 768");
    }

    // -------------------------------------------------------------------------
    // Test 2: Semantically similar texts produce closer vectors than dissimilar
    // -------------------------------------------------------------------------
    #[tokio::test]
    #[ignore = "requires GCP project + Vertex AI Embedding API - set AETERNA_GOOGLE_PROJECT_ID, AETERNA_GOOGLE_LOCATION, AETERNA_GOOGLE_EMBEDDING_MODEL, and GOOGLE_ACCESS_TOKEN or ADC"]
    async fn test_google_embed_semantic_similarity_ranking() {
        let svc = service_from_env();

        let query = "memory storage for AI agents";
        let similar = "persistent vector memory for autonomous agents";
        let dissimilar = "quarterly financial report for fiscal year 2024";

        let v_query = svc.embed(query).await.expect("embed query");
        let v_similar = svc.embed(similar).await.expect("embed similar");
        let v_dissimilar = svc.embed(dissimilar).await.expect("embed dissimilar");

        let sim_close = cosine_similarity(&v_query, &v_similar);
        let sim_far = cosine_similarity(&v_query, &v_dissimilar);

        assert!(
            sim_close > sim_far,
            "semantically similar text (score={:.4}) must rank above dissimilar text (score={:.4})",
            sim_close,
            sim_far
        );
    }

    // -------------------------------------------------------------------------
    // Test 3: Empty input is handled gracefully (either Ok or clear error)
    // -------------------------------------------------------------------------
    #[tokio::test]
    #[ignore = "requires GCP project + Vertex AI Embedding API - set AETERNA_GOOGLE_PROJECT_ID, AETERNA_GOOGLE_LOCATION, AETERNA_GOOGLE_EMBEDDING_MODEL, and GOOGLE_ACCESS_TOKEN or ADC"]
    async fn test_google_embed_empty_input_does_not_panic() {
        let svc = service_from_env();

        // The API may return an error or a valid (zero?) embedding for empty string.
        // Either outcome is acceptable — what is NOT acceptable is a panic.
        let result = svc.embed("").await;
        match result {
            Ok(v) => assert_eq!(
                v.len(),
                768,
                "if empty input succeeds, it must still return 768-dim vector"
            ),
            Err(e) => {
                let msg = e.to_string();
                assert!(
                    !msg.is_empty(),
                    "error for empty input must carry a non-empty message"
                );
            }
        }
    }

    // -------------------------------------------------------------------------
    // Helper: cosine similarity between two equal-length vectors
    // -------------------------------------------------------------------------
    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        assert_eq!(a.len(), b.len(), "vectors must have same dimension");
        let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }
        dot / (norm_a * norm_b)
    }
}
