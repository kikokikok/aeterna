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
    async fn on_session_start(&self, _session_id: &str) -> anyhow::Result<()> {
        Ok(())
    }

    async fn on_session_end(&self, _session_id: &str) -> anyhow::Result<()> {
        Ok(())
    }

    async fn on_message(&self, _session_id: &str, _message: &str) -> anyhow::Result<()> {
        Ok(())
    }

    async fn on_tool_use(
        &self,
        _session_id: &str,
        _tool_name: &str,
        _params: serde_json::Value
    ) -> anyhow::Result<()> {
        Ok(())
    }
}
