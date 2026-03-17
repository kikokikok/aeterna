//! Direct backend connections for local development.
//!
//! This module wires the CLI directly to Qdrant and any OpenAI-compatible
//! embedding/LLM API so memory commands work without a server.
//!
//! ## Environment variables
//!
//! | Variable | Required | Default | Description |
//! |---|---|---|---|
//! | `QDRANT_URL` | No | `http://localhost:6334` | Qdrant gRPC endpoint |
//! | `QDRANT_COLLECTION` | No | `aeterna_memories` | Qdrant collection name |
//! | `EMBEDDING_API_BASE` | **Yes** | — | OpenAI-compatible embeddings endpoint |
//! | `EMBEDDING_API_KEY` | No | `not-needed` | API key (local servers usually don't need one) |
//! | `EMBEDDING_MODEL` | No | `text-embedding-nomic-embed-text-v1.5` | Embedding model name |
//! | `EMBEDDING_DIMENSION` | No | `768` | Embedding vector dimension |
//! | `LLM_API_BASE` | **Yes** | — | OpenAI-compatible chat/completions endpoint |
//! | `LLM_API_KEY` | No | `not-needed` | API key (local servers usually don't need one) |
//! | `LLM_MODEL` | No | `qwen3.5-35b-a3b` | Reasoning model name |
//! | `REASONING_TIMEOUT_MS` | No | `180000` | Reasoning timeout in milliseconds |

use async_trait::async_trait;
use config::MemoryConfig;
use memory::embedding::OpenAIEmbeddingService;
use memory::llm::OpenAILlmService;
use memory::manager::MemoryManager;
use memory::providers::qdrant::QdrantProvider;
use memory::reasoning::DefaultReflectiveReasoner;
use mk_core::traits::EmbeddingService;
use mk_core::traits::LlmService;
use mk_core::types::MemoryLayer;
use std::sync::Arc;

/// Adapts `OpenAILlmService` (Error = anyhow::Error) to the
/// `LlmService<Error = Box<dyn std::error::Error + Send + Sync>>` trait object expected by
/// `MemoryManager`.
struct BoxedLlmWrapper(OpenAILlmService);

#[async_trait]
impl LlmService for BoxedLlmWrapper {
    type Error = Box<dyn std::error::Error + Send + Sync>;

    async fn generate(&self, prompt: &str) -> Result<String, Self::Error> {
        self.0.generate(prompt).await.map_err(|e| e.into())
    }

    async fn analyze_drift(
        &self,
        content: &str,
        policies: &[mk_core::types::Policy],
    ) -> Result<mk_core::types::ValidationResult, Self::Error> {
        self.0
            .analyze_drift(content, policies)
            .await
            .map_err(|e| e.into())
    }
}

/// Create a fully-wired MemoryManager connected to local Qdrant + embedding service.
pub async fn create_memory_manager(
    enable_reasoning: bool,
) -> Result<MemoryManager, Box<dyn std::error::Error + Send + Sync>> {
    let embedding_service = create_embedding_service()?;
    let dimension = embedding_service.dimension();

    // Build Qdrant client
    let qdrant_url =
        std::env::var("QDRANT_URL").unwrap_or_else(|_| "http://localhost:6334".to_string());
    let collection_name =
        std::env::var("QDRANT_COLLECTION").unwrap_or_else(|_| "aeterna_memories".to_string());

    let qdrant_client = qdrant_client::Qdrant::from_url(&qdrant_url)
        .build()
        .map_err(|e| format!("Failed to connect to Qdrant at {}: {}", qdrant_url, e))?;

    // Create provider and ensure collection exists
    let provider = QdrantProvider::new(qdrant_client, collection_name, dimension);
    provider.ensure_collection().await?;

    let mut manager = MemoryManager::new().with_embedding_service(Arc::new(embedding_service));

    if enable_reasoning {
        let llm_service: Arc<
            dyn LlmService<Error = Box<dyn std::error::Error + Send + Sync>> + Send + Sync,
        > = Arc::new(BoxedLlmWrapper(create_llm_service()?));
        let reasoner = Arc::new(DefaultReflectiveReasoner::new(llm_service.clone()));
        let mut config = MemoryConfig::default();
        config.reasoning.enabled = true;
        config.reasoning.timeout_ms = std::env::var("REASONING_TIMEOUT_MS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(180_000);
        manager = manager
            .with_llm_service(llm_service)
            .with_reasoner(reasoner)
            .with_config(config);
    }

    let layers = [
        MemoryLayer::Agent,
        MemoryLayer::User,
        MemoryLayer::Session,
        MemoryLayer::Project,
        MemoryLayer::Team,
        MemoryLayer::Org,
        MemoryLayer::Company,
    ];

    for layer in layers {
        manager
            .register_provider(layer, Arc::new(provider.clone().with_layer_scope(layer)))
            .await;
    }

    Ok(manager)
}

fn create_embedding_service()
-> Result<OpenAIEmbeddingService, Box<dyn std::error::Error + Send + Sync>> {
    let api_key = std::env::var("EMBEDDING_API_KEY")
        .or_else(|_| std::env::var("OPENAI_API_KEY"))
        .unwrap_or_else(|_| "not-needed".to_string());
    let base_url = std::env::var("EMBEDDING_API_BASE").map_err(|_| {
        "EMBEDDING_API_BASE must be set to an OpenAI-compatible endpoint (e.g. http://localhost:11434/v1)"
    })?;
    let model = std::env::var("EMBEDDING_MODEL")
        .unwrap_or_else(|_| "text-embedding-nomic-embed-text-v1.5".to_string());
    let dimension = std::env::var("EMBEDDING_DIMENSION")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(768);

    Ok(OpenAIEmbeddingService::with_base_url(
        api_key, &base_url, &model, dimension,
    ))
}

fn create_llm_service() -> Result<OpenAILlmService, Box<dyn std::error::Error + Send + Sync>> {
    let api_key = std::env::var("LLM_API_KEY")
        .or_else(|_| std::env::var("OPENAI_API_KEY"))
        .unwrap_or_else(|_| "not-needed".to_string());
    let base_url = std::env::var("LLM_API_BASE").map_err(|_| {
        "LLM_API_BASE must be set to an OpenAI-compatible endpoint (e.g. http://localhost:11434/v1)"
    })?;
    let model = std::env::var("LLM_MODEL").unwrap_or_else(|_| "qwen3.5-35b-a3b".to_string());

    Ok(OpenAILlmService::with_base_url(api_key, &base_url, model))
}

/// Parse a layer string into a MemoryLayer.
pub fn parse_layer(s: &str) -> Result<MemoryLayer, String> {
    match s.to_lowercase().as_str() {
        "agent" => Ok(MemoryLayer::Agent),
        "user" => Ok(MemoryLayer::User),
        "session" => Ok(MemoryLayer::Session),
        "project" => Ok(MemoryLayer::Project),
        "team" => Ok(MemoryLayer::Team),
        "org" => Ok(MemoryLayer::Org),
        "company" => Ok(MemoryLayer::Company),
        _ => Err(format!(
            "Invalid layer: {}. Valid: agent, user, session, project, team, org, company",
            s
        )),
    }
}
