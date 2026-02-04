//! # Code Search MCP Tool Implementations
//!
//! MCP tools that proxy to Code Search sidecar for semantic code search and call graph analysis.

use crate::codesearch::client::CodeSearchClient;
use crate::tools::Tool;
use async_trait::async_trait;
use mk_core::types::TenantContext;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::Arc;
use validator::Validate;

// ============================================================================
// Code Search Tool
// ============================================================================

pub struct CodeSearchTool {
    client: Arc<CodeSearchClient>,
}

impl CodeSearchTool {
    pub fn new(client: Arc<CodeSearchClient>) -> Self {
        Self { client }
    }
}

#[derive(Serialize, Deserialize, JsonSchema, Validate)]
pub struct CodeSearchParams {
    /// Natural language query or code pattern
    pub query: String,
    /// Maximum number of results (default: 10)
    #[serde(default)]
    pub limit: Option<usize>,
    /// Minimum relevance score threshold (0.0 to 1.0, default: 0.7)
    #[serde(default)]
    pub threshold: Option<f32>,
    /// File path pattern filter (optional)
    #[serde(default)]
    pub file_pattern: Option<String>,
    /// Language filter (e.g., "rust", "python", "go")
    #[serde(default)]
    pub language: Option<String>,
    /// Tenant context for isolation
    #[serde(rename = "tenantContext")]
    pub tenant_context: Option<TenantContext>,
}

#[async_trait]
impl Tool for CodeSearchTool {
    fn name(&self) -> &str {
        "codesearch_search"
    }

    fn description(&self) -> &str {
        "Search codebase using semantic search with natural language queries. \
         Returns relevant code chunks with file paths, line numbers, and relevance scores."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Natural language query or code pattern to search for"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of results (default: 10)",
                    "minimum": 1,
                    "maximum": 100
                },
                "threshold": {
                    "type": "number",
                    "description": "Minimum relevance score (0.0 to 1.0, default: 0.7)",
                    "minimum": 0.0,
                    "maximum": 1.0
                },
                "filePattern": {
                    "type": "string",
                    "description": "Optional file path pattern filter (glob)"
                },
                "language": {
                    "type": "string",
                    "description": "Optional language filter (rust, python, go, etc.)"
                },
                "tenantContext": {
                    "$ref": "#/definitions/TenantContext"
                }
            },
            "required": ["query"]
        })
    }

    async fn call(&self, params: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let p: CodeSearchParams = serde_json::from_value(params)?;
        p.validate()?;

        // Check if Code Search is available
        if !self.client.is_available() {
            return Ok(json!({
                "success": false,
                "error": "Code Search sidecar is currently unavailable",
                "results": []
            }));
        }

        // Prepare Code Search request
        let codesearch_params = json!({
            "query": p.query,
            "limit": p.limit.unwrap_or(10),
            "threshold": p.threshold.unwrap_or(0.7),
            "file_pattern": p.file_pattern,
            "language": p.language
        });

        // Call Code Search
        match self.client.call_tool("codesearch_search", codesearch_params).await {
            Ok(response) => {
                Ok(json!({
                    "success": true,
                    "results": response.get("results").unwrap_or(&json!([]))
                }))
            }
            Err(e) => Ok(json!({
                "success": false,
                "error": e.to_string(),
                "results": []
            })),
        }
    }
}

// ============================================================================
// Code Trace Callers Tool
// ============================================================================

pub struct CodeTraceCallersTool {
    client: Arc<CodeSearchClient>,
}

impl CodeTraceCallersTool {
    pub fn new(client: Arc<CodeSearchClient>) -> Self {
        Self { client }
    }
}

#[derive(Serialize, Deserialize, JsonSchema, Validate)]
pub struct CodeTraceCallersParams {
    /// Symbol name to trace (function, method, class)
    pub symbol: String,
    /// File path where symbol is defined (optional, improves accuracy)
    #[serde(default)]
    pub file: Option<String>,
    /// Include indirect callers (default: false)
    #[serde(default)]
    pub recursive: Option<bool>,
    /// Maximum depth for recursive tracing (default: 3)
    #[serde(default)]
    pub max_depth: Option<usize>,
    /// Tenant context
    #[serde(rename = "tenantContext")]
    pub tenant_context: Option<TenantContext>,
}

#[async_trait]
impl Tool for CodeTraceCallersTool {
    fn name(&self) -> &str {
        "codesearch_trace_callers"
    }

    fn description(&self) -> &str {
        "Find all functions/methods that call a given symbol. \
         Useful for impact analysis and refactoring safety checks."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "symbol": {
                    "type": "string",
                    "description": "Symbol name to trace (function, method, class)"
                },
                "file": {
                    "type": "string",
                    "description": "Optional file path where symbol is defined"
                },
                "recursive": {
                    "type": "boolean",
                    "description": "Include indirect callers (default: false)"
                },
                "maxDepth": {
                    "type": "integer",
                    "description": "Maximum depth for recursive tracing (default: 3)",
                    "minimum": 1,
                    "maximum": 10
                },
                "tenantContext": {
                    "$ref": "#/definitions/TenantContext"
                }
            },
            "required": ["symbol"]
        })
    }

    async fn call(&self, params: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let p: CodeTraceCallersParams = serde_json::from_value(params)?;
        p.validate()?;

        if !self.client.is_available() {
            return Ok(json!({
                "success": false,
                "error": "Code Search sidecar is currently unavailable",
                "callers": []
            }));
        }

        let codesearch_params = json!({
            "symbol": p.symbol,
            "file": p.file,
            "recursive": p.recursive.unwrap_or(false),
            "max_depth": p.max_depth.unwrap_or(3)
        });

        match self.client.call_tool("codesearch_trace_callers", codesearch_params).await {
            Ok(response) => Ok(json!({
                "success": true,
                "callers": response.get("symbols").unwrap_or(&json!([]))
            })),
            Err(e) => Ok(json!({
                "success": false,
                "error": e.to_string(),
                "callers": []
            })),
        }
    }
}

// ============================================================================
// Code Trace Callees Tool
// ============================================================================

pub struct CodeTraceCalleesTool {
    client: Arc<CodeSearchClient>,
}

impl CodeTraceCalleesTool {
    pub fn new(client: Arc<CodeSearchClient>) -> Self {
        Self { client }
    }
}

#[derive(Serialize, Deserialize, JsonSchema, Validate)]
pub struct CodeTraceCalleesParams {
    /// Symbol name to trace
    pub symbol: String,
    /// File path where symbol is defined (optional)
    #[serde(default)]
    pub file: Option<String>,
    /// Include indirect callees (default: false)
    #[serde(default)]
    pub recursive: Option<bool>,
    /// Maximum depth for recursive tracing (default: 3)
    #[serde(default)]
    pub max_depth: Option<usize>,
    /// Tenant context
    #[serde(rename = "tenantContext")]
    pub tenant_context: Option<TenantContext>,
}

#[async_trait]
impl Tool for CodeTraceCalleesTool {
    fn name(&self) -> &str {
        "codesearch_trace_callees"
    }

    fn description(&self) -> &str {
        "Find all functions/methods called by a given symbol. \
         Useful for understanding dependencies and execution flow."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "symbol": {
                    "type": "string",
                    "description": "Symbol name to trace"
                },
                "file": {
                    "type": "string",
                    "description": "Optional file path where symbol is defined"
                },
                "recursive": {
                    "type": "boolean",
                    "description": "Include indirect callees (default: false)"
                },
                "maxDepth": {
                    "type": "integer",
                    "description": "Maximum depth for recursive tracing (default: 3)",
                    "minimum": 1,
                    "maximum": 10
                },
                "tenantContext": {
                    "$ref": "#/definitions/TenantContext"
                }
            },
            "required": ["symbol"]
        })
    }

    async fn call(&self, params: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let p: CodeTraceCalleesParams = serde_json::from_value(params)?;
        p.validate()?;

        if !self.client.is_available() {
            return Ok(json!({
                "success": false,
                "error": "Code Search sidecar is currently unavailable",
                "callees": []
            }));
        }

        let codesearch_params = json!({
            "symbol": p.symbol,
            "file": p.file,
            "recursive": p.recursive.unwrap_or(false),
            "max_depth": p.max_depth.unwrap_or(3)
        });

        match self.client.call_tool("codesearch_trace_callees", codesearch_params).await {
            Ok(response) => Ok(json!({
                "success": true,
                "callees": response.get("symbols").unwrap_or(&json!([]))
            })),
            Err(e) => Ok(json!({
                "success": false,
                "error": e.to_string(),
                "callees": []
            })),
        }
    }
}

// ============================================================================
// Code Graph Tool
// ============================================================================

pub struct CodeGraphTool {
    client: Arc<CodeSearchClient>,
}

impl CodeGraphTool {
    pub fn new(client: Arc<CodeSearchClient>) -> Self {
        Self { client }
    }
}

#[derive(Serialize, Deserialize, JsonSchema, Validate)]
pub struct CodeGraphParams {
    /// Symbol name to build graph around
    pub symbol: String,
    /// File path where symbol is defined (optional)
    #[serde(default)]
    pub file: Option<String>,
    /// Graph depth (1 = direct neighbors, 2 = neighbors of neighbors, etc.)
    #[serde(default)]
    pub depth: Option<usize>,
    /// Include callers in graph (default: true)
    #[serde(default)]
    pub include_callers: Option<bool>,
    /// Include callees in graph (default: true)
    #[serde(default)]
    pub include_callees: Option<bool>,
    /// Tenant context
    #[serde(rename = "tenantContext")]
    pub tenant_context: Option<TenantContext>,
}

#[async_trait]
impl Tool for CodeGraphTool {
    fn name(&self) -> &str {
        "codesearch_graph"
    }

    fn description(&self) -> &str {
        "Build a call dependency graph for a symbol, showing both callers and callees. \
         Returns a graph structure that can be visualized or analyzed."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "symbol": {
                    "type": "string",
                    "description": "Symbol name to build graph around"
                },
                "file": {
                    "type": "string",
                    "description": "Optional file path where symbol is defined"
                },
                "depth": {
                    "type": "integer",
                    "description": "Graph depth (default: 2)",
                    "minimum": 1,
                    "maximum": 5
                },
                "includeCallers": {
                    "type": "boolean",
                    "description": "Include callers in graph (default: true)"
                },
                "includeCallees": {
                    "type": "boolean",
                    "description": "Include callees in graph (default: true)"
                },
                "tenantContext": {
                    "$ref": "#/definitions/TenantContext"
                }
            },
            "required": ["symbol"]
        })
    }

    async fn call(&self, params: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let p: CodeGraphParams = serde_json::from_value(params)?;
        p.validate()?;

        if !self.client.is_available() {
            return Ok(json!({
                "success": false,
                "error": "Code Search sidecar is currently unavailable",
                "nodes": []
            }));
        }

        let codesearch_params = json!({
            "symbol": p.symbol,
            "file": p.file,
            "depth": p.depth.unwrap_or(2),
            "include_callers": p.include_callers.unwrap_or(true),
            "include_callees": p.include_callees.unwrap_or(true)
        });

        match self.client.call_tool("codesearch_trace_graph", codesearch_params).await {
            Ok(response) => Ok(json!({
                "success": true,
                "nodes": response.get("nodes").unwrap_or(&json!([]))
            })),
            Err(e) => Ok(json!({
                "success": false,
                "error": e.to_string(),
                "nodes": []
            })),
        }
    }
}

// ============================================================================
// Code Index Status Tool
// ============================================================================

pub struct CodeIndexStatusTool {
    client: Arc<CodeSearchClient>,
}

impl CodeIndexStatusTool {
    pub fn new(client: Arc<CodeSearchClient>) -> Self {
        Self { client }
    }
}

#[derive(Serialize, Deserialize, JsonSchema, Validate)]
pub struct CodeIndexStatusParams {
    /// Project name/path (optional, returns all if not specified)
    #[serde(default)]
    pub project: Option<String>,
    /// Tenant context
    #[serde(rename = "tenantContext")]
    pub tenant_context: Option<TenantContext>,
}

#[async_trait]
impl Tool for CodeIndexStatusTool {
    fn name(&self) -> &str {
        "codesearch_index_status"
    }

    fn description(&self) -> &str {
        "Get indexing status for projects, including total files, chunks, and last indexed timestamp."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "project": {
                    "type": "string",
                    "description": "Optional project name/path (returns all if not specified)"
                },
                "tenantContext": {
                    "$ref": "#/definitions/TenantContext"
                }
            }
        })
    }

    async fn call(&self, params: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let p: CodeIndexStatusParams = serde_json::from_value(params)?;
        p.validate()?;

        if !self.client.is_available() {
            return Ok(json!({
                "success": false,
                "error": "Code Search sidecar is currently unavailable",
                "status": null
            }));
        }

        let codesearch_params = json!({
            "project": p.project
        });

        match self.client.call_tool("codesearch_index_status", codesearch_params).await {
            Ok(response) => Ok(json!({
                "success": true,
                "status": response.get("status").unwrap_or(&json!(null))
            })),
            Err(e) => Ok(json!({
                "success": false,
                "error": e.to_string(),
                "status": null
            })),
        }
    }
}

// ============================================================================
// Code Search Repo Request Tool
// ============================================================================

pub struct CodeSearchRepoRequestTool {
    client: Arc<CodeSearchClient>,
}

impl CodeSearchRepoRequestTool {
    pub fn new(client: Arc<CodeSearchClient>) -> Self {
        Self { client }
    }
}

#[derive(Serialize, Deserialize, JsonSchema, Validate)]
pub struct CodeSearchRepoRequestParams {
    /// Name for the repository
    pub name: String,
    /// Repository type (local, remote, hybrid)
    pub r#type: String,
    /// Remote URL (for remote/hybrid)
    #[serde(default)]
    pub url: Option<String>,
    /// Local path (for local/hybrid)
    #[serde(default)]
    pub path: Option<String>,
    /// Update strategy (hook, job, manual)
    #[serde(default)]
    pub strategy: Option<String>,
    /// Sync interval in minutes (default: 15)
    #[serde(default)]
    pub interval: Option<i32>,
    /// Tenant context
    #[serde(rename = "tenantContext")]
    pub tenant_context: Option<TenantContext>,
}

#[async_trait]
impl Tool for CodeSearchRepoRequestTool {
    fn name(&self) -> &str {
        "codesearch_repo_request"
    }

    fn description(&self) -> &str {
        "Request indexing for a new repository. Supports local, remote, and hybrid types with various sync strategies."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Name for the repository"
                },
                "type": {
                    "type": "string",
                    "enum": ["local", "remote", "hybrid"],
                    "description": "Repository type"
                },
                "url": {
                    "type": "string",
                    "description": "Remote URL (required for remote/hybrid)"
                },
                "path": {
                    "type": "string",
                    "description": "Local path (required for local/hybrid)"
                },
                "strategy": {
                    "type": "string",
                    "enum": ["hook", "job", "manual"],
                    "description": "Update strategy (default: manual)"
                },
                "interval": {
                    "type": "integer",
                    "description": "Sync interval in minutes (default: 15)"
                },
                "tenantContext": {
                    "$ref": "#/definitions/TenantContext"
                }
            },
            "required": ["name", "type"]
        })
    }

    async fn call(&self, params: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let p: CodeSearchRepoRequestParams = serde_json::from_value(params)?;
        p.validate()?;

        if !self.client.is_available() {
            return Ok(json!({
                "success": false,
                "error": "Code Search sidecar is currently unavailable"
            }));
        }

        let codesearch_params = json!({
            "name": p.name,
            "type": p.r#type,
            "url": p.url,
            "path": p.path,
            "strategy": p.strategy,
            "interval": p.interval
        });

        match self.client.call_tool("codesearch_repo_request", codesearch_params).await {
            Ok(response) => Ok(json!({
                "success": true,
                "request": response
            })),
            Err(e) => Ok(json!({
                "success": false,
                "error": e.to_string()
            })),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codesearch::client::CodeSearchConfig;

    #[tokio::test]
    async fn test_code_search_tool() {
        let client = Arc::new(CodeSearchClient::new(CodeSearchConfig::default()));
        let tool = CodeSearchTool::new(client);
        
        assert_eq!(tool.name(), "codesearch_search");
        assert!(!tool.description().is_empty());
    }

    #[tokio::test]
    async fn test_code_trace_callers_tool() {
        let client = Arc::new(CodeSearchClient::new(CodeSearchConfig::default()));
        let tool = CodeTraceCallersTool::new(client);
        
        assert_eq!(tool.name(), "codesearch_trace_callers");
        assert!(!tool.description().is_empty());
    }
}
