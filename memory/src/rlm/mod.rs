//! RLM (Recursive Language Model) Memory Navigation Infrastructure.
//!
//! This module provides internal infrastructure for handling complex memory search queries
//! through recursive decomposition and strategy-based execution.

pub mod executor;
pub mod router;
pub mod strategy;
pub mod trainer;

pub use executor::RlmExecutor;
pub use router::{ComplexityRouter, ComplexitySignals};
pub use strategy::DecompositionAction;
pub use trainer::DecompositionTrainer;
