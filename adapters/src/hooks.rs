use async_trait::async_trait;
use mk_core::traits::ContextHooks;

pub struct MemoryContextHooks {}

impl MemoryContextHooks {
    pub fn new() -> Self {
        Self {}
    }
}

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
