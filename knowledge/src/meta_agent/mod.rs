mod build;
mod improve;
mod r#loop;
mod quality_gate;
mod result;
mod telemetry;
mod test;
mod time_budget;
mod types;

pub use build::{BuildPhase, BuildPhaseConfig, BuildPromptTemplate, BuildPromptTemplates};
pub use improve::{
    ImprovePhase, ImprovePhaseConfig, ImprovePromptTemplate, ImprovePromptTemplates
};
pub use r#loop::{
    MetaAgentLoop, MetaAgentLoopResult, MetaAgentLoopResultExtended, MetaAgentLoopState,
    MetaAgentLoopStateExtended, MetaAgentLoopWithBudget
};
pub use quality_gate::{
    CoverageConfig, LinterConfig, QualityGateConfig, QualityGateEvaluator, QualityGateResult,
    QualityGateSummary, QualityGateType
};
pub use result::{FailureContext, ResultHandler, ResultHandlingConfig, ResultHandlingOutcome};
pub use telemetry::MetaAgentTelemetrySink;
pub use test::{TestPhase, TestPhaseConfig};
pub use time_budget::{
    BudgetCheck, BudgetStatus, TimeBudget, TimeBudgetConfig, TimeBudgetExhaustedResult
};
pub use types::{
    BuildResult, ImproveAction, ImproveResult, MetaAgentConfig, MetaAgentFailureReport,
    MetaAgentSuccessReport, MetaAgentTelemetry, TestCommand, TestResult, TestStatus
};

pub use crate::hindsight::{HindsightRetrievalConfig, HindsightRetriever};
pub use crate::note_taking::{
    GeneratedNote, NoteEmbedder, NoteGenerator, NoteGeneratorConfig, NoteRetriever, RetrievalConfig
};

use crate::context_architect::ViewMode;
use crate::hindsight::{HindsightRetrievalFilter, ScoredHindsightNote};
use crate::note_taking::ScoredNote;
use mk_core::types::ErrorSignature;
use std::sync::Arc;
use storage::postgres::PostgresBackend;

use async_trait::async_trait;

#[async_trait]
pub trait NoteLookup: Send + Sync {
    async fn retrieve(&self, query: &str, limit: usize) -> Vec<String>;
}

#[async_trait]
pub trait HindsightLookup: Send + Sync {
    async fn retrieve(&self, query: &str, limit: usize) -> Vec<String>;
}

#[async_trait]
pub trait NoteStore: Send + Sync {
    async fn add_note(&self, note: GeneratedNote);

    async fn retrieve(&self, query: &str, limit: usize) -> Vec<String>;
}

#[async_trait]
pub trait HindsightStore: Send + Sync {
    async fn retrieve(&self, query: &str, limit: usize) -> Vec<String>;
}

pub struct InMemoryNoteStore<E: NoteEmbedder> {
    retriever: tokio::sync::Mutex<NoteRetriever<E>>,
    view_mode: ViewMode
}

impl<E: NoteEmbedder> InMemoryNoteStore<E> {
    pub fn new(retriever: NoteRetriever<E>, view_mode: ViewMode) -> Self {
        Self {
            retriever: tokio::sync::Mutex::new(retriever),
            view_mode
        }
    }
}

#[async_trait]
impl<E: NoteEmbedder> NoteStore for InMemoryNoteStore<E> {
    async fn add_note(&self, note: GeneratedNote) {
        let mut retriever = self.retriever.lock().await;
        let _ = retriever.add_note(note).await;
    }

    async fn retrieve(&self, query: &str, limit: usize) -> Vec<String> {
        let retriever = self.retriever.lock().await;
        let results = retriever.retrieve_relevant(query, limit).await;
        match results {
            Ok(scored) => format_notes_for_view(&scored, self.view_mode),
            Err(_) => Vec::new()
        }
    }
}

pub struct PostgresHindsightStore {
    storage: Arc<PostgresBackend>,
    config: HindsightRetrievalConfig,
    view_mode: ViewMode,
    tenant_id: String
}

impl PostgresHindsightStore {
    pub fn new(
        storage: Arc<PostgresBackend>,
        config: HindsightRetrievalConfig,
        view_mode: ViewMode,
        tenant_id: impl Into<String>
    ) -> Self {
        Self {
            storage,
            config,
            view_mode,
            tenant_id: tenant_id.into()
        }
    }
}

#[async_trait]
impl HindsightStore for PostgresHindsightStore {
    async fn retrieve(&self, query: &str, limit: usize) -> Vec<String> {
        let signature = ErrorSignature {
            error_type: query.to_string(),
            message_pattern: query.to_string(),
            stack_patterns: vec![],
            context_patterns: vec![],
            embedding: None
        };
        let retriever = HindsightRetriever::new(self.storage.clone(), self.config.clone());
        let filter = HindsightRetrievalFilter::default();
        let results = retriever
            .retrieve(&self.tenant_id, &signature, Some(&filter))
            .await;
        match results {
            Ok(scored) => {
                let limited = scored.into_iter().take(limit).collect::<Vec<_>>();
                format_hindsight_for_view(&limited, self.view_mode)
            }
            Err(_) => Vec::new()
        }
    }
}

pub fn format_notes_for_view(scored: &[ScoredNote], view_mode: ViewMode) -> Vec<String> {
    let _ = view_mode;
    scored
        .iter()
        .map(|entry| entry.note.content.clone())
        .collect()
}

pub fn format_hindsight_for_view(
    scored: &[ScoredHindsightNote],
    view_mode: ViewMode
) -> Vec<String> {
    let _ = view_mode;
    scored
        .iter()
        .map(|entry| entry.note.content.clone())
        .collect()
}
