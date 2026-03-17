pub mod governance;
pub mod knowledge;
pub mod memory;

use async_trait::async_trait;
use serde_json::Value;

use crate::auth::TenantContext;

#[async_trait]
pub trait Skill: Send + Sync {
    fn name(&self) -> &str;
    async fn invoke(
        &self,
        tool: &str,
        params: Value,
        tenant: &TenantContext,
    ) -> Result<Value, String>;
}

pub use governance::GovernanceSkill;
pub use knowledge::KnowledgeSkill;
pub use memory::MemorySkill;
