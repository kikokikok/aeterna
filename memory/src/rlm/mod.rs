//! RLM (Recursive Language Model) Memory Navigation Infrastructure.
//!
//! This module provides internal infrastructure for handling complex memory
//! search queries through recursive decomposition and strategy-based execution.

pub mod bootstrap;
pub mod combined_trainer;
pub mod executor;
pub mod router;
pub mod strategy;
pub mod trainer;

pub use bootstrap::{BootstrapTaskTemplate, BootstrapTrainer, generate_bootstrap_tasks};
pub use combined_trainer::CombinedMemoryTrainer;
pub use executor::RlmExecutor;
pub use router::{ComplexityRouter, ComplexitySignals};
pub use strategy::DecompositionAction;
pub use trainer::DecompositionTrainer;
