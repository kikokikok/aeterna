use crate::bridge::{SyncNowTool, SyncStatusTool};
use crate::knowledge::{KnowledgeCheckTool, KnowledgeQueryTool, KnowledgeShowTool};
use crate::memory::{MemoryAddTool, MemoryDeleteTool, MemorySearchTool};
use crate::tools::ToolRegistry;
use memory::manager::MemoryManager;
use mk_core::traits::KnowledgeRepository;
use std::sync::Arc;
use sync::bridge::SyncManager;

pub struct McpServer {
    registry: ToolRegistry,
}

impl McpServer {
    pub fn new(
        memory_manager: Arc<MemoryManager>,
        sync_manager: Arc<SyncManager>,
        knowledge_repository: Arc<
            dyn KnowledgeRepository<Error = knowledge::repository::RepositoryError>,
        >,
    ) -> Self {
        let mut registry = ToolRegistry::new();

        registry.register(Box::new(MemoryAddTool::new(memory_manager.clone())));
        registry.register(Box::new(MemorySearchTool::new(memory_manager.clone())));
        registry.register(Box::new(MemoryDeleteTool::new(memory_manager)));

        registry.register(Box::new(KnowledgeQueryTool::new(
            knowledge_repository.clone(),
        )));
        registry.register(Box::new(KnowledgeShowTool::new(
            knowledge_repository.clone(),
        )));
        registry.register(Box::new(KnowledgeCheckTool::new()));

        registry.register(Box::new(SyncNowTool::new(sync_manager.clone())));
        registry.register(Box::new(SyncStatusTool::new(sync_manager)));

        Self { registry }
    }

    pub fn registry(&self) -> &ToolRegistry {
        &self.registry
    }
}
