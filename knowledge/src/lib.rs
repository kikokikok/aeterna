//! # Knowledge Repository
//!
//! Git-based knowledge management with governance.

pub mod api;
pub mod context_architect;
pub mod durable_events;
pub mod federation;
pub mod git_provider;
pub mod governance;
pub mod governance_client;
pub mod hindsight;
pub mod manager;
pub mod meta_agent;
pub mod note_taking;
pub mod pr_proposal_storage;
pub mod repository;
pub mod scheduler;
pub mod telemetry;
pub mod resolver;
pub mod tenant_repo_resolver;
