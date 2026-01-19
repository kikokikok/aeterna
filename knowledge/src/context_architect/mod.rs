//! # Context Architect
//!
//! Hierarchical context compression for efficient memory storage.
//! Implements the CCA (Confucius Code Agent) Context Architect pattern.

mod assembler;
mod budget;
mod compressor;
mod failure_handling;
mod generator;
mod prompts;
mod triggers;

pub use assembler::{
    AssembledContext, AssemblerConfig, ContextAssembler, ContextEntry, ContextMetadata,
    ContextView, SummarySource, cosine_similarity,
};
pub use budget::{
    BudgetCheck, BudgetError, BudgetExhaustedAction, BudgetMetrics, BudgetStatus, BudgetTracker,
    BudgetTrackerConfig, QueuedRequest, SummarizationBudget, TieredModelConfig,
};
pub use compressor::{
    CompressedEntry, CompressedLayer, CompressionResult, CompressorConfig, HierarchicalCompressor,
    LayerContent, LayerEntry, ViewMode,
};
pub use failure_handling::{
    CachedSummaryStore, CircuitBreaker, CircuitBreakerConfig, CircuitState, FailureMetrics,
    RetryConfig, alert_on_consecutive_failures, retry_with_backoff,
};
pub use generator::{
    BatchSummaryRequest, BatchSummaryResult, BatchedSummarizer,
    BatchedSummaryResult as BatchedResult, BudgetAwareSummaryGenerator, BudgetAwareSummaryRequest,
    BudgetAwareSummaryResult, GenerationError, LayerBatchResult, SummaryGenerator,
    SummaryGeneratorConfig, SummaryRequest, SummaryResult, estimate_tokens,
};
pub use prompts::{PromptTemplate, PromptTemplates};
pub use triggers::{
    EntryState, SummaryState, SummaryTriggerMonitor, TriggerMonitorConfig, TriggerReason,
    TriggerResult,
};

use async_trait::async_trait;

#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn complete(&self, prompt: &str) -> Result<String, LlmError>;

    async fn complete_with_system(&self, system: &str, user: &str) -> Result<String, LlmError>;
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum LlmError {
    #[error("API request failed: {0}")]
    RequestFailed(String),

    #[error("Rate limited: retry after {retry_after_secs} seconds")]
    RateLimited { retry_after_secs: u64 },

    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    #[error("Provider not configured: {0}")]
    NotConfigured(String),

    #[error("Timeout after {0} seconds")]
    Timeout(u64),
}
