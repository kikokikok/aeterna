use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct PromptWiring {
    pub additions: Vec<PromptAddition>,
    pub tool_config: ToolConfig,
    pub sequencing_hints: Vec<ToolSequenceHint>,
}

#[derive(Debug, Clone)]
pub struct ToolSequenceHint {
    pub when_tool: String,
    pub suggest_next: String,
}

#[derive(Debug, Clone)]
pub struct PromptAddition {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Default)]
pub struct ToolConfig {
    pub disabled_tools: Vec<String>,
    pub suggested_tools: Vec<String>,
    pub overrides: HashMap<String, String>,
    pub hints: Vec<String>,
    pub context_hints: Vec<String>,
}

impl ToolConfig {
    pub fn merge(&mut self, other: ToolConfig) {
        self.disabled_tools.extend(other.disabled_tools);
        self.suggested_tools.extend(other.suggested_tools);
        self.hints.extend(other.hints);
        self.context_hints.extend(other.context_hints);
        for (key, value) in other.overrides {
            self.overrides.insert(key, value);
        }
        self.dedup();
    }

    fn dedup(&mut self) {
        self.disabled_tools.sort();
        self.disabled_tools.dedup();
        self.suggested_tools.sort();
        self.suggested_tools.dedup();
        self.hints.sort();
        self.hints.dedup();
        self.context_hints.sort();
        self.context_hints.dedup();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_config_merge() {
        let mut a = ToolConfig::default();
        a.disabled_tools.push("a".to_string());
        let mut b = ToolConfig::default();
        b.disabled_tools.push("b".to_string());
        b.suggested_tools.push("tool".to_string());
        b.context_hints.push("hint".to_string());

        a.merge(b);
        assert!(a.disabled_tools.contains(&"a".to_string()));
        assert!(a.disabled_tools.contains(&"b".to_string()));
        assert!(a.suggested_tools.contains(&"tool".to_string()));
        assert!(a.context_hints.contains(&"hint".to_string()));
    }
}
