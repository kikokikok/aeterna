use crate::pointer::{HindsightPointerState, SummaryPointerState};
use mk_core::types::{MemoryLayer, SummaryDepth};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SyncState {
    pub version: String,
    pub last_sync_at: Option<i64>,
    pub last_knowledge_commit: Option<String>,
    pub knowledge_hashes: HashMap<String, String>,
    pub pointer_mapping: HashMap<String, String>,
    pub knowledge_layers: HashMap<String, mk_core::types::KnowledgeLayer>,
    pub failed_items: Vec<SyncFailure>,
    pub federation_conflicts: Vec<FederationConflict>,
    pub upstream_commits: HashMap<String, String>,
    pub stats: SyncStats,
    #[serde(default)]
    pub summary_state: SummaryPointerState,
    #[serde(default)]
    pub hindsight_state: HindsightPointerState
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FederationConflict {
    pub upstream_id: String,
    pub reason: String,
    pub detected_at: i64
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SyncFailure {
    pub knowledge_id: String,
    pub error: String,
    pub failed_at: i64,
    pub retry_count: u32
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SyncConflict {
    HashMismatch {
        knowledge_id: String,
        memory_id: String,
        expected_hash: String,
        actual_hash: String
    },
    OrphanedPointer {
        memory_id: String,
        knowledge_id: String
    },
    MissingPointer {
        knowledge_id: String,
        expected_memory_id: String
    },
    DuplicatePointer {
        knowledge_id: String,
        memory_ids: Vec<String>
    },
    StatusChange {
        knowledge_id: String,
        memory_id: String,
        new_status: mk_core::types::KnowledgeStatus
    },
    LayerMismatch {
        knowledge_id: String,
        memory_id: String,
        expected_layer: mk_core::types::KnowledgeLayer,
        actual_layer: mk_core::types::KnowledgeLayer
    },
    DetectionError {
        target_id: String,
        error: String
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SyncTrigger {
    Staleness {
        last_sync_at: i64,
        threshold_mins: u32
    },
    CommitMismatch {
        last_commit: String,
        head_commit: String
    },
    Manual
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SyncStats {
    pub total_syncs: u64,
    pub total_items_synced: u64,
    pub total_conflicts: u64,
    pub total_governance_blocks: u64,
    pub avg_sync_duration_ms: u64,
    pub drift_score: f32,
    pub policy_violations: u64,
    pub total_summaries_synced: u64,
    pub total_summaries_invalidated: u64,
    pub total_hindsight_patterns: u64
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SummarySyncTrigger {
    SourceContentChanged {
        entry_id: String,
        layer: MemoryLayer,
        previous_hash: String,
        new_hash: String
    },
    TimeThresholdExceeded {
        entry_id: String,
        layer: MemoryLayer,
        age_seconds: u64,
        threshold_seconds: u64
    },
    ChangeCountExceeded {
        entry_id: String,
        layer: MemoryLayer,
        change_count: u32,
        threshold: u32
    },
    ManualRefresh {
        entry_id: String,
        layer: MemoryLayer
    },
    LayerConfigChanged {
        layer: MemoryLayer,
        depths_added: Vec<SummaryDepth>,
        depths_removed: Vec<SummaryDepth>
    },
    ParentSummaryInvalidated {
        entry_id: String,
        layer: MemoryLayer,
        parent_layer: MemoryLayer
    }
}

impl Default for SyncState {
    fn default() -> Self {
        Self {
            version: "1.0".to_string(),
            last_sync_at: None,
            last_knowledge_commit: None,
            knowledge_hashes: HashMap::new(),
            pointer_mapping: HashMap::new(),
            knowledge_layers: HashMap::new(),
            failed_items: Vec::new(),
            federation_conflicts: Vec::new(),
            upstream_commits: HashMap::new(),
            stats: SyncStats::default(),
            summary_state: SummaryPointerState::default(),
            hindsight_state: HindsightPointerState::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_state_default() {
        let state = SyncState::default();
        assert_eq!(state.version, "1.0");
        assert!(state.last_sync_at.is_none());
        assert!(state.summary_state.pointers.is_empty());
        assert!(state.hindsight_state.pointers.is_empty());
    }

    #[test]
    fn test_sync_stats_default() {
        let stats = SyncStats::default();
        assert_eq!(stats.total_summaries_synced, 0);
        assert_eq!(stats.total_summaries_invalidated, 0);
        assert_eq!(stats.total_hindsight_patterns, 0);
    }

    #[test]
    fn test_summary_sync_trigger_serialization() {
        let triggers = vec![
            SummarySyncTrigger::SourceContentChanged {
                entry_id: "entry-1".to_string(),
                layer: MemoryLayer::Project,
                previous_hash: "old".to_string(),
                new_hash: "new".to_string()
            },
            SummarySyncTrigger::TimeThresholdExceeded {
                entry_id: "entry-1".to_string(),
                layer: MemoryLayer::Project,
                age_seconds: 3600,
                threshold_seconds: 1800
            },
            SummarySyncTrigger::ChangeCountExceeded {
                entry_id: "entry-1".to_string(),
                layer: MemoryLayer::Project,
                change_count: 10,
                threshold: 5
            },
            SummarySyncTrigger::ManualRefresh {
                entry_id: "entry-1".to_string(),
                layer: MemoryLayer::Project
            },
            SummarySyncTrigger::LayerConfigChanged {
                layer: MemoryLayer::Project,
                depths_added: vec![SummaryDepth::Detailed],
                depths_removed: vec![]
            },
            SummarySyncTrigger::ParentSummaryInvalidated {
                entry_id: "entry-1".to_string(),
                layer: MemoryLayer::Project,
                parent_layer: MemoryLayer::Team
            },
        ];

        for trigger in triggers {
            let json = serde_json::to_string(&trigger).unwrap();
            let deserialized: SummarySyncTrigger = serde_json::from_str(&json).unwrap();
            assert_eq!(trigger, deserialized);
        }
    }

    #[test]
    fn test_sync_state_with_summary_state() {
        let mut state = SyncState::default();

        let ptr = crate::pointer::SummaryPointer::new(
            "entry-1".to_string(),
            MemoryLayer::Project,
            SummaryDepth::Sentence,
            "hash".to_string(),
            "source".to_string(),
            50
        );
        state.summary_state.set_pointer(ptr);

        assert_eq!(state.summary_state.total_summaries, 1);

        let json = serde_json::to_string(&state).unwrap();
        let deserialized: SyncState = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.summary_state.total_summaries, 1);
    }

    #[test]
    fn test_sync_state_with_hindsight_state() {
        let mut state = SyncState::default();

        let ptr = crate::pointer::HindsightPointer::new("sig-123".to_string(), MemoryLayer::User);
        state.hindsight_state.set_pointer(ptr);

        assert_eq!(state.hindsight_state.total_patterns, 1);

        let json = serde_json::to_string(&state).unwrap();
        let deserialized: SyncState = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.hindsight_state.total_patterns, 1);
    }
}
