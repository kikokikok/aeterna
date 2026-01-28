//! # Operation Hints System
//!
//! Capability toggles for controlling system behavior at runtime.
//!
//! Hints allow users to enable/disable features like reasoning, summarization,
//! caching, etc. This controls the system's intelligence level based on needs
//! (cost, latency, verbosity).
//!
//! # M-CANONICAL-DOCS
//!
//! ## Purpose
//! Provides a flexible system for toggling capabilities at three surfaces:
//! - Per-request: CLI flags or API parameters
//! - Per-context: `.aeterna/context.toml` configuration
//! - Per-tenant: Server-side organizational defaults
//!
//! ## Precedence (highest to lowest)
//! 1. Per-request hints (CLI flags, API params)
//! 2. Environment variables (`AETERNA_HINTS_*`)
//! 3. Context file (`.aeterna/context.toml`)
//! 4. Org/tenant defaults
//! 5. System defaults
//!
//! ## Usage
//! ```rust
//! use mk_core::hints::{HintPreset, OperationHints};
//!
//! // Use a preset
//! let hints = OperationHints::from_preset(HintPreset::Fast);
//!
//! // Or build custom hints
//! let hints = OperationHints::default()
//!     .with_reasoning(false)
//!     .with_caching(true);
//!
//! // Parse from CLI-style string
//! let hints = OperationHints::parse_hint_string("no-llm,no-reasoning,caching");
//! ```

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use strum::{Display, EnumIter, EnumString};
use utoipa::ToSchema;

/// Hint presets for common use cases.
///
/// Presets provide sensible defaults for different operational contexts.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    ToSchema,
    JsonSchema,
    Display,
    EnumString,
    EnumIter,
    Default,
)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum HintPreset {
    /// Minimal processing: CI/CD, high-volume, cost-sensitive
    /// Disables: reasoning, multi-hop, summarization, llm
    Minimal,

    /// Fast processing: Interactive, latency-sensitive
    /// Disables: reasoning, multi-hop
    Fast,

    /// Standard processing: Default for humans
    /// All features enabled with sensible defaults
    #[default]
    Standard,

    /// Full processing: Deep analysis, debugging
    /// All features enabled, more thorough processing
    Full,

    /// Offline mode: Disconnected work
    /// Disables: llm, governance sync
    Offline,

    /// Agent mode: AI agent default
    /// Optimized for agent workflows
    Agent,

    /// Custom: User-defined hints (no preset applied)
    Custom
}

impl HintPreset {
    /// Get the description for this preset
    #[must_use]
    pub fn description(&self) -> &'static str {
        match self {
            HintPreset::Minimal => "CI/CD, high-volume, cost-sensitive. Disables LLM features.",
            HintPreset::Fast => "Interactive, latency-sensitive. Skips expensive reasoning.",
            HintPreset::Standard => "Default for humans. All features with sensible defaults.",
            HintPreset::Full => "Deep analysis, debugging. More thorough processing.",
            HintPreset::Offline => "Disconnected work. No external API calls.",
            HintPreset::Agent => "AI agent workflows. Optimized for automation.",
            HintPreset::Custom => "User-defined hints. No preset applied."
        }
    }

    /// Get typical use cases for this preset
    #[must_use]
    pub fn use_cases(&self) -> &'static [&'static str] {
        match self {
            HintPreset::Minimal => &["CI/CD pipelines", "Batch processing", "Cost optimization"],
            HintPreset::Fast => &["Interactive CLI", "Real-time queries", "Development"],
            HintPreset::Standard => &["General usage", "Human operators", "Documentation"],
            HintPreset::Full => &["Debugging", "Deep analysis", "Compliance audits"],
            HintPreset::Offline => &["Air-gapped environments", "Network issues", "Local dev"],
            HintPreset::Agent => &["AI agents", "Automation", "Background tasks"],
            HintPreset::Custom => &["Advanced users", "Specific requirements"]
        }
    }
}

/// Operation hints for controlling system behavior.
///
/// Each hint is a boolean toggle that enables/disables a capability.
/// Hints can be combined and have precedence rules for resolution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OperationHints {
    /// The preset these hints are based on (if any)
    #[serde(default)]
    pub preset: HintPreset,

    /// Enable pre-retrieval reasoning and query refinement (MemR³)
    #[serde(default = "default_true")]
    pub reasoning: bool,

    /// Enable multi-hop retrieval for complex queries
    #[serde(default = "default_true")]
    pub multi_hop: bool,

    /// Enable layer summaries and compressed context
    #[serde(default = "default_true")]
    pub summarization: bool,

    /// Enable query/result caching
    #[serde(default = "default_true")]
    pub caching: bool,

    /// Enable policy checks and approval workflows
    #[serde(default = "default_true")]
    pub governance: bool,

    /// Enable detailed audit logging
    #[serde(default = "default_true")]
    pub audit: bool,

    /// Enable any LLM-powered features
    #[serde(default = "default_true")]
    pub llm: bool,

    /// Enable automatic memory promotion based on rewards
    #[serde(default = "default_false")]
    pub auto_promote: bool,

    /// Enable knowledge drift detection
    #[serde(default = "default_true")]
    pub drift_check: bool,

    /// Enable graph traversal for related memories
    #[serde(default = "default_true")]
    pub graph: bool,

    /// Enable CCA (Confucius Code Agent) capabilities
    #[serde(default = "default_true")]
    pub cca: bool,

    /// Enable A2A (Agent-to-Agent) protocol
    #[serde(default = "default_true")]
    pub a2a: bool,

    /// Enable verbose output for debugging
    #[serde(default = "default_false")]
    pub verbose: bool,

    /// Custom hint overrides (for extensibility)
    #[serde(default)]
    pub custom: HashMap<String, bool>
}

fn default_true() -> bool {
    true
}

fn default_false() -> bool {
    false
}

impl Default for OperationHints {
    fn default() -> Self {
        Self::from_preset(HintPreset::Standard)
    }
}

impl OperationHints {
    /// Create hints from a preset
    #[must_use]
    pub fn from_preset(preset: HintPreset) -> Self {
        match preset {
            HintPreset::Minimal => Self {
                preset,
                reasoning: false,
                multi_hop: false,
                summarization: false,
                caching: true,
                governance: true,
                audit: false,
                llm: false,
                auto_promote: false,
                drift_check: false,
                graph: false,
                cca: false,
                a2a: false,
                verbose: false,
                custom: HashMap::new()
            },
            HintPreset::Fast => Self {
                preset,
                reasoning: false,
                multi_hop: false,
                summarization: true,
                caching: true,
                governance: true,
                audit: true,
                llm: true,
                auto_promote: false,
                drift_check: true,
                graph: true,
                cca: false,
                a2a: true,
                verbose: false,
                custom: HashMap::new()
            },
            HintPreset::Standard => Self {
                preset,
                reasoning: true,
                multi_hop: true,
                summarization: true,
                caching: true,
                governance: true,
                audit: true,
                llm: true,
                auto_promote: false,
                drift_check: true,
                graph: true,
                cca: true,
                a2a: true,
                verbose: false,
                custom: HashMap::new()
            },
            HintPreset::Full => Self {
                preset,
                reasoning: true,
                multi_hop: true,
                summarization: true,
                caching: true,
                governance: true,
                audit: true,
                llm: true,
                auto_promote: true,
                drift_check: true,
                graph: true,
                cca: true,
                a2a: true,
                verbose: true,
                custom: HashMap::new()
            },
            HintPreset::Offline => Self {
                preset,
                reasoning: false,
                multi_hop: false,
                summarization: false,
                caching: true,
                governance: false,
                audit: true,
                llm: false,
                auto_promote: false,
                drift_check: false,
                graph: true,
                cca: false,
                a2a: false,
                verbose: false,
                custom: HashMap::new()
            },
            HintPreset::Agent => Self {
                preset,
                reasoning: true,
                multi_hop: true,
                summarization: true,
                caching: true,
                governance: true,
                audit: true,
                llm: true,
                auto_promote: true,
                drift_check: true,
                graph: true,
                cca: true,
                a2a: true,
                verbose: false,
                custom: HashMap::new()
            },
            HintPreset::Custom => Self {
                preset,
                reasoning: true,
                multi_hop: true,
                summarization: true,
                caching: true,
                governance: true,
                audit: true,
                llm: true,
                auto_promote: false,
                drift_check: true,
                graph: true,
                cca: true,
                a2a: true,
                verbose: false,
                custom: HashMap::new()
            }
        }
    }

    /// Parse hints from a CLI-style string.
    ///
    /// Format: `hint1,hint2,no-hint3,hint4=true`
    ///
    /// Examples:
    /// - `"no-llm,no-reasoning"` - disable llm and reasoning
    /// - `"fast"` - use fast preset
    /// - `"minimal,verbose"` - minimal preset with verbose enabled
    #[must_use]
    pub fn parse_hint_string(s: &str) -> Self {
        let mut hints = Self::default();

        for part in s.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }

            // Check if it's a preset name
            if let Ok(preset) = part.parse::<HintPreset>() {
                hints = Self::from_preset(preset);
                continue;
            }

            // Parse individual hint
            let (name, value) = if let Some(stripped) = part.strip_prefix("no-") {
                (stripped, false)
            } else if let Some((name, val)) = part.split_once('=') {
                let value = val.eq_ignore_ascii_case("true") || val == "1";
                (name, value)
            } else {
                (part, true)
            };

            hints.set_hint(name, value);
        }

        hints.preset = HintPreset::Custom;
        hints
    }

    /// Set a hint by name
    pub fn set_hint(&mut self, name: &str, value: bool) {
        match name.to_lowercase().as_str() {
            "reasoning" => self.reasoning = value,
            "multi-hop" | "multihop" | "multi_hop" => self.multi_hop = value,
            "summarization" | "summary" => self.summarization = value,
            "caching" | "cache" => self.caching = value,
            "governance" | "gov" => self.governance = value,
            "audit" => self.audit = value,
            "llm" => self.llm = value,
            "auto-promote" | "autopromote" | "auto_promote" => self.auto_promote = value,
            "drift-check" | "driftcheck" | "drift_check" | "drift" => self.drift_check = value,
            "graph" => self.graph = value,
            "cca" => self.cca = value,
            "a2a" => self.a2a = value,
            "verbose" => self.verbose = value,
            _ => {
                self.custom.insert(name.to_string(), value);
            }
        }
    }

    /// Get a hint by name
    #[must_use]
    pub fn get_hint(&self, name: &str) -> Option<bool> {
        match name.to_lowercase().as_str() {
            "reasoning" => Some(self.reasoning),
            "multi-hop" | "multihop" | "multi_hop" => Some(self.multi_hop),
            "summarization" | "summary" => Some(self.summarization),
            "caching" | "cache" => Some(self.caching),
            "governance" | "gov" => Some(self.governance),
            "audit" => Some(self.audit),
            "llm" => Some(self.llm),
            "auto-promote" | "autopromote" | "auto_promote" => Some(self.auto_promote),
            "drift-check" | "driftcheck" | "drift_check" | "drift" => Some(self.drift_check),
            "graph" => Some(self.graph),
            "cca" => Some(self.cca),
            "a2a" => Some(self.a2a),
            "verbose" => Some(self.verbose),
            _ => self.custom.get(name).copied()
        }
    }

    /// Builder: set reasoning hint
    #[must_use]
    pub fn with_reasoning(mut self, value: bool) -> Self {
        self.reasoning = value;
        self.preset = HintPreset::Custom;
        self
    }

    /// Builder: set multi-hop hint
    #[must_use]
    pub fn with_multi_hop(mut self, value: bool) -> Self {
        self.multi_hop = value;
        self.preset = HintPreset::Custom;
        self
    }

    /// Builder: set summarization hint
    #[must_use]
    pub fn with_summarization(mut self, value: bool) -> Self {
        self.summarization = value;
        self.preset = HintPreset::Custom;
        self
    }

    /// Builder: set caching hint
    #[must_use]
    pub fn with_caching(mut self, value: bool) -> Self {
        self.caching = value;
        self.preset = HintPreset::Custom;
        self
    }

    /// Builder: set governance hint
    #[must_use]
    pub fn with_governance(mut self, value: bool) -> Self {
        self.governance = value;
        self.preset = HintPreset::Custom;
        self
    }

    /// Builder: set audit hint
    #[must_use]
    pub fn with_audit(mut self, value: bool) -> Self {
        self.audit = value;
        self.preset = HintPreset::Custom;
        self
    }

    /// Builder: set llm hint
    #[must_use]
    pub fn with_llm(mut self, value: bool) -> Self {
        self.llm = value;
        self.preset = HintPreset::Custom;
        self
    }

    /// Builder: set auto-promote hint
    #[must_use]
    pub fn with_auto_promote(mut self, value: bool) -> Self {
        self.auto_promote = value;
        self.preset = HintPreset::Custom;
        self
    }

    /// Builder: set drift-check hint
    #[must_use]
    pub fn with_drift_check(mut self, value: bool) -> Self {
        self.drift_check = value;
        self.preset = HintPreset::Custom;
        self
    }

    /// Builder: set graph hint
    #[must_use]
    pub fn with_graph(mut self, value: bool) -> Self {
        self.graph = value;
        self.preset = HintPreset::Custom;
        self
    }

    /// Builder: set cca hint
    #[must_use]
    pub fn with_cca(mut self, value: bool) -> Self {
        self.cca = value;
        self.preset = HintPreset::Custom;
        self
    }

    /// Builder: set a2a hint
    #[must_use]
    pub fn with_a2a(mut self, value: bool) -> Self {
        self.a2a = value;
        self.preset = HintPreset::Custom;
        self
    }

    /// Builder: set verbose hint
    #[must_use]
    pub fn with_verbose(mut self, value: bool) -> Self {
        self.verbose = value;
        self.preset = HintPreset::Custom;
        self
    }

    /// Builder: set custom hint
    #[must_use]
    pub fn with_custom(mut self, name: impl Into<String>, value: bool) -> Self {
        self.custom.insert(name.into(), value);
        self.preset = HintPreset::Custom;
        self
    }

    /// Merge with another hints object (other takes precedence)
    #[must_use]
    pub fn merge(&self, other: &Self) -> Self {
        let mut result = self.clone();

        // Only override if other has explicit customizations
        if other.preset == HintPreset::Custom || other.preset != self.preset {
            result.reasoning = other.reasoning;
            result.multi_hop = other.multi_hop;
            result.summarization = other.summarization;
            result.caching = other.caching;
            result.governance = other.governance;
            result.audit = other.audit;
            result.llm = other.llm;
            result.auto_promote = other.auto_promote;
            result.drift_check = other.drift_check;
            result.graph = other.graph;
            result.cca = other.cca;
            result.a2a = other.a2a;
            result.verbose = other.verbose;
            result.preset = other.preset;
        }

        // Always merge custom hints
        for (k, v) in &other.custom {
            result.custom.insert(k.clone(), *v);
        }

        result
    }

    /// Check if any LLM features are enabled
    #[must_use]
    pub fn has_llm_features(&self) -> bool {
        self.llm && (self.reasoning || self.summarization || self.cca)
    }

    /// Check if this is a minimal/lightweight configuration
    #[must_use]
    pub fn is_lightweight(&self) -> bool {
        !self.reasoning && !self.multi_hop && !self.summarization
    }

    /// Convert to a hint string for CLI/API
    #[must_use]
    pub fn to_hint_string(&self) -> String {
        let mut parts = Vec::new();

        if !self.reasoning {
            parts.push("no-reasoning");
        }
        if !self.multi_hop {
            parts.push("no-multi-hop");
        }
        if !self.summarization {
            parts.push("no-summarization");
        }
        if !self.caching {
            parts.push("no-caching");
        }
        if !self.governance {
            parts.push("no-governance");
        }
        if !self.audit {
            parts.push("no-audit");
        }
        if !self.llm {
            parts.push("no-llm");
        }
        if self.auto_promote {
            parts.push("auto-promote");
        }
        if !self.drift_check {
            parts.push("no-drift-check");
        }
        if !self.graph {
            parts.push("no-graph");
        }
        if !self.cca {
            parts.push("no-cca");
        }
        if !self.a2a {
            parts.push("no-a2a");
        }
        if self.verbose {
            parts.push("verbose");
        }

        let custom_parts: Vec<String> = self
            .custom
            .iter()
            .map(|(k, v)| if *v { k.clone() } else { format!("no-{k}") })
            .collect();

        for part in &custom_parts {
            parts.push(part.as_str());
        }

        if parts.is_empty() {
            "standard".to_string()
        } else {
            parts.join(",")
        }
    }

    /// Get all hint names and their current values
    #[must_use]
    pub fn all_hints(&self) -> Vec<(&'static str, bool)> {
        let mut hints = vec![
            ("reasoning", self.reasoning),
            ("multi-hop", self.multi_hop),
            ("summarization", self.summarization),
            ("caching", self.caching),
            ("governance", self.governance),
            ("audit", self.audit),
            ("llm", self.llm),
            ("auto-promote", self.auto_promote),
            ("drift-check", self.drift_check),
            ("graph", self.graph),
            ("cca", self.cca),
            ("a2a", self.a2a),
            ("verbose", self.verbose),
        ];

        // Note: custom hints are not included in this method
        // Use `custom` field directly to access custom hints
        hints.sort_by_key(|(name, _)| *name);
        hints
    }
}

/// Hints configuration for context.toml
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, ToSchema, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub struct HintsConfig {
    /// Default preset to use
    #[serde(default)]
    pub preset: Option<HintPreset>,

    /// Individual hint overrides
    #[serde(flatten)]
    pub overrides: HashMap<String, bool>
}

impl HintsConfig {
    /// Convert to OperationHints
    #[must_use]
    pub fn to_operation_hints(&self) -> OperationHints {
        let mut hints = match self.preset {
            Some(preset) => OperationHints::from_preset(preset),
            None => OperationHints::default()
        };

        for (name, value) in &self.overrides {
            hints.set_hint(name, *value);
        }

        hints
    }
}

/// Environment-based hints resolution
impl OperationHints {
    /// Load hints from environment variables.
    ///
    /// Environment variables:
    /// - `AETERNA_HINTS`: Comma-separated hint string
    /// - `AETERNA_HINTS_PRESET`: Preset name
    /// - `AETERNA_HINTS_REASONING`: true/false
    /// - `AETERNA_HINTS_LLM`: true/false
    /// - etc.
    #[must_use]
    pub fn from_env() -> Self {
        let mut hints = Self::default();

        // Check for preset first
        if let Ok(preset_str) = std::env::var("AETERNA_HINTS_PRESET")
            && let Ok(preset) = preset_str.parse::<HintPreset>()
        {
            hints = Self::from_preset(preset);
        }

        // Check for hint string
        if let Ok(hint_str) = std::env::var("AETERNA_HINTS") {
            hints = Self::parse_hint_string(&hint_str);
        }

        // Check individual environment variables
        let env_hints = [
            ("AETERNA_HINTS_REASONING", "reasoning"),
            ("AETERNA_HINTS_MULTI_HOP", "multi-hop"),
            ("AETERNA_HINTS_SUMMARIZATION", "summarization"),
            ("AETERNA_HINTS_CACHING", "caching"),
            ("AETERNA_HINTS_GOVERNANCE", "governance"),
            ("AETERNA_HINTS_AUDIT", "audit"),
            ("AETERNA_HINTS_LLM", "llm"),
            ("AETERNA_HINTS_AUTO_PROMOTE", "auto-promote"),
            ("AETERNA_HINTS_DRIFT_CHECK", "drift-check"),
            ("AETERNA_HINTS_GRAPH", "graph"),
            ("AETERNA_HINTS_CCA", "cca"),
            ("AETERNA_HINTS_A2A", "a2a"),
            ("AETERNA_HINTS_VERBOSE", "verbose")
        ];

        for (env_var, hint_name) in env_hints {
            if let Ok(value) = std::env::var(env_var) {
                let bool_value = value.eq_ignore_ascii_case("true") || value == "1";
                hints.set_hint(hint_name, bool_value);
            }
        }

        hints
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preset_minimal() {
        let hints = OperationHints::from_preset(HintPreset::Minimal);
        assert!(!hints.reasoning);
        assert!(!hints.multi_hop);
        assert!(!hints.summarization);
        assert!(!hints.llm);
        assert!(hints.caching);
        assert!(hints.governance);
    }

    #[test]
    fn test_preset_fast() {
        let hints = OperationHints::from_preset(HintPreset::Fast);
        assert!(!hints.reasoning);
        assert!(!hints.multi_hop);
        assert!(hints.summarization);
        assert!(hints.llm);
        assert!(hints.caching);
    }

    #[test]
    fn test_preset_standard() {
        let hints = OperationHints::from_preset(HintPreset::Standard);
        assert!(hints.reasoning);
        assert!(hints.multi_hop);
        assert!(hints.summarization);
        assert!(hints.llm);
        assert!(hints.caching);
    }

    #[test]
    fn test_preset_full() {
        let hints = OperationHints::from_preset(HintPreset::Full);
        assert!(hints.reasoning);
        assert!(hints.multi_hop);
        assert!(hints.auto_promote);
        assert!(hints.verbose);
    }

    #[test]
    fn test_preset_offline() {
        let hints = OperationHints::from_preset(HintPreset::Offline);
        assert!(!hints.reasoning);
        assert!(!hints.llm);
        assert!(!hints.governance);
        assert!(!hints.a2a);
        assert!(hints.caching);
        assert!(hints.graph);
    }

    #[test]
    fn test_parse_hint_string_simple() {
        let hints = OperationHints::parse_hint_string("no-llm,no-reasoning");
        assert!(!hints.llm);
        assert!(!hints.reasoning);
        assert!(hints.caching);
    }

    #[test]
    fn test_parse_hint_string_with_preset() {
        let hints = OperationHints::parse_hint_string("minimal,verbose");
        assert!(!hints.llm);
        assert!(hints.verbose);
    }

    #[test]
    fn test_parse_hint_string_with_equals() {
        let hints = OperationHints::parse_hint_string("llm=false,verbose=true");
        assert!(!hints.llm);
        assert!(hints.verbose);
    }

    #[test]
    fn test_builder_pattern() {
        let hints = OperationHints::default()
            .with_reasoning(false)
            .with_llm(false)
            .with_verbose(true);

        assert!(!hints.reasoning);
        assert!(!hints.llm);
        assert!(hints.verbose);
        assert_eq!(hints.preset, HintPreset::Custom);
    }

    #[test]
    fn test_merge_hints() {
        let base = OperationHints::from_preset(HintPreset::Standard);
        let overrides = OperationHints::default()
            .with_reasoning(false)
            .with_verbose(true);

        let merged = base.merge(&overrides);
        assert!(!merged.reasoning);
        assert!(merged.verbose);
    }

    #[test]
    fn test_to_hint_string() {
        let hints = OperationHints::from_preset(HintPreset::Minimal);
        let s = hints.to_hint_string();
        assert!(s.contains("no-reasoning"));
        assert!(s.contains("no-llm"));
    }

    #[test]
    fn test_get_set_hint() {
        let mut hints = OperationHints::default();
        hints.set_hint("reasoning", false);
        assert_eq!(hints.get_hint("reasoning"), Some(false));

        hints.set_hint("custom-hint", true);
        assert_eq!(hints.get_hint("custom-hint"), Some(true));
    }

    #[test]
    fn test_has_llm_features() {
        let hints = OperationHints::from_preset(HintPreset::Standard);
        assert!(hints.has_llm_features());

        let hints = OperationHints::from_preset(HintPreset::Minimal);
        assert!(!hints.has_llm_features());
    }

    #[test]
    fn test_is_lightweight() {
        let hints = OperationHints::from_preset(HintPreset::Minimal);
        assert!(hints.is_lightweight());

        let hints = OperationHints::from_preset(HintPreset::Standard);
        assert!(!hints.is_lightweight());
    }

    #[test]
    fn test_hints_config_to_operation_hints() {
        let config = HintsConfig {
            preset: Some(HintPreset::Fast),
            overrides: {
                let mut m = HashMap::new();
                m.insert("verbose".to_string(), true);
                m
            }
        };

        let hints = config.to_operation_hints();
        assert!(!hints.reasoning); // From Fast preset
        assert!(hints.verbose); // From override
    }

    #[test]
    fn test_preset_serialization() {
        let preset = HintPreset::Minimal;
        let json = serde_json::to_string(&preset).unwrap();
        assert_eq!(json, "\"minimal\"");

        let deserialized: HintPreset = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, HintPreset::Minimal);
    }

    #[test]
    fn test_hints_serialization() {
        let hints = OperationHints::from_preset(HintPreset::Fast);
        let json = serde_json::to_string(&hints).unwrap();
        let deserialized: OperationHints = serde_json::from_str(&json).unwrap();
        assert_eq!(hints, deserialized);
    }

    #[test]
    fn test_all_hints() {
        let hints = OperationHints::default();
        let all = hints.all_hints();
        assert!(all.len() >= 13); // At least 13 built-in hints
        assert!(all.iter().any(|(name, _)| *name == "reasoning"));
        assert!(all.iter().any(|(name, _)| *name == "llm"));
    }

    #[test]
    fn test_preset_description() {
        assert!(!HintPreset::Minimal.description().is_empty());
        assert!(!HintPreset::Fast.description().is_empty());
        assert!(!HintPreset::Standard.description().is_empty());
    }

    #[test]
    fn test_preset_use_cases() {
        assert!(!HintPreset::Minimal.use_cases().is_empty());
        assert!(!HintPreset::Fast.use_cases().is_empty());
    }
}
