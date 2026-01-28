use mk_core::types::{MemoryLayer, SummaryDepth};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum SummarySyncEvent {
    Created(SummaryCreated),
    Updated(SummaryUpdated),
    Invalidated(SummaryInvalidated),
    Deleted(SummaryDeleted)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SummaryCreated {
    pub entry_id: String,
    pub layer: MemoryLayer,
    pub depth: SummaryDepth,
    pub content_hash: String,
    pub token_count: u32,
    pub created_at: i64
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SummaryUpdated {
    pub entry_id: String,
    pub layer: MemoryLayer,
    pub depth: SummaryDepth,
    pub previous_hash: String,
    pub new_hash: String,
    pub token_count: u32,
    pub updated_at: i64,
    pub reason: SummaryUpdateReason
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum SummaryUpdateReason {
    SourceContentChanged,
    ConfigurationChanged,
    ManualRefresh,
    ScheduledUpdate,
    PersonalizationContextChanged
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SummaryInvalidated {
    pub entry_id: String,
    pub layer: MemoryLayer,
    pub depths: Vec<SummaryDepth>,
    pub reason: InvalidationReason,
    pub invalidated_at: i64,
    pub source_content_hash: Option<String>
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum InvalidationReason {
    SourceContentChanged {
        previous_hash: String,
        new_hash: String
    },
    SourceDeleted,
    ConfigurationChanged,
    ManualInvalidation,
    StaleThresholdExceeded {
        age_seconds: u64
    },
    ParentLayerChanged {
        parent_layer: MemoryLayer
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SummaryDeleted {
    pub entry_id: String,
    pub layer: MemoryLayer,
    pub depths: Vec<SummaryDepth>,
    pub deleted_at: i64,
    pub reason: DeletionReason
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum DeletionReason {
    SourceDeleted,
    LayerPruned,
    ManualDeletion,
    TenantCleanup
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HindsightSyncEvent {
    pub event_type: HindsightEventType,
    pub tenant_id: String,
    pub timestamp: i64
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum HindsightEventType {
    ErrorSignatureCreated {
        signature_id: String,
        error_type: String
    },
    ResolutionRecorded {
        resolution_id: String,
        error_signature_id: String,
        success: bool
    },
    HindsightNoteCreated {
        note_id: String,
        error_signature_id: String
    },
    HindsightNoteUpdated {
        note_id: String,
        previous_hash: String,
        new_hash: String
    },
    ResolutionPromoted {
        resolution_id: String,
        from_layer: MemoryLayer,
        to_layer: MemoryLayer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_summary_created_serialization() {
        let event = SummarySyncEvent::Created(SummaryCreated {
            entry_id: "entry-123".to_string(),
            layer: MemoryLayer::Project,
            depth: SummaryDepth::Sentence,
            content_hash: "abc123".to_string(),
            token_count: 50,
            created_at: 1704067200
        });

        let json = serde_json::to_string(&event).unwrap();
        let deserialized: SummarySyncEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, deserialized);
    }

    #[test]
    fn test_summary_updated_serialization() {
        let event = SummarySyncEvent::Updated(SummaryUpdated {
            entry_id: "entry-123".to_string(),
            layer: MemoryLayer::Team,
            depth: SummaryDepth::Paragraph,
            previous_hash: "old123".to_string(),
            new_hash: "new456".to_string(),
            token_count: 200,
            updated_at: 1704067200,
            reason: SummaryUpdateReason::SourceContentChanged
        });

        let json = serde_json::to_string(&event).unwrap();
        let deserialized: SummarySyncEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, deserialized);
    }

    #[test]
    fn test_summary_invalidated_serialization() {
        let event = SummarySyncEvent::Invalidated(SummaryInvalidated {
            entry_id: "entry-123".to_string(),
            layer: MemoryLayer::Session,
            depths: vec![SummaryDepth::Sentence, SummaryDepth::Paragraph],
            reason: InvalidationReason::SourceContentChanged {
                previous_hash: "old".to_string(),
                new_hash: "new".to_string()
            },
            invalidated_at: 1704067200,
            source_content_hash: Some("content-hash".to_string())
        });

        let json = serde_json::to_string(&event).unwrap();
        let deserialized: SummarySyncEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, deserialized);
    }

    #[test]
    fn test_summary_deleted_serialization() {
        let event = SummarySyncEvent::Deleted(SummaryDeleted {
            entry_id: "entry-123".to_string(),
            layer: MemoryLayer::User,
            depths: vec![SummaryDepth::Detailed],
            deleted_at: 1704067200,
            reason: DeletionReason::SourceDeleted
        });

        let json = serde_json::to_string(&event).unwrap();
        let deserialized: SummarySyncEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, deserialized);
    }

    #[test]
    fn test_invalidation_reason_variants() {
        let reasons = vec![
            InvalidationReason::SourceContentChanged {
                previous_hash: "old".to_string(),
                new_hash: "new".to_string()
            },
            InvalidationReason::SourceDeleted,
            InvalidationReason::ConfigurationChanged,
            InvalidationReason::ManualInvalidation,
            InvalidationReason::StaleThresholdExceeded { age_seconds: 3600 },
            InvalidationReason::ParentLayerChanged {
                parent_layer: MemoryLayer::Org
            },
        ];

        for reason in reasons {
            let json = serde_json::to_string(&reason).unwrap();
            let deserialized: InvalidationReason = serde_json::from_str(&json).unwrap();
            assert_eq!(reason, deserialized);
        }
    }

    #[test]
    fn test_summary_update_reason_variants() {
        let reasons = vec![
            SummaryUpdateReason::SourceContentChanged,
            SummaryUpdateReason::ConfigurationChanged,
            SummaryUpdateReason::ManualRefresh,
            SummaryUpdateReason::ScheduledUpdate,
            SummaryUpdateReason::PersonalizationContextChanged,
        ];

        for reason in reasons {
            let json = serde_json::to_string(&reason).unwrap();
            let deserialized: SummaryUpdateReason = serde_json::from_str(&json).unwrap();
            assert_eq!(reason, deserialized);
        }
    }

    #[test]
    fn test_deletion_reason_variants() {
        let reasons = vec![
            DeletionReason::SourceDeleted,
            DeletionReason::LayerPruned,
            DeletionReason::ManualDeletion,
            DeletionReason::TenantCleanup,
        ];

        for reason in reasons {
            let json = serde_json::to_string(&reason).unwrap();
            let deserialized: DeletionReason = serde_json::from_str(&json).unwrap();
            assert_eq!(reason, deserialized);
        }
    }

    #[test]
    fn test_hindsight_sync_event_error_signature_created() {
        let event = HindsightSyncEvent {
            event_type: HindsightEventType::ErrorSignatureCreated {
                signature_id: "sig-123".to_string(),
                error_type: "NullPointerException".to_string()
            },
            tenant_id: "tenant-1".to_string(),
            timestamp: 1704067200
        };

        let json = serde_json::to_string(&event).unwrap();
        let deserialized: HindsightSyncEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, deserialized);
    }

    #[test]
    fn test_hindsight_sync_event_resolution_recorded() {
        let event = HindsightSyncEvent {
            event_type: HindsightEventType::ResolutionRecorded {
                resolution_id: "res-456".to_string(),
                error_signature_id: "sig-123".to_string(),
                success: true
            },
            tenant_id: "tenant-1".to_string(),
            timestamp: 1704067200
        };

        let json = serde_json::to_string(&event).unwrap();
        let deserialized: HindsightSyncEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, deserialized);
    }

    #[test]
    fn test_hindsight_sync_event_note_created() {
        let event = HindsightSyncEvent {
            event_type: HindsightEventType::HindsightNoteCreated {
                note_id: "note-789".to_string(),
                error_signature_id: "sig-123".to_string()
            },
            tenant_id: "tenant-1".to_string(),
            timestamp: 1704067200
        };

        let json = serde_json::to_string(&event).unwrap();
        let deserialized: HindsightSyncEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, deserialized);
    }

    #[test]
    fn test_hindsight_sync_event_resolution_promoted() {
        let event = HindsightSyncEvent {
            event_type: HindsightEventType::ResolutionPromoted {
                resolution_id: "res-456".to_string(),
                from_layer: MemoryLayer::User,
                to_layer: MemoryLayer::Team
            },
            tenant_id: "tenant-1".to_string(),
            timestamp: 1704067200
        };

        let json = serde_json::to_string(&event).unwrap();
        let deserialized: HindsightSyncEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, deserialized);
    }
}
