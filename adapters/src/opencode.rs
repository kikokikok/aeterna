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
    pub tools: Vec<Value>,
}

pub fn get_categorized_tools(adapter: &OpenCodeAdapter) -> Vec<OpenCodeToolCategory> {
    vec![
        OpenCodeToolCategory {
            name: "memory".to_string(),
            description: "Store, search, and manage memories across scopes".to_string(),
            tools: adapter.get_memory_tools(),
        },
        OpenCodeToolCategory {
            name: "knowledge".to_string(),
            description: "Query and explore the knowledge repository".to_string(),
            tools: adapter.get_knowledge_tools(),
        },
        OpenCodeToolCategory {
            name: "governance".to_string(),
            description: "Manage organizational governance and approval workflows".to_string(),
            tools: adapter.get_governance_tools(),
        },
        OpenCodeToolCategory {
            name: "policy".to_string(),
            description: "Create, validate, and manage Cedar policies".to_string(),
            tools: adapter.get_policy_tools(),
        },
        OpenCodeToolCategory {
            name: "sync".to_string(),
            description: "Synchronize memory and knowledge".to_string(),
            tools: adapter.get_sync_tools(),
        },
        OpenCodeToolCategory {
            name: "graph".to_string(),
            description: "Query and manage knowledge graph relationships".to_string(),
            tools: adapter.get_graph_tools(),
        },
        OpenCodeToolCategory {
            name: "cca".to_string(),
            description: "Confucius Code Agent capabilities".to_string(),
            tools: adapter.get_cca_tools(),
        },
        OpenCodeToolCategory {
            name: "codesearch".to_string(),
            description: "Semantic code search, call graph tracing, and index status".to_string(),
            tools: adapter.get_codesearch_tools(),
        },
    ]
}

pub struct CodeSearchContextEnhancer;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeContextInjection {
    pub search_tool: String,
    pub trace_callers_tool: String,
    pub trace_callees_tool: String,
    pub memory_link_tool: String,
    pub suggested_query: String,
}

impl CodeSearchContextEnhancer {
    pub fn new() -> Self {
        Self
    }

    pub fn build_code_context_injection(query: &str) -> CodeContextInjection {
        CodeContextInjection {
            search_tool: "codesearch_search".to_string(),
            trace_callers_tool: "codesearch_trace_callers".to_string(),
            trace_callees_tool: "codesearch_trace_callees".to_string(),
            memory_link_tool: "graph_related".to_string(),
            suggested_query: query.to_string(),
        }
    }

    pub fn call_graph_context_tools() -> Vec<String> {
        vec![
            "codesearch_trace_callers".to_string(),
            "codesearch_trace_callees".to_string(),
            "codesearch_graph".to_string(),
        ]
    }

    pub fn memory_link_tools() -> Vec<String> {
        vec![
            "graph_related".to_string(),
            "graph_link".to_string(),
            "graph_context".to_string(),
        ]
    }
}

impl Default for CodeSearchContextEnhancer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_category_serialization() {
        let category = OpenCodeToolCategory {
            name: "memory".to_string(),
            description: "Memory tools".to_string(),
            tools: vec![],
        };

        let json = serde_json::to_string(&category).unwrap();
        assert!(json.contains("memory"));
        assert!(json.contains("Memory tools"));
    }

    #[test]
    fn test_code_context_injection_fields() {
        let injection = CodeSearchContextEnhancer::build_code_context_injection("auth middleware");
        assert_eq!(injection.search_tool, "codesearch_search");
        assert_eq!(injection.trace_callers_tool, "codesearch_trace_callers");
        assert_eq!(injection.trace_callees_tool, "codesearch_trace_callees");
        assert_eq!(injection.memory_link_tool, "graph_related");
        assert_eq!(injection.suggested_query, "auth middleware");
    }

    #[test]
    fn test_call_graph_context_tools() {
        let tools = CodeSearchContextEnhancer::call_graph_context_tools();
        assert!(tools.contains(&"codesearch_trace_callers".to_string()));
        assert!(tools.contains(&"codesearch_trace_callees".to_string()));
        assert!(tools.contains(&"codesearch_graph".to_string()));
    }

    #[test]
    fn test_memory_link_tools() {
        let tools = CodeSearchContextEnhancer::memory_link_tools();
        assert!(tools.contains(&"graph_related".to_string()));
        assert!(tools.contains(&"graph_link".to_string()));
        assert!(tools.contains(&"graph_context".to_string()));
    }

    #[test]
    fn test_enhancer_default() {
        let _ = CodeSearchContextEnhancer::default();
    }
}
