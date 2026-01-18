//! # Note-Taking Agent
//!
//! Trajectory distillation to Markdown documentation.
//! Captures agent tool executions, distills learnings, and generates reusable
//! notes.

mod capture;
mod distiller;
mod generator;
mod lifecycle;
mod retrieval;

pub use capture::{
    AsyncCaptureMetrics, AsyncTrajectoryCapture, SensitivePatterns, SessionTrajectoryCapture,
    StorageBackendAdapter, TrajectoryCapture, TrajectoryConfig, TrajectoryEvent, TrajectoryFilter,
    TrajectoryStorage, TrajectoryStorageError
};
pub use distiller::{
    DistillationResult, DistillationTrigger, Distiller, DistillerConfig, ExtractedSection
};
pub use generator::{GeneratedNote, NoteGenerator, NoteGeneratorConfig, NoteTemplate};
pub use lifecycle::{
    AutoTransitionResult, LifecycleConfig, LifecycleTransitionError, NoteLifecycleManager,
    NoteStatus, NoteWithLifecycle
};

pub type NoteViewMode = crate::context_architect::ViewMode;
pub use retrieval::{
    NoteEmbedder, NoteIndex, NoteRetriever, RetrievalConfig, RetrievalFilter, ScoredNote
};
