//! OpenCode Plugin Integration Module
//!
//! Re-exports and utilities for integrating Aeterna with OpenCode AI coding
//! assistant.
//!
//! The OpenCode adapter provides:
//! - Automatic context resolution from git environment
//! - Tool categorization for OpenCode UI
//! - Session lifecycle hooks for memory management
//! - MCP-compliant request/response handling

pub use crate::ecosystem::{EcosystemAdapter, OpenCodeAdapter};
pub use crate::hooks::MemoryContextHooks;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenCodeToolCategory {
    pub name: String,
    pub description: String,
    pub tools: Vec<Value>
}

pub fn get_categorized_tools(adapter: &OpenCodeAdapter) -> Vec<OpenCodeToolCategory> {
    vec![
        OpenCodeToolCategory {
            name: "memory".to_string(),
            description: "Store, search, and manage memories across scopes".to_string(),
            tools: adapter.get_memory_tools()
        },
        OpenCodeToolCategory {
            name: "knowledge".to_string(),
            description: "Query and explore the knowledge repository".to_string(),
            tools: adapter.get_knowledge_tools()
        },
        OpenCodeToolCategory {
            name: "governance".to_string(),
            description: "Manage organizational governance and approval workflows".to_string(),
            tools: adapter.get_governance_tools()
        },
        OpenCodeToolCategory {
            name: "policy".to_string(),
            description: "Create, validate, and manage Cedar policies".to_string(),
            tools: adapter.get_policy_tools()
        },
        OpenCodeToolCategory {
            name: "sync".to_string(),
            description: "Synchronize memory and knowledge".to_string(),
            tools: adapter.get_sync_tools()
        },
        OpenCodeToolCategory {
            name: "graph".to_string(),
            description: "Query memory relationship graphs".to_string(),
            tools: adapter.get_graph_tools()
        },
        OpenCodeToolCategory {
            name: "cca".to_string(),
            description: "Confucius Code Agent capabilities".to_string(),
            tools: adapter.get_cca_tools()
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_category_serialization() {
        let category = OpenCodeToolCategory {
            name: "memory".to_string(),
            description: "Memory tools".to_string(),
            tools: vec![]
        };

        let json = serde_json::to_string(&category).unwrap();
        assert!(json.contains("memory"));
        assert!(json.contains("Memory tools"));
    }
}
