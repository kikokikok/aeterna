//! # Configuration System
//!
//! Centralized configuration management for the Memory-Knowledge system.
//!
//! This crate provides:
//! - Configuration structures for all system components
//! - Environment variable loading (12-factor app principles)
//! - Configuration file loading (TOML/YAML)
//! - Configuration precedence (CLI > env > file > defaults)
//! - Configuration validation
//! - Hot reload functionality
//!
//! # Best Practices
//!
//! - Uses `validator` crate for input validation
//! - Follows 12-factor app configuration principles
//! - Provides clear error messages for invalid configuration
//! - Thread-safe configuration access

pub mod cca;
pub mod config;
pub mod file_loader;
pub mod hot_reload;
pub mod loader;
pub mod precedence;

pub use cca::{
    CcaConfig, ContextArchitectConfig, HindsightConfig, MetaAgentConfig, NoteTakingConfig,
};
pub use config::{
    Config, DeploymentConfig, GraphConfig, MemoryConfig, ObservabilityConfig, ProviderConfig,
    ReasoningConfig, SyncConfig, ToolConfig,
};
pub use file_loader::{load_from_file, load_from_toml, load_from_yaml};
pub use hot_reload::watch_config;
pub use loader::load_from_env;
pub use precedence::merge_configs;
pub use validator::Validate;
