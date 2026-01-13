use mk_core::types::{KnowledgeLayer, KnowledgeType, MemoryLayer};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgePointer {
    pub source_type: KnowledgeType,
    pub source_id: String,
    pub content_hash: String,
    pub synced_at: i64,
    pub source_layer: KnowledgeLayer,
    pub is_orphaned: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgePointerMetadata {
    #[serde(rename = "type")]
    pub kind: String,
    pub knowledge_pointer: KnowledgePointer,
    pub tags: Vec<String>,
}

impl Default for KnowledgePointerMetadata {
    fn default() -> Self {
        Self {
            kind: "knowledge_pointer".to_string(),
            knowledge_pointer: KnowledgePointer {
                source_type: KnowledgeType::Adr,
                source_id: String::new(),
                content_hash: String::new(),
                synced_at: 0,
                source_layer: KnowledgeLayer::Company,
                is_orphaned: false,
            },
            tags: Vec::new(),
        }
    }
}

pub fn map_layer(knowledge_layer: KnowledgeLayer) -> MemoryLayer {
    match knowledge_layer {
        KnowledgeLayer::Company => MemoryLayer::Company,
        KnowledgeLayer::Org => MemoryLayer::Org,
        KnowledgeLayer::Team => MemoryLayer::Team,
        KnowledgeLayer::Project => MemoryLayer::Project,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_knowledge_pointer_metadata_default() {
        let metadata = KnowledgePointerMetadata::default();

        assert_eq!(metadata.kind, "knowledge_pointer");
        assert_eq!(metadata.knowledge_pointer.source_type, KnowledgeType::Adr);
        assert!(metadata.knowledge_pointer.source_id.is_empty());
        assert!(metadata.knowledge_pointer.content_hash.is_empty());
        assert_eq!(metadata.knowledge_pointer.synced_at, 0);
        assert_eq!(
            metadata.knowledge_pointer.source_layer,
            KnowledgeLayer::Company
        );
        assert!(!metadata.knowledge_pointer.is_orphaned);
        assert!(metadata.tags.is_empty());
    }

    #[test]
    fn test_map_layer_company() {
        assert_eq!(map_layer(KnowledgeLayer::Company), MemoryLayer::Company);
    }

    #[test]
    fn test_map_layer_org() {
        assert_eq!(map_layer(KnowledgeLayer::Org), MemoryLayer::Org);
    }

    #[test]
    fn test_map_layer_team() {
        assert_eq!(map_layer(KnowledgeLayer::Team), MemoryLayer::Team);
    }

    #[test]
    fn test_map_layer_project() {
        assert_eq!(map_layer(KnowledgeLayer::Project), MemoryLayer::Project);
    }
}
