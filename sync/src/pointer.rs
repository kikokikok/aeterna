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
