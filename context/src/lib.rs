//! Context auto-resolution for Aeterna.
//!
//! This crate provides automatic detection and resolution of tenant context
//! from multiple sources with precedence:
//!
//! 1. Explicit overrides (CLI flags, API params)
//! 2. Environment variables (`AETERNA_*`)
//! 3. Context file (`.aeterna/context.toml`)
//! 4. Git remote URL -> `project_id`
//! 5. Git config user.email -> `user_id`
//! 6. Organization defaults (future: from server)
//! 7. System defaults ("default"/"default")
//!
//! # Example
//!
//! ```rust,ignore
//! use context::{ContextResolver, ResolvedContext};
//!
//! // Auto-detect everything from current directory
//! let ctx = ContextResolver::new().resolve()?;
//! println!("Tenant: {} (from {})", ctx.tenant_id.value, ctx.tenant_id.source);
//!
//! // With explicit overrides
//! let ctx = ContextResolver::new()
//!     .with_override("tenant_id", "acme-corp")
//!     .with_override("hints", "no-llm,fast")
//!     .resolve()?;
//! ```

pub mod cedar;
mod resolver;
mod types;

pub use cedar::{
    AccessibleLayers, AuthorizationDecision, AuthorizationDiagnostics, AuthorizationRequest,
    AuthorizationResponse, CedarClient, CedarConfig, CedarError, Entity, EntityUid
};
pub use resolver::{CedarContextResolver, ContextError, ContextResolver};
pub use types::{
    ContextConfig, ContextSource, ResolvedContext, ResolvedValue, ServerConfig, StorageConfig
};
