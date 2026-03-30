//! # Code Search Integration Module
//!
//! Provides MCP tools for code intelligence via pluggable external backends.
//!
//! ## Architecture
//! - `client`: Pluggable `CodeIntelligenceBackend` trait + `CodeSearchClient` facade with circuit breaker
//! - `tools`: MCP tool implementations (codesearch_search, codesearch_trace_*, etc.)
//! - `types`: Type definitions for requests and responses
//!
//! ## Backends
//! - `McpCodeIntelligence`: HTTP JSON-RPC proxy to any MCP code intelligence server (JetBrains, VS Code, etc.)
//! - `MockCodeIntelligence`: In-memory mock for testing
//! - `NoBackend`: Graceful degradation with informative install instructions
//!
//! ## Tools Provided
//! - `codesearch_search`: Semantic code search using natural language queries
//! - `codesearch_trace_callers`: Find all functions that call a given symbol
//! - `codesearch_trace_callees`: Find all functions called by a given symbol
//! - `codesearch_graph`: Build call dependency graph for a symbol
//! - `codesearch_index_status`: Get indexing status for projects
//! - `codesearch_repo_request`: Request indexing for a new repository

pub mod client;
pub mod tools;
pub mod types;

pub use client::{
    CodeIntelligenceBackend, CodeSearchClient, CodeSearchConfig, McpCodeIntelligence,
    MockCodeIntelligence, NoBackend,
};
pub use tools::{
    CodeGraphTool, CodeIndexStatusTool, CodeSearchRepoRequestTool, CodeSearchTool,
    CodeTraceCalleesTool, CodeTraceCallersTool,
};
