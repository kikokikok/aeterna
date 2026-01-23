//! # OPAL Data Fetcher
//!
//! This crate provides an HTTP server that exposes organizational data from PostgreSQL
//! to OPAL (Open Policy Administration Layer) and Cedar Agent for policy decisions.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
//! │   PostgreSQL    │────►│  OPAL Fetcher   │────►│   Cedar Agent   │
//! │  (Referential)  │     │  (This crate)   │     │                 │
//! └─────────────────┘     └─────────────────┘     └─────────────────┘
//!         │                       ▲
//!         │ pg_notify             │ HTTP
//!         └───────────────────────┘
//! ```
//!
//! ## Endpoints
//!
//! - `GET /v1/hierarchy` - Returns organizational hierarchy as Cedar entities
//! - `GET /v1/users` - Returns users with memberships as Cedar entities
//! - `GET /v1/agents` - Returns agents with delegation chains as Cedar entities
//! - `GET /health` - Health check endpoint
//! - `GET /metrics` - Prometheus metrics endpoint
//!
//! ## Real-time Updates
//!
//! The fetcher listens to PostgreSQL NOTIFY events via the `referential_changes` channel
//! and publishes updates to OPAL via its PubSub mechanism.

pub mod entities;
pub mod error;
pub mod handlers;
pub mod listener;
pub mod routes;
pub mod server;
pub mod state;

pub use error::FetcherError;
pub use server::OpalFetcherServer;
pub use state::AppState;
