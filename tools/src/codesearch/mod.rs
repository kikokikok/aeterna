//! # Code Search Integration Module
//!
//! Provides MCP tools for semantic code search and call graph analysis via Code Search sidecar.
//!
//! ## Architecture
//! - `client`: MCP client for communicating with Code Search sidecar
//! - `tools`: MCP tool implementations (code_search, code_trace_*, etc.)
//! - `types`: Type definitions for requests and responses
//!
//! ## Tools Provided
//! - `code_search`: Semantic code search using natural language queries
//! - `code_trace_callers`: Find all functions that call a given symbol
//! - `code_trace_callees`: Find all functions called by a given symbol
//! - `code_graph`: Build call dependency graph for a symbol
//! - `code_index_status`: Get indexing status for projects

pub mod client;
pub mod tools;
pub mod types;

pub use client::CodeSearchClient;
pub use tools::{
    CodeGraphTool, CodeIndexStatusTool, CodeSearchRepoRequestTool, CodeSearchTool,
    CodeTraceCalleesTool, CodeTraceCallersTool,
};
