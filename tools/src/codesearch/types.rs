//! # Code Search Type Definitions
//!
//! Types for Code Search requests and responses.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Code search result
#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone)]
pub struct CodeChunk {
    /// File path relative to project root
    pub file: String,
    /// Starting line number
    pub line: usize,
    /// Ending line number (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_line: Option<usize>,
    /// Code content
    pub content: String,
    /// Relevance score (0.0 to 1.0)
    pub score: f32,
    /// Language detected
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// Additional metadata
    #[serde(default)]
    pub metadata: serde_json::Map<String, Value>,
}

/// Function/symbol reference in call graph
#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone)]
pub struct SymbolReference {
    /// Symbol name (function, class, method)
    pub symbol: String,
    /// File path
    pub file: String,
    /// Line number where symbol is defined/called
    pub line: usize,
    /// Symbol kind (function, method, class, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// Signature (for functions)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

/// Call graph node
#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone)]
pub struct CallGraphNode {
    /// Symbol reference
    pub symbol: SymbolReference,
    /// Direct callers
    #[serde(default)]
    pub callers: Vec<SymbolReference>,
    /// Direct callees
    #[serde(default)]
    pub callees: Vec<SymbolReference>,
    /// Depth from root (0 = starting symbol)
    pub depth: usize,
}

/// Index status for a project
#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone)]
pub struct IndexStatus {
    /// Project name/path
    pub project: String,
    /// Total files indexed
    pub files_indexed: usize,
    /// Total code chunks
    pub chunks: usize,
    /// Last indexed timestamp (ISO 8601)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_indexed: Option<String>,
    /// Indexing state (idle, indexing, error)
    pub state: String,
    /// Error message if state is error
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Code Search MCP request structure (for proxying)
#[derive(Serialize, Deserialize, Debug)]
pub struct CodeSearchRequest {
    /// Tool name (codesearch_search, codesearch_trace_callers, etc.)
    pub tool: String,
    /// Tool parameters
    pub params: Value,
}

/// Code Search MCP response structure (for proxying)
#[derive(Serialize, Deserialize, Debug)]
pub struct CodeSearchResponse {
    /// Success flag
    pub success: bool,
    /// Response data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    /// Error message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
