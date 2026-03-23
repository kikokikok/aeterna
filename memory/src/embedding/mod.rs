#[cfg(feature = "bedrock-provider")]
pub mod bedrock;
pub mod factory;
#[cfg(feature = "google-provider")]
pub mod google;
pub mod mock;
#[cfg(feature = "embedding-integration")]
pub mod openai;

#[cfg(feature = "bedrock-provider")]
pub use bedrock::BedrockEmbeddingService;
pub use factory::{
    EmbeddingFactoryError, EmbeddingProviderConfig, EmbeddingProviderType,
    create_embedding_service_from_env,
};
#[cfg(feature = "google-provider")]
pub use google::GoogleEmbeddingService;
pub use mock::MockEmbeddingService;
#[cfg(feature = "embedding-integration")]
pub use openai::OpenAIEmbeddingService;
