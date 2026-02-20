use std::collections::HashMap;
use std::sync::Arc;

use mk_core::types::TenantContext;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::ExtensionError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionContextState {
    pub state: HashMap<String, Value>,
    pub version: u32,
}

pub struct ExtensionContext {
    pub tenant_ctx: TenantContext,
    pub session_id: String,
    pub extension_id: String,
    pub tool_registry: Arc<crate::tools::ToolRegistry>,
    state: HashMap<String, Value>,
    max_state_bytes: usize,
}

impl ExtensionContext {
    pub fn new(
        tenant_ctx: TenantContext,
        session_id: String,
        extension_id: String,
        tool_registry: Arc<crate::tools::ToolRegistry>,
        max_state_bytes: usize,
    ) -> Self {
        Self {
            tenant_ctx,
            session_id,
            extension_id,
            tool_registry,
            state: HashMap::new(),
            max_state_bytes,
        }
    }

    pub fn get_state<T: serde::de::DeserializeOwned>(
        &self,
        key: &str,
    ) -> Result<Option<T>, ExtensionError> {
        match self.state.get(key) {
            Some(value) => serde_json::from_value(value.clone())
                .map(Some)
                .map_err(|err| ExtensionError::Serialization(err.to_string())),
            None => Ok(None),
        }
    }

    pub fn set_state<T: Serialize>(&mut self, key: &str, value: T) -> Result<(), ExtensionError> {
        let next_value = serde_json::to_value(value)
            .map_err(|err| ExtensionError::Serialization(err.to_string()))?;
        self.state.insert(key.to_string(), next_value);
        self.enforce_size_limit()?;
        Ok(())
    }

    pub fn clear_state(&mut self) {
        self.state.clear();
    }

    pub fn state(&self) -> &HashMap<String, Value> {
        &self.state
    }

    pub fn replace_state(&mut self, state: HashMap<String, Value>) {
        self.state = state;
    }

    pub fn to_state_payload(&self) -> Result<ExtensionContextState, ExtensionError> {
        Ok(ExtensionContextState {
            state: self.state.clone(),
            version: 1,
        })
    }

    fn enforce_size_limit(&self) -> Result<(), ExtensionError> {
        let bytes = serde_json::to_vec(&self.state)
            .map_err(|err| ExtensionError::Serialization(err.to_string()))?;
        if bytes.len() > self.max_state_bytes {
            return Err(ExtensionError::StateTooLarge);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_set_get() {
        let ctx = TenantContext::default();
        let registry = Arc::new(crate::tools::ToolRegistry::new());
        let mut ext_ctx =
            ExtensionContext::new(ctx, "s".to_string(), "e".to_string(), registry, 1024);
        ext_ctx.set_state("key", "value").unwrap();
        let value: Option<String> = ext_ctx.get_state("key").unwrap();
        assert_eq!(value.unwrap(), "value");
    }

    #[test]
    fn test_state_size_limit() {
        let ctx = TenantContext::default();
        let registry = Arc::new(crate::tools::ToolRegistry::new());
        let mut ext_ctx =
            ExtensionContext::new(ctx, "s".to_string(), "e".to_string(), registry, 10);
        let result = ext_ctx.set_state("key", "this is too long");
        assert!(matches!(result, Err(ExtensionError::StateTooLarge)));
    }
}
