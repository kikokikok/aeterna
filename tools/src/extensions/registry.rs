use std::collections::HashMap;
use std::sync::Arc;

use super::{
    ExtensionCallback, ExtensionError, ExtensionStateConfig, PromptAddition, PromptWiring,
    ToolConfig,
};

#[derive(Clone)]
pub struct ExtensionRegistration {
    pub id: String,
    pub callbacks: Arc<dyn ExtensionCallback>,
    pub prompt_additions: Vec<PromptAddition>,
    pub tool_config: ToolConfig,
    pub sequence_hints: Vec<super::prompt::ToolSequenceHint>,
    pub priority: i32,
    pub enabled: bool,
    pub state_config: ExtensionStateConfig,
}

impl ExtensionRegistration {
    pub fn new(id: impl Into<String>, callbacks: Arc<dyn ExtensionCallback>) -> Self {
        Self {
            id: id.into(),
            callbacks,
            prompt_additions: Vec::new(),
            tool_config: ToolConfig::default(),
            sequence_hints: Vec::new(),
            priority: 0,
            enabled: true,
            state_config: ExtensionStateConfig::default(),
        }
    }

    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_prompt_additions(mut self, additions: Vec<PromptAddition>) -> Self {
        self.prompt_additions = additions;
        self
    }

    pub fn with_tool_config(mut self, config: ToolConfig) -> Self {
        self.tool_config = config;
        self
    }

    pub fn with_sequence_hints(mut self, hints: Vec<super::prompt::ToolSequenceHint>) -> Self {
        self.sequence_hints = hints;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn with_state_config(mut self, config: ExtensionStateConfig) -> Self {
        self.state_config = config;
        self
    }
}

pub struct ExtensionRegistry {
    extensions: HashMap<String, ExtensionRegistration>,
}

impl ExtensionRegistry {
    pub fn new() -> Self {
        Self {
            extensions: HashMap::new(),
        }
    }

    pub fn register_extension(
        &mut self,
        registration: ExtensionRegistration,
    ) -> Result<(), ExtensionError> {
        if self.extensions.contains_key(&registration.id) {
            return Err(ExtensionError::AlreadyRegistered);
        }
        if registration.id.trim().is_empty() {
            return Err(ExtensionError::InvalidRegistration("Empty id".to_string()));
        }
        self.validate_registration(&registration)?;
        self.extensions
            .insert(registration.id.clone(), registration);
        Ok(())
    }

    fn validate_registration(
        &self,
        registration: &ExtensionRegistration,
    ) -> Result<(), ExtensionError> {
        if registration
            .prompt_additions
            .iter()
            .any(|p| p.role.trim().is_empty())
        {
            return Err(ExtensionError::InvalidRegistration(
                "Missing role".to_string(),
            ));
        }
        if registration
            .prompt_additions
            .iter()
            .any(|p| p.content.trim().is_empty())
        {
            return Err(ExtensionError::InvalidRegistration(
                "Missing content".to_string(),
            ));
        }
        if registration
            .tool_config
            .disabled_tools
            .iter()
            .any(|t| t.trim().is_empty())
        {
            return Err(ExtensionError::InvalidRegistration(
                "Invalid tool name".to_string(),
            ));
        }
        if registration
            .sequence_hints
            .iter()
            .any(|h| h.when_tool.trim().is_empty())
        {
            return Err(ExtensionError::InvalidRegistration(
                "Missing tool hint".to_string(),
            ));
        }
        for (key, value) in &registration.tool_config.overrides {
            if key.trim().is_empty() || value.trim().is_empty() {
                return Err(ExtensionError::InvalidRegistration(
                    "Invalid override".to_string(),
                ));
            }
            if self
                .extensions
                .values()
                .any(|ext| ext.tool_config.overrides.get(key).is_some())
            {
                return Err(ExtensionError::InvalidRegistration(
                    "Conflicting tool override".to_string(),
                ));
            }
        }
        Ok(())
    }

    pub fn enable_extension(&mut self, id: &str, enabled: bool) -> Result<(), ExtensionError> {
        let extension = self
            .extensions
            .get_mut(id)
            .ok_or(ExtensionError::NotFound)?;
        extension.enabled = enabled;
        Ok(())
    }

    pub fn list_ordered(&self) -> Vec<ExtensionRegistration> {
        let mut extensions: Vec<_> = self.extensions.values().cloned().collect();
        extensions.sort_by(|a, b| b.priority.cmp(&a.priority));
        extensions
    }

    pub fn prompt_wiring(&self) -> PromptWiring {
        let mut wiring = PromptWiring::default();
        for extension in self.list_ordered() {
            if !extension.enabled {
                continue;
            }
            wiring.additions.extend(extension.prompt_additions.clone());
            wiring.tool_config.merge(extension.tool_config.clone());
            wiring
                .sequencing_hints
                .extend(extension.sequence_hints.clone());
        }
        wiring
    }
}

impl Default for ExtensionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extensions::ExtensionCallback;
    use async_trait::async_trait;

    struct Noop;

    #[async_trait]
    impl ExtensionCallback for Noop {}

    #[test]
    fn test_registry_ordering() {
        let mut registry = ExtensionRegistry::new();
        let a = ExtensionRegistration::new("a", Arc::new(Noop)).with_priority(1);
        let b = ExtensionRegistration::new("b", Arc::new(Noop)).with_priority(5);
        registry.register_extension(a).unwrap();
        registry.register_extension(b).unwrap();

        let ordered = registry.list_ordered();
        assert_eq!(ordered[0].id, "b");
    }

    #[test]
    fn test_registry_validation() {
        let mut registry = ExtensionRegistry::new();
        let invalid = ExtensionRegistration::new("", Arc::new(Noop));
        let err = registry.register_extension(invalid).unwrap_err();
        assert!(matches!(err, ExtensionError::InvalidRegistration(_)));
    }

    #[test]
    fn test_registry_conflicting_override() {
        let mut registry = ExtensionRegistry::new();
        let mut config = ToolConfig::default();
        config.overrides.insert("tool".to_string(), "a".to_string());
        registry
            .register_extension(
                ExtensionRegistration::new("a", Arc::new(Noop)).with_tool_config(config),
            )
            .unwrap();

        let mut config = ToolConfig::default();
        config.overrides.insert("tool".to_string(), "b".to_string());
        let err = registry
            .register_extension(
                ExtensionRegistration::new("b", Arc::new(Noop)).with_tool_config(config),
            )
            .unwrap_err();
        assert!(matches!(err, ExtensionError::InvalidRegistration(_)));
    }

    #[test]
    fn test_prompt_wiring_merge() {
        let mut registry = ExtensionRegistry::new();
        let mut config = ToolConfig::default();
        config.suggested_tools.push("tool1".to_string());
        let hint = super::super::prompt::ToolSequenceHint {
            when_tool: "a".to_string(),
            suggest_next: "b".to_string(),
        };
        registry
            .register_extension(
                ExtensionRegistration::new("a", Arc::new(Noop))
                    .with_tool_config(config)
                    .with_sequence_hints(vec![hint])
                    .with_prompt_additions(vec![PromptAddition {
                        role: "system".to_string(),
                        content: "hint".to_string(),
                    }]),
            )
            .unwrap();

        let wiring = registry.prompt_wiring();
        assert_eq!(wiring.additions.len(), 1);
        assert_eq!(wiring.sequencing_hints.len(), 1);
        assert!(
            wiring
                .tool_config
                .suggested_tools
                .contains(&"tool1".to_string())
        );
    }
}
