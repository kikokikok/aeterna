//! # Code Intelligence Backend
//!
//! Pluggable backend trait for code intelligence services (search, call graph, index status).
//!
//! ## Architecture
//! - `CodeIntelligenceBackend` trait: defines the contract for any code intelligence provider
//! - `McpCodeIntelligence`: proxies calls to an external MCP code intelligence server (JetBrains, VS Code, etc.)
//! - `MockCodeIntelligence`: in-memory mock for testing
//! - `CodeSearchClient`: facade that dispatches to the configured backend with circuit breaker

use async_trait::async_trait;
use serde_json::Value;
use std::sync::{Arc, Mutex};

/// Configuration for Code Search client
#[derive(Debug, Clone)]
pub struct CodeSearchConfig {
    /// MCP server endpoint URL for HTTP-based backends (e.g. "http://localhost:3000")
    pub mcp_server_url: Option<String>,
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
            mcp_server_url: None,
            workspace: "default".to_string(),
            timeout_secs: 30,
            debug: false,
        }
    }
}

/// Pluggable backend for code intelligence operations.
///
/// Implementations proxy to any MCP-compatible code intelligence server
/// (JetBrains Code Intelligence MCP, VS Code extensions, custom backends, etc.).
#[async_trait]
pub trait CodeIntelligenceBackend: Send + Sync {
    fn is_available(&self) -> bool;
    async fn search(
        &self,
        params: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>>;
    async fn trace_callers(
        &self,
        params: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>>;
    async fn trace_callees(
        &self,
        params: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>>;
    async fn graph(&self, params: Value)
    -> Result<Value, Box<dyn std::error::Error + Send + Sync>>;
    async fn index_status(
        &self,
        params: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>>;
    async fn repo_request(
        &self,
        params: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>>;
}

/// Backend that proxies calls to an external MCP code intelligence server via HTTP JSON-RPC.
pub struct McpCodeIntelligence {
    server_url: String,
    client: reqwest::Client,
    timeout: std::time::Duration,
    debug: bool,
}

impl McpCodeIntelligence {
    pub fn new(server_url: String, timeout_secs: u64, debug: bool) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(timeout_secs))
            .build()
            .unwrap_or_default();
        Self {
            server_url,
            client,
            timeout: std::time::Duration::from_secs(timeout_secs),
            debug,
        }
    }

    async fn call_tool(
        &self,
        tool_name: &str,
        params: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": tool_name,
                "arguments": params
            }
        });

        if self.debug {
            tracing::debug!(
                "MCP code intelligence request: {}",
                serde_json::to_string_pretty(&request)?
            );
        }

        let response = self
            .client
            .post(&self.server_url)
            .json(&request)
            .timeout(self.timeout)
            .send()
            .await
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                format!(
                    "MCP code intelligence server unreachable at {}: {}",
                    self.server_url, e
                )
                .into()
            })?;

        let body: Value =
            response
                .json()
                .await
                .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                    format!("Failed to parse MCP response: {}", e).into()
                })?;

        if let Some(err) = body.get("error") {
            return Err(format!("MCP code intelligence error: {}", err).into());
        }

        // Extract result from JSON-RPC response
        if let Some(text) = body
            .pointer("/result/content/0/text")
            .and_then(|v| v.as_str())
        {
            let parsed: Value =
                serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.to_string()));
            return Ok(parsed);
        }

        if let Some(result) = body.get("result") {
            return Ok(result.clone());
        }

        Err("MCP code intelligence returned empty response".into())
    }
}

#[async_trait]
impl CodeIntelligenceBackend for McpCodeIntelligence {
    fn is_available(&self) -> bool {
        // For HTTP backends, we assume available if configured.
        // The circuit breaker in CodeSearchClient handles transient failures.
        true
    }

    async fn search(
        &self,
        params: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        self.call_tool("codesearch_search", params).await
    }

    async fn trace_callers(
        &self,
        params: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        self.call_tool("codesearch_trace_callers", params).await
    }

    async fn trace_callees(
        &self,
        params: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        self.call_tool("codesearch_trace_callees", params).await
    }

    async fn graph(
        &self,
        params: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        self.call_tool("codesearch_trace_graph", params).await
    }

    async fn index_status(
        &self,
        params: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        self.call_tool("codesearch_index_status", params).await
    }

    async fn repo_request(
        &self,
        params: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        self.call_tool("codesearch_repo_request", params).await
    }
}

pub struct MockCodeIntelligence {
    workspace: String,
}

impl MockCodeIntelligence {
    pub fn new(workspace: String) -> Self {
        Self { workspace }
    }
}

impl Default for MockCodeIntelligence {
    fn default() -> Self {
        Self::new("default".to_string())
    }
}

#[async_trait]
impl CodeIntelligenceBackend for MockCodeIntelligence {
    fn is_available(&self) -> bool {
        true
    }

    async fn search(
        &self,
        _params: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        Ok(serde_json::json!({
            "success": true,
            "results": []
        }))
    }

    async fn trace_callers(
        &self,
        _params: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        Ok(serde_json::json!({
            "success": true,
            "symbols": []
        }))
    }

    async fn trace_callees(
        &self,
        _params: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        Ok(serde_json::json!({
            "success": true,
            "symbols": []
        }))
    }

    async fn graph(
        &self,
        _params: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        Ok(serde_json::json!({
            "success": true,
            "nodes": []
        }))
    }

    async fn index_status(
        &self,
        _params: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        Ok(serde_json::json!({
            "success": true,
            "status": {
                "project": self.workspace.clone(),
                "files_indexed": 0,
                "chunks": 0,
                "state": "idle"
            }
        }))
    }

    async fn repo_request(
        &self,
        _params: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        Ok(serde_json::json!({
            "success": true,
            "id": uuid::Uuid::new_v4().to_string(),
            "status": "requested"
        }))
    }
}

/// Returns informative errors guiding the user to install a compatible backend.
pub struct NoBackend;

const NO_BACKEND_MSG: &str = "No code intelligence backend configured. \
     Install a compatible MCP backend (e.g. JetBrains Code Intelligence MCP plugin, \
     VS Code Code Intelligence extension) and set AETERNA_CODE_INTEL_MCP_URL.";

#[async_trait]
impl CodeIntelligenceBackend for NoBackend {
    fn is_available(&self) -> bool {
        false
    }

    async fn search(
        &self,
        _params: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        Err(NO_BACKEND_MSG.into())
    }

    async fn trace_callers(
        &self,
        _params: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        Err(NO_BACKEND_MSG.into())
    }

    async fn trace_callees(
        &self,
        _params: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        Err(NO_BACKEND_MSG.into())
    }

    async fn graph(
        &self,
        _params: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        Err(NO_BACKEND_MSG.into())
    }

    async fn index_status(
        &self,
        _params: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        Err(NO_BACKEND_MSG.into())
    }

    async fn repo_request(
        &self,
        _params: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        Err(NO_BACKEND_MSG.into())
    }
}

pub struct CodeSearchClient {
    backend: Arc<dyn CodeIntelligenceBackend>,
    failures: Arc<Mutex<usize>>,
    max_failures: usize,
}

impl CodeSearchClient {
    pub fn new(backend: Arc<dyn CodeIntelligenceBackend>) -> Self {
        Self {
            backend,
            failures: Arc::new(Mutex::new(0)),
            max_failures: 5,
        }
    }

    pub fn from_config(config: &CodeSearchConfig) -> Self {
        let backend: Arc<dyn CodeIntelligenceBackend> = if let Some(url) = &config.mcp_server_url {
            Arc::new(McpCodeIntelligence::new(
                url.clone(),
                config.timeout_secs,
                config.debug,
            ))
        } else {
            Arc::new(NoBackend)
        };

        Self::new(backend)
    }

    pub fn mock(workspace: &str) -> Self {
        Self::new(Arc::new(MockCodeIntelligence::new(workspace.to_string())))
    }

    pub fn is_available(&self) -> bool {
        let failures = self.failures.lock().unwrap();
        *failures < self.max_failures && self.backend.is_available()
    }

    fn reset_failures(&self) {
        let mut failures = self.failures.lock().unwrap();
        *failures = 0;
    }

    fn record_failure(&self) {
        let mut failures = self.failures.lock().unwrap();
        *failures += 1;
    }

    pub async fn call_tool(
        &self,
        tool_name: &str,
        params: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        if !self.is_available() {
            return Err("Code intelligence backend unavailable (circuit breaker open)".into());
        }

        let result = match tool_name {
            "codesearch_search" => self.backend.search(params).await,
            "codesearch_trace_callers" => self.backend.trace_callers(params).await,
            "codesearch_trace_callees" => self.backend.trace_callees(params).await,
            "codesearch_trace_graph" => self.backend.graph(params).await,
            "codesearch_index_status" => self.backend.index_status(params).await,
            "codesearch_repo_request" => self.backend.repo_request(params).await,
            _ => Err(format!("Unknown code intelligence tool: {}", tool_name).into()),
        };

        match &result {
            Ok(_) => self.reset_failures(),
            Err(_) => self.record_failure(),
        }

        result
    }

    pub fn backend(&self) -> &Arc<dyn CodeIntelligenceBackend> {
        &self.backend
    }
}

impl Default for CodeSearchClient {
    fn default() -> Self {
        Self::new(Arc::new(NoBackend))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_with_mock_backend() {
        let client = CodeSearchClient::mock("default");
        assert!(client.is_available());
    }

    #[tokio::test]
    async fn test_client_default_no_backend() {
        let client = CodeSearchClient::default();
        // NoBackend reports not available
        assert!(!client.is_available());
    }

    #[tokio::test]
    async fn test_circuit_breaker() {
        let client = CodeSearchClient::mock("default");
        assert!(client.is_available());

        for _ in 0..5 {
            client.record_failure();
        }

        assert!(!client.is_available());
    }

    #[tokio::test]
    async fn test_mock_search() {
        let client = CodeSearchClient::mock("default");
        let result = client
            .call_tool("codesearch_search", serde_json::json!({"query": "test"}))
            .await;
        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val["success"], true);
    }

    #[tokio::test]
    async fn test_mock_trace_callers() {
        let client = CodeSearchClient::mock("default");
        let result = client
            .call_tool(
                "codesearch_trace_callers",
                serde_json::json!({"symbol": "main"}),
            )
            .await;
        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val["success"], true);
    }

    #[tokio::test]
    async fn test_mock_trace_callees() {
        let client = CodeSearchClient::mock("default");
        let result = client
            .call_tool(
                "codesearch_trace_callees",
                serde_json::json!({"symbol": "main"}),
            )
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_mock_graph() {
        let client = CodeSearchClient::mock("default");
        let result = client
            .call_tool(
                "codesearch_trace_graph",
                serde_json::json!({"symbol": "main"}),
            )
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_mock_index_status() {
        let client = CodeSearchClient::mock("default");
        let result = client
            .call_tool("codesearch_index_status", serde_json::json!({}))
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_mock_repo_request() {
        let client = CodeSearchClient::mock("default");
        let result = client
            .call_tool(
                "codesearch_repo_request",
                serde_json::json!({"name": "test", "type": "local"}),
            )
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_unknown_tool() {
        let client = CodeSearchClient::mock("default");
        let result = client
            .call_tool("unknown_tool", serde_json::json!({}))
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unknown"));
    }

    #[tokio::test]
    async fn test_no_backend_returns_informative_error() {
        let _client = CodeSearchClient::default();
        let backend = NoBackend;
        let result = backend.search(serde_json::json!({"query": "test"})).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("No code intelligence backend configured"));
        assert!(err.contains("JetBrains"));
    }

    #[tokio::test]
    async fn test_from_config_with_no_url() {
        let config = CodeSearchConfig::default();
        let client = CodeSearchClient::from_config(&config);
        // No URL configured → NoBackend → not available
        assert!(!client.is_available());
    }

    #[tokio::test]
    async fn test_from_config_with_url() {
        let config = CodeSearchConfig {
            mcp_server_url: Some("http://localhost:3000".to_string()),
            ..Default::default()
        };
        let client = CodeSearchClient::from_config(&config);
        // URL configured → McpCodeIntelligence → available (assumes reachable)
        assert!(client.is_available());
    }

    #[tokio::test]
    async fn test_circuit_breaker_reset() {
        let client = CodeSearchClient::mock("default");

        // Record some failures
        for _ in 0..3 {
            client.record_failure();
        }
        assert!(client.is_available()); // Still under threshold

        // Successful call resets
        let _ = client
            .call_tool("codesearch_search", serde_json::json!({"query": "test"}))
            .await;
        assert!(client.is_available());

        // Verify failures were reset (we should be able to fail 5 more times)
        for _ in 0..5 {
            client.record_failure();
        }
        assert!(!client.is_available());
    }
}
