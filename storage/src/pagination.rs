//! Re-export pagination primitives from mk_core.
//!
//! The canonical definitions live in `mk_core::pagination` so they can be
//! referenced by both the core trait definitions and storage implementations
//! without circular dependencies.

pub use mk_core::pagination::{PaginatedResult, PaginationParams};
