//! Shared test fixtures for Aeterna workspace.
//!
//! Provides single, shared instances of testcontainers across all test files:
//! - PostgreSQL (port 5432)
//! - Redis (port 6379)
//! - Qdrant (ports 6333/6334)
//! - MinIO (port 9000)
//!
//! Each fixture is lazily initialized once per test process and automatically
//! cleaned up when the process exits.

mod fixtures;

pub use fixtures::*;
