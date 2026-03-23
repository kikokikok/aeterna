#[cfg(feature = "llm-integration")]
pub use mock::MockLlmService;

pub mod mock;

#[cfg(feature = "llm-integration")]
pub mod aisdk;
#[cfg(feature = "bedrock-provider")]
pub mod bedrock;
#[cfg(feature = "llm-integration")]
pub mod extractor;
pub mod factory;
#[cfg(feature = "google-provider")]
pub mod google;
#[cfg(feature = "embedding-integration")]
pub mod openai;

#[cfg(feature = "llm-integration")]
pub use aisdk::AisdkLlmService;
#[cfg(feature = "bedrock-provider")]
pub use bedrock::BedrockLlmService;
#[cfg(feature = "llm-integration")]
pub use extractor::EntityExtractor;
pub use factory::{
    LlmFactoryError, LlmProviderConfig, LlmProviderType, create_llm_service_from_env,
};
#[cfg(feature = "google-provider")]
pub use google::GoogleLlmService;
#[cfg(feature = "embedding-integration")]
pub use openai::OpenAILlmService;
