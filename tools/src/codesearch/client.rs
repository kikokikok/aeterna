//! # Code Search MCP Client
//!
//! Client for communicating with Code Search sidecar via MCP protocol.
//!
//! ## Communication Pattern
//! - stdio-based MCP communication with Code Search process
//! - JSON-RPC for tool invocation
//! - Circuit breaker for resilience
//! - Spawn-per-call: each request spawns a new `codesearch mcp serve` process

use serde_json::Value;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::{Duration, timeout};

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
    /// Extra arguments to pass to the Code Search binary (default: ["mcp", "serve"])
    pub mcp_args: Vec<String>,
    /// When true, use mock responses instead of spawning the binary (default: false)
    pub use_mock: bool,
}

impl Default for CodeSearchConfig {
    fn default() -> Self {
        Self {
            binary_path: "codesearch".to_string(),
            workspace: "default".to_string(),
            timeout_secs: 30,
            debug: false,
            mcp_args: vec!["mcp".to_string(), "serve".to_string()],
            use_mock: false,
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
            eprintln!(
                "Code Search request: {}",
                serde_json::to_string_pretty(&request)?
            );
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

    async fn execute_mcp_call(
        &self,
        request: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        if self.config.use_mock {
            return self.execute_mcp_call_mock(&request);
        }

        match self.execute_mcp_call_stdio(&request).await {
            Ok(response) => Ok(response),
            Err(e) => {
                let err_msg = e.to_string();
                let is_binary_missing = err_msg.contains("No such file or directory")
                    || err_msg.contains("not found")
                    || err_msg.contains("os error 2");

                if is_binary_missing && self.config.use_mock {
                    self.execute_mcp_call_mock(&request)
                } else {
                    Err(e)
                }
            }
        }
    }

    async fn execute_mcp_call_stdio(
        &self,
        request: &Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let mut child = Command::new(&self.config.binary_path)
            .args(&self.config.mcp_args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                format!(
                    "Failed to spawn Code Search binary '{}': {}",
                    self.config.binary_path, e
                )
                .into()
            })?;

        let mut stdin = child
            .stdin
            .take()
            .ok_or("Failed to open stdin for Code Search process")?;
        let request_bytes = serde_json::to_vec(request)?;
        stdin.write_all(&request_bytes).await?;
        stdin.write_all(b"\n").await?;
        drop(stdin);

        let output = child.wait_with_output().await.map_err(
            |e| -> Box<dyn std::error::Error + Send + Sync> {
                format!("Code Search process failed: {}", e).into()
            },
        )?;

        if !output.status.success() {
            return Err(
                format!("Code Search process exited with status: {}", output.status).into(),
            );
        }

        let response: Value = serde_json::from_slice(&output.stdout).map_err(
            |e| -> Box<dyn std::error::Error + Send + Sync> {
                format!("Failed to parse Code Search response: {}", e).into()
            },
        )?;

        if let Some(err) = response.get("error") {
            return Err(format!("Code Search MCP error: {}", err).into());
        }

        if let Some(text) = response
            .pointer("/result/content/0/text")
            .and_then(|v| v.as_str())
        {
            let parsed: Value =
                serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.to_string()));
            return Ok(parsed);
        }

        if let Some(result) = response.get("result") {
            return Ok(result.clone());
        }

        Err("Code Search returned empty response".into())
    }

    fn execute_mcp_call_mock(
        &self,
        request: &Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let tool_name = request["params"]["name"]
            .as_str()
            .ok_or("Missing tool name")?;

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
            use_mock: true,
            ..Default::default()
        });

        assert!(client.is_available());

        for _ in 0..5 {
            client.record_failure();
        }

        assert!(!client.is_available());
    }

    #[tokio::test]
    async fn test_mock_call_tool() {
        let client = CodeSearchClient::new(CodeSearchConfig {
            use_mock: true,
            ..Default::default()
        });

        let result = client
            .call_tool("codesearch_search", serde_json::json!({"query": "test"}))
            .await;
        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val["success"], true);
    }

    #[tokio::test]
    async fn test_binary_not_found_returns_error() {
        let client = CodeSearchClient::new(CodeSearchConfig {
            binary_path: "nonexistent_binary_that_does_not_exist".to_string(),
            use_mock: false,
            ..Default::default()
        });

        let result = client
            .call_tool("codesearch_search", serde_json::json!({"query": "test"}))
            .await;
        assert!(result.is_err());
    }
}
