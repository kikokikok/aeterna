use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// Core trait for implementing MCP (Model Context Protocol) tools.
///
/// Tools provide a standardized interface for AI agents to interact with system
/// capabilities. Each tool defines its name, description, input schema, and
/// execution logic.
///
/// # M-CANONICAL-DOCS
///
/// ## Purpose
/// Defines the contract that all tools must implement to be registered and
/// invoked through the tool registry. Enables pluggable, type-safe tool
/// execution with JSON Schema validation.
///
/// ## Usage
/// ```rust,no_run
/// use async_trait::async_trait;
/// use serde_json::Value;
/// use tools::tools::Tool;
///
/// struct MyCustomTool;
///
/// #[async_trait]
/// impl Tool for MyCustomTool {
///     fn name(&self) -> &str {
///         "my_custom_tool"
///     }
///
///     fn description(&self) -> &str {
///         "Does something useful"
///     }
///
///     fn input_schema(&self) -> Value {
///         serde_json::json!({
///             "type": "object",
///             "properties": {
///                 "input": { "type": "string" }
///             },
///             "required": ["input"]
///         })
///     }
///
///     async fn call(&self, params: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
///         // Process params and return result
///         Ok(serde_json::json!({ "result": "success" }))
///     }
/// }
/// ```
///
/// ## Methods
/// - `name`: Unique identifier for the tool
/// - `description`: Human-readable description of what the tool does
/// - `input_schema`: JSON Schema defining valid input parameters
/// - `call`: Async execution method that processes input and returns output
#[async_trait]
pub trait Tool: Send + Sync + 'static {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> Value;
    async fn call(&self, params: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>>;
}

/// Error codes for tool execution failures.
///
/// # M-CANONICAL-DOCS
///
/// ## Purpose
/// Provides standardized error classification for tool operations, enabling
/// proper error handling and retry logic.
///
/// ## Variants
/// - `InvalidInput`: Input validation failed (non-retryable)
/// - `NotFound`: Requested resource not found (non-retryable)
/// - `ProviderError`: External provider or service failure (retryable)
/// - `RateLimited`: Request rate limit exceeded (retryable)
/// - `Unauthorized`: Authentication/authorization failure (non-retryable)
/// - `Timeout`: Operation timed out (retryable)
/// - `Conflict`: Concurrent modification or state conflict (retryable)
/// - `InternalError`: Unexpected system error (non-retryable)
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ToolErrorCode {
    InvalidInput,
    NotFound,
    ProviderError,
    RateLimited,
    Unauthorized,
    Timeout,
    Conflict,
    InternalError
}

/// Generic response wrapper for tool execution results.
///
/// # M-CANONICAL-DOCS
///
/// ## Purpose
/// Provides a consistent response format for all tool operations, enabling
/// success/failure detection and structured error handling across all tools.
///
/// ## Usage
/// ```rust,no_run
/// use serde_json::json;
/// use tools::tools::{ToolError, ToolErrorCode, ToolResponse};
///
/// // Success response
/// let success = ToolResponse::<String> {
///     success: true,
///     data: Some("result data".to_string()),
///     error: None
/// };
///
/// // Error response
/// let error = ToolResponse::<()> {
///     success: false,
///     data: None,
///     error: Some(ToolError::new(ToolErrorCode::NotFound, "Not found"))
/// };
/// ```
///
/// ## Fields
/// - `success`: Indicates whether the operation succeeded
/// - `data`: Result data on success (omitted on failure)
/// - `error`: Error details on failure (omitted on success)
#[derive(Serialize, Deserialize)]
pub struct ToolResponse<T> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ToolError>
}

/// Detailed error information for tool failures.
///
/// # M-CANONICAL-DOCS
///
/// ## Purpose
/// Encapsulates error details including error code, message, retryability
/// status, and optional context. Enables consumers to make informed decisions
/// about error handling and retries.
///
/// ## Usage
/// ```rust,no_run
/// use tools::tools::{ToolError, ToolErrorCode};
/// use serde_json::json;
///
/// // Basic error
/// let error = ToolError::new(ToolErrorCode::InvalidInput, "Missing required field");
///
/// // Error with details
/// let error = ToolError::new(ToolErrorCode::NotFound, "Resource not found")
///     .with_details(json!({ "resource_id": "123" }));
///
/// // Check if retryable
/// if error.retryable {
///     // Retry logic
/// }
/// ```
///
/// ## Fields
/// - `code`: Standardized error code for classification
/// - `message`: Human-readable error description
/// - `retryable`: Whether the operation can be safely retried
/// - `details`: Additional context for debugging (optional)
#[derive(Serialize, Deserialize)]
pub struct ToolError {
    pub code: ToolErrorCode,
    pub message: String,
    pub retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>
}

impl ToolError {
    pub fn new(code: ToolErrorCode, message: impl Into<String>) -> Self {
        let retryable = matches!(
            code,
            ToolErrorCode::RateLimited | ToolErrorCode::Timeout | ToolErrorCode::ProviderError
        );
        Self {
            code,
            message: message.into(),
            retryable,
            details: None
        }
    }

    pub fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }
}

/// Central registry for managing and invoking tools.
///
/// # M-CANONICAL-DOCS
///
/// ## Purpose
/// Provides a centralized mechanism for registering, discovering, and invoking
/// tools. Enables dynamic tool management and type-safe execution across the
/// system.
///
/// ## Usage
/// ```rust,no_run
/// use tools::tools::{ToolRegistry, Tool, ToolDefinition};
/// use async_trait::async_trait;
/// use serde_json::{json, Value};
/// use std::error::Error;
///
/// struct MyTool;
///
/// #[async_trait]
/// impl Tool for MyTool {
///     fn name(&self) -> &str { "my_tool" }
///     fn description(&self) -> &str { "My example tool" }
///     fn input_schema(&self) -> Value { json!({}) }
///     async fn call(&self, _params: Value) -> Result<Value, Box<dyn Error + Send + Sync>> {
///         Ok(json!({ "result": "success" }))
///     }
/// }
///
/// // Create registry
/// let mut registry = ToolRegistry::new();
///
/// // Register tools
/// registry.register(Box::new(MyTool));
///
/// // List available tools
/// let tools: Vec<ToolDefinition> = registry.list_tools();
/// ```
///
/// ## Methods
/// - `new`: Creates an empty tool registry
/// - `register`: Registers a tool by its unique name
/// - `call`: Invokes a registered tool with the given parameters
/// - `list_tools`: Returns metadata for all registered tools
#[derive(Clone)]
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>
}

impl std::fmt::Debug for ToolRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolRegistry")
            .field("tools", &self.tools.keys().collect::<Vec<_>>())
            .finish()
    }
}

#[allow(clippy::new_without_default)]
impl Default for ToolRegistry {
    fn default() -> Self {
        Self {
            tools: HashMap::new()
        }
    }
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool.into());
    }

    pub fn register_arc(&mut self, tool: Arc<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub async fn call(
        &self,
        name: &str,
        params: Value
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let tool = self
            .tools
            .get(name)
            .ok_or(format!("Tool {} not found", name))?;
        tool.call(params).await
    }

    pub fn list_tools(&self) -> Vec<ToolDefinition> {
        self.tools
            .values()
            .map(|t| ToolDefinition {
                name: t.name().to_string(),
                description: t.description().to_string(),
                input_schema: t.input_schema()
            })
            .collect()
    }
}

/// Metadata definition for a registered tool.
///
/// # M-CANONICAL-DOCS
///
/// ## Purpose
/// Provides tool discovery information without exposing implementation details.
/// Used for tool listing, documentation generation, and client UI.
///
/// ## Usage
/// ```rust,no_run
/// use tools::tools::ToolDefinition;
/// use serde_json::json;
///
/// let definition = ToolDefinition {
///     name: "my_tool".to_string(),
///     description: "Does something useful".to_string(),
///     input_schema: json!({
///         "type": "object",
///         "properties": {
///             "input": { "type": "string" }
///         },
///         "required": ["input"]
///     }),
/// };
/// ```
///
/// ## Fields
/// - `name`: Unique tool identifier used for invocation
/// - `description`: Human-readable description of tool purpose
/// - `input_schema`: JSON Schema defining valid input parameters
#[derive(Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestTool {
        name: String
    }

    #[async_trait]
    impl Tool for TestTool {
        fn name(&self) -> &str {
            &self.name
        }
        fn description(&self) -> &str {
            "Test tool"
        }
        fn input_schema(&self) -> Value {
            serde_json::json!({})
        }
        async fn call(
            &self,
            _params: Value
        ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
            Ok(serde_json::json!({"result": "success"}))
        }
    }

    #[tokio::test]
    async fn test_tool_registry_operations() {
        let mut registry = ToolRegistry::new();

        registry.register(Box::new(TestTool {
            name: "tool1".to_string()
        }));
        registry.register(Box::new(TestTool {
            name: "tool2".to_string()
        }));

        let tools = registry.list_tools();
        assert_eq!(tools.len(), 2);
        assert!(tools.iter().any(|t| t.name == "tool1"));
        assert!(tools.iter().any(|t| t.name == "tool2"));

        let result = registry.call("tool1", serde_json::json!({})).await.unwrap();
        assert_eq!(result["result"], "success");

        let err = registry.call("nonexistent", serde_json::json!({})).await;
        assert!(err.is_err());
        assert!(err.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_tool_registry_duplicate_registration() {
        let mut registry = ToolRegistry::new();

        registry.register(Box::new(TestTool {
            name: "same".to_string()
        }));
        registry.register(Box::new(TestTool {
            name: "same".to_string()
        }));

        let tools = registry.list_tools();
        assert_eq!(tools.len(), 1);
    }

    #[test]
    fn test_tool_error_retryability() {
        let err = ToolError::new(ToolErrorCode::RateLimited, "Too many requests");
        assert!(err.retryable);

        let err = ToolError::new(ToolErrorCode::InvalidInput, "Bad params");
        assert!(!err.retryable);

        let err = ToolError::new(ToolErrorCode::NotFound, "Not found");
        assert!(!err.retryable);

        let err = ToolError::new(ToolErrorCode::Timeout, "Timed out");
        assert!(err.retryable);
    }
}
