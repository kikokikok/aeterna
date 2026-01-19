mod capture;
mod dedup;
mod note_gen;
mod promotion;
mod query;
mod resolution;
mod retrieval;

pub use capture::{CapturedError, ErrorCapture, ErrorCaptureConfig, ErrorContext, ErrorNormalizer};
pub use dedup::{
    DeduplicationConfig, DeduplicationError, DeduplicationMetrics, DeduplicationResult,
    ErrorSignatureIndex, InMemorySignatureStorage, IndexedSignature, ResolutionMerger, ScanResult,
    SignatureStorage,
};
pub use note_gen::{
    HindsightNoteGenerationMode, HindsightNoteGenerator, HindsightNoteGeneratorConfig,
    HindsightNoteRequest, HindsightNoteResult, HindsightPromptTemplate, HindsightPromptTemplates,
    NoteGenerationError,
};
pub use promotion::{
    HindsightPromoter, HindsightPromotionConfig, HindsightPromotionError, HindsightPromotionRequest,
};
pub use query::{HindsightMatch, HindsightQuery, HindsightQueryConfig};
pub use resolution::{
    ApplicationContext, ApplicationRecord, FailureContext, InMemoryResolutionStorage,
    ResolutionMetrics, ResolutionOutcome, ResolutionStorage, ResolutionStorageError,
    ResolutionTracker, ResolutionTrackerConfig,
};
pub use retrieval::{
    HindsightRetrievalConfig, HindsightRetrievalFilter, HindsightRetriever, ScoredHindsightNote,
};
