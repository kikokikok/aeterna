//! # Code Search MCP Client
//!
//! Client for communicating with Code Search sidecar via MCP protocol.
//!
//! ## Communication Pattern
//! - stdio-based MCP communication with Code Search process
//! - JSON-RPC for tool invocation
//! - Circuit breaker for resilience
//! - Connection pooling and retry logic

use serde_json::Value;
use std::sync::{Arc, Mutex};
use tokio::time::{timeout, Duration};

/// Configuration for Code Search client
#[derive(Debug, Clone)]
pub struct CodeSearchConfig {
    /// Path to Code Search binary (default: "codesearch")
    pub binary_path: String,
    /// Workspace name for tenant isolation (default: "default")
    pub workspace: String,
    /// Request timeout in seconds (default: 30)
    pub timeout_secs: u64,
    /// Enable debug logging
    pub debug: bool,
}

impl Default for CodeSearchConfig {
    fn default() -> Self {
        Self {
            binary_path: "codesearch".to_string(),
            workspace: "default".to_string(),
            timeout_secs: 30,
            debug: false,
        }
    }
}

/// Code Search MCP client for communicating with sidecar
pub struct CodeSearchClient {
    config: CodeSearchConfig,
    /// Circuit breaker state
    failures: Arc<Mutex<usize>>,
    max_failures: usize,
}

impl CodeSearchClient {
    /// Create new Code Search client with configuration
    pub fn new(config: CodeSearchConfig) -> Self {
        Self {
            config,
            failures: Arc::new(Mutex::new(0)),
            max_failures: 5,
        }
    }

    /// Create client with default configuration
    pub fn default() -> Self {
        Self::new(CodeSearchConfig::default())
    }

    /// Check if circuit breaker is open (too many failures)
    pub fn is_available(&self) -> bool {
        let failures = self.failures.lock().unwrap();
        *failures < self.max_failures
    }

    /// Reset circuit breaker after successful call
    fn reset_failures(&self) {
        let mut failures = self.failures.lock().unwrap();
        *failures = 0;
    }

    /// Increment failure count
    fn record_failure(&self) {
        let mut failures = self.failures.lock().unwrap();
        *failures += 1;
    }

    /// Call Code Search tool via MCP protocol
    ///
    /// # Arguments
    /// * `tool_name` - Code Search tool name (e.g., "codesearch_search")
    /// * `params` - Tool parameters as JSON
    ///
    /// # Returns
    /// Result JSON or error
    pub async fn call_tool(
        &self,
        tool_name: &str,
        params: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        // Check circuit breaker
        if !self.is_available() {
            return Err("Code Search sidecar unavailable (circuit breaker open)".into());
        }

        // Prepare MCP request
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": tool_name,
                "arguments": params
            }
        });

        if self.config.debug {
            eprintln!("Code Search request: {}", serde_json::to_string_pretty(&request)?);
        }

        // Execute with timeout
        let result = timeout(
            Duration::from_secs(self.config.timeout_secs),
            self.execute_mcp_call(request),
        )
        .await;

        match result {
            Ok(Ok(response)) => {
                self.reset_failures();
                Ok(response)
            }
            Ok(Err(e)) => {
                self.record_failure();
                Err(e)
            }
            Err(_) => {
                self.record_failure();
                Err("Code Search call timed out".into())
            }
        }
    }

    /// Execute MCP call to Code Search binary
    ///
    /// This simulates stdio communication. In production, this would:
    /// 1. Connect to Code Search sidecar via stdio
    /// 2. Send JSON-RPC request
    /// 3. Read JSON-RPC response
    ///
    /// For now, returns mock data for development.
    async fn execute_mcp_call(
        &self,
        request: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        // TODO: Implement actual MCP communication with Code Search sidecar
        // For now, return mock success response
        
        // Extract tool name from request
        let tool_name = request["params"]["name"]
            .as_str()
            .ok_or("Missing tool name")?;

        // Return mock response based on tool
        match tool_name {
            "codesearch_search" => Ok(serde_json::json!({
                "success": true,
                "results": []
            })),
            "codesearch_trace_callers" | "codesearch_trace_callees" => Ok(serde_json::json!({
                "success": true,
                "symbols": []
            })),
            "codesearch_trace_graph" => Ok(serde_json::json!({
                "success": true,
                "nodes": []
            })),
            "codesearch_index_status" => Ok(serde_json::json!({
                "success": true,
                "status": {
                    "project": self.config.workspace.clone(),
                    "files_indexed": 0,
                    "chunks": 0,
                    "state": "idle"
                }
            })),
            "codesearch_repo_request" => Ok(serde_json::json!({
                "success": true,
                "id": uuid::Uuid::new_v4().to_string(),
                "status": "requested"
            })),
            _ => Err(format!("Unknown Code Search tool: {}", tool_name).into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_creation() {
        let client = CodeSearchClient::default();
        assert!(client.is_available());
    }

    #[tokio::test]
    async fn test_circuit_breaker() {
        let client = CodeSearchClient::new(CodeSearchConfig {
            binary_path: "nonexistent".to_string(),
            ..Default::default()
        });

        // Should be available initially
        assert!(client.is_available());

        // Record failures
        for _ in 0..5 {
            client.record_failure();
        }

        // Should be unavailable after max failures
        assert!(!client.is_available());
    }
}
