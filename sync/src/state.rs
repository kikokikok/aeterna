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
    pub stats: SyncStats
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
    pub policy_violations: u64
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
            stats: SyncStats::default()
        }
    }
}
