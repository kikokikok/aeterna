pub mod mock;
#[cfg(feature = "embedding-integration")]
pub mod openai;

pub use mock::MockEmbeddingService;
#[cfg(feature = "embedding-integration")]
pub use openai::OpenAIEmbeddingService;
