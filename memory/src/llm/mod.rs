#[cfg(feature = "llm-integration")]
pub use mock::MockLlmService;

pub mod mock;

#[cfg(feature = "llm-integration")]
pub mod aisdk;
#[cfg(feature = "llm-integration")]
pub mod extractor;
#[cfg(feature = "embedding-integration")]
pub mod openai;

#[cfg(feature = "llm-integration")]
pub use aisdk::AisdkLlmService;
#[cfg(feature = "llm-integration")]
pub use extractor::EntityExtractor;
#[cfg(feature = "embedding-integration")]
pub use openai::OpenAILlmService;
