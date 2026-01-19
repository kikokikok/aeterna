# CCA Extension Development Guide

This guide explains how to create custom extensions for CCA (Confucius Code Agent) capabilities. Extensions enable you to customize agent behavior, add prompt enhancements, configure tools, and manage stateful interactions.

## Overview

The CCA extension system provides typed callbacks with state management, allowing you to:

- Transform input messages before processing
- Enrich context with custom data
- Process tagged content (e.g., `@file`, `@url`)
- Post-process LLM outputs
- Maintain stateful context across agent interactions

Extensions run **client-side** in the OpenCode Plugin, providing low-latency processing while accessing server-side tools via MCP.

## Extension Architecture

### Execution Flow

```
User Input
    │
    ▼
Extension Chain (Priority DESC)
    │
    ├─ Extension A (priority: 10) → on_input_messages()
    │
    ├─ Extension B (priority: 5)  → on_input_messages()
    │
    └─ Extension C (priority: 1)  → on_input_messages()
    │
    ▼
Agent Processing (with modified messages)
    │
    ▼
LLM Output
    │
    ▼
Extension Chain (Priority DESC)
    │
    ├─ Extension A → on_llm_output()
    │
    ├─ Extension B → on_llm_output()
    │
    └─ Extension C → on_llm_output()
    │
    ▼
User sees final output
```

Each extension in the chain can:
- Read/write state via `ExtensionContext`
- Call Aeterna MCP tools via `ctx.tool_registry`
- Transform data and pass to next extension

## Core Components

### 1. ExtensionCallback Trait

The `ExtensionCallback` trait defines four hook points:

```rust
use async_trait::async_trait;
use tools::extensions::{ExtensionCallback, ExtensionContext, ExtensionError, ExtensionMessage};

#[async_trait]
pub trait ExtensionCallback: Send + Sync {
    /// Called when input messages are received
    async fn on_input_messages(
        &self,
        ctx: &mut ExtensionContext,
        messages: Vec<ExtensionMessage>
    ) -> Result<Vec<ExtensionMessage>, ExtensionError> {
        Ok(messages)  // Default: pass through unchanged
    }

    /// Called for plain text processing
    async fn on_plain_text(
        &self,
        ctx: &mut ExtensionContext,
        text: String
    ) -> Result<String, ExtensionError> {
        Ok(text)  // Default: pass through unchanged
    }

    /// Called when a tagged element is encountered (e.g., @file, @url)
    async fn on_tag(
        &self,
        ctx: &mut ExtensionContext,
        tag: String,
        content: String
    ) -> Result<String, ExtensionError> {
        Ok(content)  // Default: pass through unchanged
    }

    /// Called after LLM generates output
    async fn on_llm_output(
        &self,
        ctx: &mut ExtensionContext,
        output: String
    ) -> Result<String, ExtensionError> {
        Ok(output)  // Default: pass through unchanged
    }
}
```

### 2. ExtensionContext API

The `ExtensionContext` provides access to:

```rust
pub struct ExtensionContext {
    pub tenant_ctx: TenantContext,      // Multi-tenant context
    pub session_id: String,             // Current session ID
    pub extension_id: String,           // This extension's ID
    pub tool_registry: Arc<ToolRegistry>, // Access to MCP tools
    // Private fields...
}

impl ExtensionContext {
    /// Get typed state value
    pub fn get_state<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, ExtensionError>;

    /// Set typed state value
    pub fn set_state<T: Serialize>(&mut self, key: &str, value: T) -> Result<(), ExtensionError>;

    /// Clear all state for this extension
    pub fn clear_state(&mut self);

    /// Get raw state map (advanced usage)
    pub fn state(&self) -> &HashMap<String, Value>;

    /// Replace entire state (advanced usage)
    pub fn replace_state(&mut self, state: HashMap<String, Value>);
}
```

### 3. ExtensionRegistration

Register an extension with configuration:

```rust
pub struct ExtensionRegistration {
    pub id: String,                          // Unique identifier
    pub callbacks: Arc<dyn ExtensionCallback>, // Callback implementation
    pub prompt_additions: Vec<PromptAddition>, // Prompt enhancements
    pub tool_config: ToolConfig,             // Tool behavior overrides
    pub sequence_hints: Vec<ToolSequenceHint>, // Tool sequencing hints
    pub priority: i32,                       // Execution priority (higher = earlier)
    pub enabled: bool,                       // Enable/disable flag
    pub state_config: ExtensionStateConfig,  // State management config
}

impl ExtensionRegistration {
    pub fn new(id: impl Into<String>, callbacks: Arc<dyn ExtensionCallback>) -> Self;
    pub fn with_priority(mut self, priority: i32) -> Self;
    pub fn with_prompt_additions(mut self, additions: Vec<PromptAddition>) -> Self;
    pub fn with_tool_config(mut self, config: ToolConfig) -> Self;
    pub fn with_sequence_hints(mut self, hints: Vec<ToolSequenceHint>) -> Self;
    pub fn enabled(mut self, enabled: bool) -> Self;
    pub fn with_state_config(mut self, config: ExtensionStateConfig) -> Self;
}
```

### 4. ExtensionRegistry

Manage multiple extensions:

```rust
pub struct ExtensionRegistry {
    // Private fields...
}

impl ExtensionRegistry {
    pub fn new() -> Self;

    /// Register a new extension
    pub fn register_extension(&mut self, registration: ExtensionRegistration) -> Result<(), ExtensionError>;

    /// Enable or disable an extension
    pub fn enable_extension(&mut self, id: &str, enabled: bool) -> Result<(), ExtensionError>;

    /// Get extensions in priority order (DESC)
    pub fn list_ordered(&self) -> Vec<ExtensionRegistration>;

    /// Generate prompt wiring for all enabled extensions
    pub fn prompt_wiring(&self) -> PromptWiring;
}
```

### 5. PromptWiring

Combine prompt additions and tool configurations from all extensions:

```rust
pub struct PromptWiring {
    pub additions: Vec<PromptAddition>,      // Prompt text additions
    pub tool_config: ToolConfig,             // Combined tool config
    pub sequencing_hints: Vec<ToolSequenceHint>, // Tool sequencing
}

pub struct PromptAddition {
    pub role: String,      // "system", "user", "assistant"
    pub content: String,   // Prompt text to add
}

pub struct ToolConfig {
    pub suggested_tools: Vec<String>,        // Tools to suggest
    pub disabled_tools: Vec<String>,         // Tools to disable
    pub overrides: HashMap<String, String>,  // Tool parameter overrides
}

pub struct ToolSequenceHint {
    pub when_tool: String,    // When this tool is used...
    pub suggest_next: String, // ...suggest this tool next
}
```

## State Management

### State Limits

To prevent memory exhaustion, state is limited:

```rust
pub struct ExtensionStateConfig {
    pub max_state_size_bytes: usize,  // Default: 1MB
    pub state_ttl_seconds: u64,       // Default: 3600 (1 hour)
}
```

- **Size Limit**: Default 1MB per extension (configurable)
- **TTL**: Default 1 hour (configurable)
- **Eviction**: LRU (Least Recently Used) when storage is full
- **Compression**: zstd compression for storage efficiency

### State Operations

```rust
// Set state
ctx.set_state("key", value)?;  // Enforces size limit

// Get state
let value: Option<MyType> = ctx.get_state("key")?;

// Clear state
ctx.clear_state();

// Example: Track conversation count
let count: u32 = ctx.get_state("message_count")?.unwrap_or(0);
ctx.set_state("message_count", count + 1)?;
```

### State Persistence

State is persisted to Redis with compression:

1. After each callback, state is serialized to JSON
2. Compressed with zstd
3. Stored in Redis with key: `ext:state:{session_id}:{extension_id}`
4. TTL applied based on `state_ttl_seconds`
5. Before next callback, state is loaded and decompressed

## Complete Extension Example

### Scenario: Context-Aware Code Reviewer

This extension tracks code patterns across a session and suggests improvements based on accumulated knowledge.

```rust
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tools::extensions::{
    ExtensionCallback, ExtensionContext, ExtensionError, ExtensionMessage,
    ExtensionRegistration, ExtensionRegistry, ExtensionStateConfig,
    PromptAddition, ToolConfig
};

/// State stored across callback invocations
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ReviewerState {
    patterns_seen: HashMap<String, u32>,  // Pattern name → count
    last_review_time: Option<u64>,
    suggestions_given: Vec<String>,
}

impl Default for ReviewerState {
    fn default() -> Self {
        Self {
            patterns_seen: HashMap::new(),
            last_review_time: None,
            suggestions_given: Vec::new(),
        }
    }
}

/// Custom extension implementation
struct CodeReviewerExtension {
    min_pattern_count: u32,
}

impl CodeReviewerExtension {
    fn new(min_pattern_count: u32) -> Self {
        Self { min_pattern_count }
    }

    /// Detect code patterns in text
    fn detect_patterns(&self, text: &str) -> Vec<String> {
        let mut patterns = Vec::new();
        
        if text.contains("unwrap()") {
            patterns.push("unwrap-usage".to_string());
        }
        if text.contains("clone()") {
            patterns.push("clone-usage".to_string());
        }
        if text.contains("Arc<Mutex<") {
            patterns.push("arc-mutex-pattern".to_string());
        }
        if text.contains("panic!") {
            patterns.push("panic-usage".to_string());
        }
        
        patterns
    }

    /// Generate suggestions based on accumulated state
    fn generate_suggestions(&self, state: &ReviewerState) -> Vec<String> {
        let mut suggestions = Vec::new();

        for (pattern, count) in &state.patterns_seen {
            if *count >= self.min_pattern_count {
                match pattern.as_str() {
                    "unwrap-usage" => {
                        suggestions.push(format!(
                            "You've used unwrap() {} times. Consider using ? operator or match for better error handling.",
                            count
                        ));
                    }
                    "clone-usage" => {
                        suggestions.push(format!(
                            "You've used clone() {} times. Consider borrowing or using Arc<T> to reduce allocations.",
                            count
                        ));
                    }
                    "arc-mutex-pattern" => {
                        suggestions.push(format!(
                            "You've used Arc<Mutex<T>> {} times. Consider using message passing (channels) for cleaner concurrency.",
                            count
                        ));
                    }
                    "panic-usage" => {
                        suggestions.push(format!(
                            "You've used panic! {} times. Consider returning Result<T, E> for recoverable errors.",
                            count
                        ));
                    }
                    _ => {}
                }
            }
        }

        suggestions
    }
}

#[async_trait]
impl ExtensionCallback for CodeReviewerExtension {
    /// Process incoming messages to detect patterns
    async fn on_input_messages(
        &self,
        ctx: &mut ExtensionContext,
        messages: Vec<ExtensionMessage>
    ) -> Result<Vec<ExtensionMessage>, ExtensionError> {
        // Load state
        let mut state: ReviewerState = ctx
            .get_state("reviewer_state")?
            .unwrap_or_default();

        // Analyze messages for patterns
        for msg in &messages {
            if msg.role == "user" {
                let patterns = self.detect_patterns(&msg.content);
                for pattern in patterns {
                    *state.patterns_seen.entry(pattern).or_insert(0) += 1;
                }
            }
        }

        // Save state
        ctx.set_state("reviewer_state", state)?;

        // Pass messages through unchanged
        Ok(messages)
    }

    /// Add code review suggestions to LLM output
    async fn on_llm_output(
        &self,
        ctx: &mut ExtensionContext,
        output: String
    ) -> Result<String, ExtensionError> {
        // Load state
        let state: ReviewerState = ctx
            .get_state("reviewer_state")?
            .unwrap_or_default();

        // Generate suggestions
        let suggestions = self.generate_suggestions(&state);

        if suggestions.is_empty() {
            return Ok(output);
        }

        // Append suggestions to output
        let mut enhanced = output;
        enhanced.push_str("\n\n## Code Review Suggestions\n\n");
        for (i, suggestion) in suggestions.iter().enumerate() {
            enhanced.push_str(&format!("{}. {}\n", i + 1, suggestion));
        }

        Ok(enhanced)
    }
}

/// Register the extension
pub fn register_code_reviewer(registry: &mut ExtensionRegistry) -> Result<(), ExtensionError> {
    let extension = Arc::new(CodeReviewerExtension::new(3));  // Suggest after 3 occurrences

    let registration = ExtensionRegistration::new("code-reviewer", extension)
        .with_priority(5)  // Medium priority
        .with_prompt_additions(vec![
            PromptAddition {
                role: "system".to_string(),
                content: "You are a code reviewer. Track patterns and provide constructive feedback.".to_string(),
            }
        ])
        .with_state_config(ExtensionStateConfig {
            max_state_size_bytes: 512 * 1024,  // 512KB
            state_ttl_seconds: 7200,  // 2 hours
        });

    registry.register_extension(registration)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mk_core::types::TenantContext;
    use tools::tools::ToolRegistry;

    #[tokio::test]
    async fn test_code_reviewer_detects_patterns() {
        let extension = CodeReviewerExtension::new(2);
        let tool_registry = Arc::new(ToolRegistry::new());
        let mut ctx = ExtensionContext::new(
            TenantContext::default(),
            "test-session".to_string(),
            "code-reviewer".to_string(),
            tool_registry,
            64 * 1024,
        );

        let messages = vec![
            ExtensionMessage {
                role: "user".to_string(),
                content: "let value = map.get(\"key\").unwrap();".to_string(),
            },
            ExtensionMessage {
                role: "user".to_string(),
                content: "let value = map.get(\"key2\").unwrap();".to_string(),
            },
        ];

        let result = extension.on_input_messages(&mut ctx, messages).await.unwrap();
        assert_eq!(result.len(), 2);

        // Check state
        let state: ReviewerState = ctx.get_state("reviewer_state").unwrap().unwrap();
        assert_eq!(state.patterns_seen.get("unwrap-usage"), Some(&2));
    }

    #[tokio::test]
    async fn test_code_reviewer_generates_suggestions() {
        let extension = CodeReviewerExtension::new(2);
        let tool_registry = Arc::new(ToolRegistry::new());
        let mut ctx = ExtensionContext::new(
            TenantContext::default(),
            "test-session".to_string(),
            "code-reviewer".to_string(),
            tool_registry,
            64 * 1024,
        );

        // Set state with detected patterns
        let mut state = ReviewerState::default();
        state.patterns_seen.insert("unwrap-usage".to_string(), 3);
        ctx.set_state("reviewer_state", state).unwrap();

        let output = "Here's the code you requested.".to_string();
        let enhanced = extension.on_llm_output(&mut ctx, output).await.unwrap();

        assert!(enhanced.contains("Code Review Suggestions"));
        assert!(enhanced.contains("unwrap()"));
    }
}
```

## Extension Patterns

### Pattern 1: Enrichment Extension

Add context from external sources:

```rust
struct EnrichmentExtension {
    api_client: Arc<ExternalApiClient>,
}

#[async_trait]
impl ExtensionCallback for EnrichmentExtension {
    async fn on_input_messages(
        &self,
        ctx: &mut ExtensionContext,
        mut messages: Vec<ExtensionMessage>
    ) -> Result<Vec<ExtensionMessage>, ExtensionError> {
        // Detect entities in user message
        if let Some(last_msg) = messages.last() {
            if last_msg.role == "user" {
                let entities = self.extract_entities(&last_msg.content);
                
                // Fetch enrichment data
                for entity in entities {
                    if let Ok(data) = self.api_client.lookup(&entity).await {
                        // Add enrichment as system message
                        messages.push(ExtensionMessage {
                            role: "system".to_string(),
                            content: format!("Context for {}: {}", entity, data),
                        });
                    }
                }
            }
        }

        Ok(messages)
    }
}
```

### Pattern 2: Validation Extension

Validate outputs before returning to user:

```rust
struct ValidationExtension {
    rules: Vec<ValidationRule>,
}

#[async_trait]
impl ExtensionCallback for ValidationExtension {
    async fn on_llm_output(
        &self,
        _ctx: &mut ExtensionContext,
        output: String
    ) -> Result<String, ExtensionError> {
        for rule in &self.rules {
            if !rule.validate(&output) {
                return Err(ExtensionError::Callback(
                    format!("Output failed validation: {}", rule.name)
                ));
            }
        }
        Ok(output)
    }
}
```

### Pattern 3: Caching Extension

Cache expensive operations:

```rust
struct CachingExtension {
    cache: Arc<RwLock<HashMap<String, String>>>,
}

#[async_trait]
impl ExtensionCallback for CachingExtension {
    async fn on_plain_text(
        &self,
        ctx: &mut ExtensionContext,
        text: String
    ) -> Result<String, ExtensionError> {
        // Check cache
        {
            let cache = self.cache.read().unwrap();
            if let Some(cached) = cache.get(&text) {
                return Ok(cached.clone());
            }
        }

        // Process (expensive operation)
        let processed = self.expensive_processing(&text).await?;

        // Store in cache
        {
            let mut cache = self.cache.write().unwrap();
            cache.insert(text, processed.clone());
        }

        Ok(processed)
    }
}
```

### Pattern 4: Stateful Conversation Tracker

Track conversation flow:

```rust
#[derive(Serialize, Deserialize)]
struct ConversationState {
    turn_count: u32,
    topics: Vec<String>,
    sentiment: f32,
}

struct ConversationTrackerExtension;

#[async_trait]
impl ExtensionCallback for ConversationTrackerExtension {
    async fn on_input_messages(
        &self,
        ctx: &mut ExtensionContext,
        messages: Vec<ExtensionMessage>
    ) -> Result<Vec<ExtensionMessage>, ExtensionError> {
        let mut state: ConversationState = ctx
            .get_state("conversation")?
            .unwrap_or(ConversationState {
                turn_count: 0,
                topics: Vec::new(),
                sentiment: 0.0,
            });

        state.turn_count += 1;
        
        // Analyze topics
        for msg in &messages {
            if msg.role == "user" {
                let topics = self.extract_topics(&msg.content);
                state.topics.extend(topics);
            }
        }

        ctx.set_state("conversation", state)?;
        Ok(messages)
    }
}
```

## Best Practices

### 1. Keep Extensions Focused

Each extension should have a single responsibility:
- ✅ Good: `CodeReviewerExtension` (reviews code)
- ❌ Bad: `EverythingExtension` (reviews code, translates, validates, etc.)

### 2. Handle State Gracefully

Always provide defaults when state is missing:

```rust
let state: MyState = ctx
    .get_state("my_state")?
    .unwrap_or_default();  // ✅ Graceful fallback
```

### 3. Validate Inputs

Don't trust data from previous extensions:

```rust
async fn on_input_messages(
    &self,
    ctx: &mut ExtensionContext,
    messages: Vec<ExtensionMessage>
) -> Result<Vec<ExtensionMessage>, ExtensionError> {
    if messages.is_empty() {
        return Err(ExtensionError::Callback("Empty messages".to_string()));
    }
    // Continue processing...
}
```

### 4. Use Appropriate Priorities

- **High (10+)**: Critical preprocessing (security, validation)
- **Medium (5-9)**: Enrichment, analysis
- **Low (1-4)**: Formatting, cosmetic changes

### 5. Limit State Size

Monitor state size to avoid hitting limits:

```rust
let state_size = serde_json::to_vec(&state)?.len();
if state_size > 900_000 {  // 90% of 1MB limit
    // Trim old data
    state.history.truncate(100);
}
ctx.set_state("state", state)?;
```

### 6. Handle Timeouts

Extensions have a default 5-second timeout. For long operations:

```rust
async fn on_input_messages(
    &self,
    ctx: &mut ExtensionContext,
    messages: Vec<ExtensionMessage>
) -> Result<Vec<ExtensionMessage>, ExtensionError> {
    // Spawn background task for long operation
    tokio::spawn(async move {
        self.long_running_analysis(messages.clone()).await;
    });

    // Return immediately
    Ok(messages)
}
```

### 7. Test Thoroughly

Write unit tests for each callback:

```rust
#[tokio::test]
async fn test_my_extension() {
    let ext = MyExtension::new();
    let mut ctx = create_test_context();
    
    let messages = vec![test_message()];
    let result = ext.on_input_messages(&mut ctx, messages).await;
    
    assert!(result.is_ok());
    // Additional assertions...
}
```

## Debugging Extensions

### Enable Debug Logging

```rust
use tracing::{info, debug, error};

async fn on_input_messages(
    &self,
    ctx: &mut ExtensionContext,
    messages: Vec<ExtensionMessage>
) -> Result<Vec<ExtensionMessage>, ExtensionError> {
    debug!("Extension {} processing {} messages", ctx.extension_id, messages.len());
    
    let state: MyState = ctx.get_state("state")?.unwrap_or_default();
    info!("Current state: {:?}", state);
    
    // Processing...
    
    Ok(messages)
}
```

### Inspect State

Use the `ExtensionContext::state()` method to dump raw state:

```rust
let raw_state = ctx.state();
println!("Raw state: {:#?}", raw_state);
```

### Monitor Performance

Track callback duration:

```rust
use std::time::Instant;

let start = Instant::now();
let result = self.process(ctx, messages).await?;
let duration = start.elapsed();

if duration.as_millis() > 100 {
    warn!("Extension {} took {}ms", ctx.extension_id, duration.as_millis());
}
```

## Deployment

### OpenCode Plugin Integration

Extensions are registered in the OpenCode plugin:

```typescript
// opencode-plugin/src/extensions/index.ts
import { ExtensionRegistry } from '@aeterna/opencode-plugin';

const registry = new ExtensionRegistry();

// Register Rust-compiled WASM extension
registry.registerWasm('code-reviewer', await loadWasm('code-reviewer.wasm'));

// Register JavaScript extension
registry.register({
  id: 'js-formatter',
  priority: 3,
  callbacks: {
    onLlmOutput: async (ctx, output) => {
      return formatMarkdown(output);
    }
  }
});
```

### State Storage Configuration

Configure Redis for state persistence:

```toml
[extensions.state]
redis_url = "redis://localhost:6379"
compression = "zstd"
compression_level = 3
max_state_size_bytes = 1048576  # 1MB
default_ttl_seconds = 3600  # 1 hour
```

## Next Steps

- Review [Architecture](architecture.md) to understand where extensions fit
- Explore [API Reference](api-reference.md) for available MCP tools
- See [Configuration](configuration.md) for extension state settings
- Check out example extensions in `tools/src/extensions/examples/`
