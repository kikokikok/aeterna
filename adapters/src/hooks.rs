use async_trait::async_trait;
use mk_core::traits::ContextHooks;
use serde::{Deserialize, Serialize};

pub struct MemoryContextHooks {}

impl MemoryContextHooks {
    pub fn new() -> Self {
        Self {}
    }
}

/// CCA-specific event types for hook integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CcaHookEvent {
    /// Context has been assembled by CCA Context Architect
    #[serde(rename = "chat.context_assembled")]
    ContextAssembled {
        session_id: String,
        token_budget: u32,
        layers_included: Vec<String>,
        entry_count: usize,
    },

    /// Tool trajectory has been captured for note distillation
    #[serde(rename = "tool.trajectory_captured")]
    TrajectoryCaptured {
        session_id: String,
        tool_name: String,
        success: bool,
        event_count: usize,
    },

    /// Error has been captured for hindsight learning
    #[serde(rename = "error.captured")]
    ErrorCaptured {
        session_id: String,
        error_type: String,
        message_pattern: String,
        context_patterns: Vec<String>,
    },
}

/// Mapping of CCA hooks to ContextHooks trait methods
///
/// ## Hook Integration for CCA
///
/// The following CCA capabilities map to ContextHooks trait methods:
///
/// | CCA Hook              | ContextHooks Method | Trigger                     |
/// |-----------------------|-------------------|------------------------------|
/// | chat.context_assembled | on_message        | After context assembled    |
/// | tool.trajectory_captured | on_tool_use      | After tool execution    |
/// | session.ended         | on_session_end     | On session close       |
/// | error.captured         | on_tool_use (error) | On tool failure       |
///
/// ## Implementation Notes
///
/// OpenCode plugin implementers should:
///
/// 1. **Context Injection**: Call `context_assemble` tool before sending
///    messages to LLM
///    - Parse the response for `layersIncluded` and `entryCount`
///    - Inject assembled content into system prompt or message
///
/// 2. **Trajectory Capture**: Emit `tool.trajectory_captured` after each tool
///    call
///    - Include `toolName`, `success`, and duration
///    - Use `note_capture` tool to manually trigger distillation
///
/// 3. **Error Handling**: Emit `error.captured` events on tool failures
///    - Include error signature details
///    - Use `hindsight_query` tool to find resolution patterns
///
/// 4. **Note Distillation**: Trigger on `session.ended` hook
///    - Check trajectory event count
///    - Auto-distill if threshold reached
///    - Or use `note_capture` tool manually
pub struct CcaHooks {}

#[async_trait]
impl ContextHooks for MemoryContextHooks {
    async fn on_session_start(
        &self,
        _ctx: mk_core::types::TenantContext,
        _session_id: &str,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn on_session_end(
        &self,
        _ctx: mk_core::types::TenantContext,
        _session_id: &str,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn on_message(
        &self,
        _ctx: mk_core::types::TenantContext,
        _session_id: &str,
        _message: &str,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn on_tool_use(
        &self,
        _ctx: mk_core::types::TenantContext,
        _session_id: &str,
        _tool_name: &str,
        _params: serde_json::Value,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mk_core::types::TenantContext;
    use serde_json::json;

    #[test]
    fn test_memory_context_hooks_new() {
        let hooks = MemoryContextHooks::new();
        let _ = hooks;
    }

    #[tokio::test]
    async fn test_memory_context_hooks_methods() {
        let hooks = MemoryContextHooks::new();
        let ctx = TenantContext::default();

        assert!(
            hooks
                .on_session_start(ctx.clone(), "test-session")
                .await
                .is_ok()
        );
        assert!(
            hooks
                .on_session_end(ctx.clone(), "test-session")
                .await
                .is_ok()
        );
        assert!(
            hooks
                .on_message(ctx.clone(), "test-session", "test message")
                .await
                .is_ok()
        );
        assert!(
            hooks
                .on_tool_use(ctx, "test-session", "test_tool", json!({}))
                .await
                .is_ok()
        );
    }

    #[test]
    fn test_context_hooks_trait_implementation() {
        use mk_core::traits::ContextHooks;

        fn assert_implements_context_hooks<T: ContextHooks>() {}

        assert_implements_context_hooks::<MemoryContextHooks>();
    }

    #[test]
    fn test_hooks_send_sync_bounds() {
        fn assert_send_sync<T: Send + Sync>() {}

        assert_send_sync::<MemoryContextHooks>();
    }
}
