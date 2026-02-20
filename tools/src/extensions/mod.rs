mod context;
mod limits;
mod prompt;
mod registry;
mod state;

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use mk_core::types::TenantContext;

pub use context::{ExtensionContext, ExtensionContextState, ExtensionMessage};
pub use limits::{ExtensionStateConfig, ExtensionStateLimiter, ExtensionStateMetrics, LruEntry};
pub use prompt::{PromptAddition, PromptWiring, ToolConfig, ToolSequenceHint};
pub use registry::{ExtensionRegistration, ExtensionRegistry};
pub use state::{ExtensionStateError, ExtensionStateStore};

#[derive(Debug, thiserror::Error)]
pub enum ExtensionError {
    #[error("Extension already registered")]
    AlreadyRegistered,

    #[error("Extension not found")]
    NotFound,

    #[error("Callback failed: {0}")]
    Callback(String),

    #[error("Callback timed out")]
    Timeout,

    #[error("State too large")]
    StateTooLarge,

    #[error("Invalid registration: {0}")]
    InvalidRegistration(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("State store error: {0}")]
    StateStore(String),
}

#[async_trait]
pub trait ExtensionCallback: Send + Sync {
    async fn on_input_messages(
        &self,
        _ctx: &mut ExtensionContext,
        messages: Vec<ExtensionMessage>,
    ) -> Result<Vec<ExtensionMessage>, ExtensionError> {
        Ok(messages)
    }

    async fn on_plain_text(
        &self,
        _ctx: &mut ExtensionContext,
        text: String,
    ) -> Result<String, ExtensionError> {
        Ok(text)
    }

    async fn on_tag(
        &self,
        _ctx: &mut ExtensionContext,
        _tag: String,
        content: String,
    ) -> Result<String, ExtensionError> {
        Ok(content)
    }

    async fn on_llm_output(
        &self,
        _ctx: &mut ExtensionContext,
        output: String,
    ) -> Result<String, ExtensionError> {
        Ok(output)
    }
}

pub struct ExtensionExecutor {
    registry: Arc<ExtensionRegistry>,
    state_store: Option<Arc<ExtensionStateStore>>,
    callback_timeout: Duration,
    max_state_bytes: usize,
}

impl std::fmt::Debug for ExtensionExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExtensionExecutor")
            .field("callback_timeout", &self.callback_timeout)
            .field("max_state_bytes", &self.max_state_bytes)
            .finish()
    }
}

impl ExtensionExecutor {
    pub fn new(registry: Arc<ExtensionRegistry>) -> Self {
        Self {
            registry,
            state_store: None,
            callback_timeout: Duration::from_secs(5),
            max_state_bytes: 64 * 1024,
        }
    }

    pub fn with_state_store(mut self, store: Arc<ExtensionStateStore>) -> Self {
        self.state_store = Some(store);
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.callback_timeout = timeout;
        self
    }

    pub fn with_state_limit(mut self, max_bytes: usize) -> Self {
        self.max_state_bytes = max_bytes;
        self
    }

    pub fn prompt_wiring(&self) -> crate::extensions::PromptWiring {
        self.registry.prompt_wiring()
    }

    pub async fn on_input_messages(
        &self,
        ctx: TenantContext,
        session_id: &str,
        tool_registry: Arc<crate::tools::ToolRegistry>,
        messages: Vec<ExtensionMessage>,
    ) -> Result<Vec<ExtensionMessage>, ExtensionError> {
        let mut current = messages;
        for extension in self.registry.list_ordered() {
            if !extension.enabled {
                continue;
            }
            let mut ext_ctx = ExtensionContext::new(
                ctx.clone(),
                session_id.to_string(),
                extension.id.clone(),
                tool_registry.clone(),
                self.max_state_bytes,
            );
            self.load_state(&mut ext_ctx).await?;
            let result = run_with_timeout(
                self.callback_timeout,
                extension.callbacks.on_input_messages(&mut ext_ctx, current),
            )
            .await?;
            self.save_state(&ext_ctx).await?;
            current = result;
        }
        Ok(current)
    }

    pub async fn on_plain_text(
        &self,
        ctx: TenantContext,
        session_id: &str,
        tool_registry: Arc<crate::tools::ToolRegistry>,
        text: String,
    ) -> Result<String, ExtensionError> {
        let mut current = text;
        for extension in self.registry.list_ordered() {
            if !extension.enabled {
                continue;
            }
            let mut ext_ctx = ExtensionContext::new(
                ctx.clone(),
                session_id.to_string(),
                extension.id.clone(),
                tool_registry.clone(),
                self.max_state_bytes,
            );
            self.load_state(&mut ext_ctx).await?;
            let result = run_with_timeout(
                self.callback_timeout,
                extension.callbacks.on_plain_text(&mut ext_ctx, current),
            )
            .await?;
            self.save_state(&ext_ctx).await?;
            current = result;
        }
        Ok(current)
    }

    pub async fn on_tag(
        &self,
        ctx: TenantContext,
        session_id: &str,
        tool_registry: Arc<crate::tools::ToolRegistry>,
        tag: String,
        content: String,
    ) -> Result<String, ExtensionError> {
        let mut current = content;
        for extension in self.registry.list_ordered() {
            if !extension.enabled {
                continue;
            }
            let mut ext_ctx = ExtensionContext::new(
                ctx.clone(),
                session_id.to_string(),
                extension.id.clone(),
                tool_registry.clone(),
                self.max_state_bytes,
            );
            self.load_state(&mut ext_ctx).await?;
            let result = run_with_timeout(
                self.callback_timeout,
                extension
                    .callbacks
                    .on_tag(&mut ext_ctx, tag.clone(), current),
            )
            .await?;
            self.save_state(&ext_ctx).await?;
            current = result;
        }
        Ok(current)
    }

    pub async fn on_llm_output(
        &self,
        ctx: TenantContext,
        session_id: &str,
        tool_registry: Arc<crate::tools::ToolRegistry>,
        output: String,
    ) -> Result<String, ExtensionError> {
        let mut current = output;
        for extension in self.registry.list_ordered() {
            if !extension.enabled {
                continue;
            }
            let mut ext_ctx = ExtensionContext::new(
                ctx.clone(),
                session_id.to_string(),
                extension.id.clone(),
                tool_registry.clone(),
                self.max_state_bytes,
            );
            self.load_state(&mut ext_ctx).await?;
            let result = run_with_timeout(
                self.callback_timeout,
                extension.callbacks.on_llm_output(&mut ext_ctx, current),
            )
            .await?;
            self.save_state(&ext_ctx).await?;
            current = result;
        }
        Ok(current)
    }

    async fn load_state(&self, ctx: &mut ExtensionContext) -> Result<(), ExtensionError> {
        if let Some(store) = &self.state_store {
            store
                .load(ctx)
                .await
                .map_err(|err| ExtensionError::StateStore(err.to_string()))?;
        }
        Ok(())
    }

    async fn save_state(&self, ctx: &ExtensionContext) -> Result<(), ExtensionError> {
        if let Some(store) = &self.state_store {
            store
                .save(ctx)
                .await
                .map_err(|err| ExtensionError::StateStore(err.to_string()))?;
        }
        Ok(())
    }
}

async fn run_with_timeout<F, T>(timeout_duration: Duration, fut: F) -> Result<T, ExtensionError>
where
    F: std::future::Future<Output = Result<T, ExtensionError>>,
{
    match tokio::time::timeout(timeout_duration, fut).await {
        Ok(result) => result,
        Err(_) => Err(ExtensionError::Timeout),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct TestCallback {
        label: String,
        calls: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait]
    impl ExtensionCallback for TestCallback {
        async fn on_plain_text(
            &self,
            _ctx: &mut ExtensionContext,
            text: String,
        ) -> Result<String, ExtensionError> {
            self.calls.lock().unwrap().push(self.label.clone());
            Ok(format!("{text}-{}", self.label))
        }
    }

    #[tokio::test]
    async fn test_extension_executor_order() {
        let mut registry = ExtensionRegistry::new();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let reg1 = ExtensionRegistration::new(
            "a",
            Arc::new(TestCallback {
                label: "a".to_string(),
                calls: calls.clone(),
            }),
        )
        .with_priority(1);
        let reg2 = ExtensionRegistration::new(
            "b",
            Arc::new(TestCallback {
                label: "b".to_string(),
                calls: calls.clone(),
            }),
        )
        .with_priority(2);

        registry.register_extension(reg1).unwrap();
        registry.register_extension(reg2).unwrap();

        let executor = ExtensionExecutor::new(Arc::new(registry));
        let tool_registry = Arc::new(crate::tools::ToolRegistry::new());
        let result = executor
            .on_plain_text(
                TenantContext::default(),
                "session",
                tool_registry,
                "start".to_string(),
            )
            .await
            .unwrap();

        assert_eq!(result, "start-b-a");
        assert_eq!(calls.lock().unwrap().as_slice(), &["b", "a"]);
    }
}
