use std::collections::HashMap;
use std::sync::Arc;

use mk_core::traits::KnowledgeRepository;
use mk_core::types::{
    KnowledgeEntry, KnowledgeLayer, KnowledgeStatus, KnowledgeType, TenantContext
};
use tracing::info_span;

use crate::governance::GovernanceEngine;
use crate::repository::RepositoryError;

#[derive(Debug, Clone)]
pub struct HindsightPromotionConfig {
    pub min_successes: u32,
    pub require_governance_check: bool,
    pub proposal_prefix: String
}

impl Default for HindsightPromotionConfig {
    fn default() -> Self {
        Self {
            min_successes: 3,
            require_governance_check: true,
            proposal_prefix: "proposals/hindsight/".to_string()
        }
    }
}

#[derive(Debug, Clone)]
pub struct HindsightPromotionRequest {
    pub note_id: String,
    pub target_layer: KnowledgeLayer,
    pub success_count: u32,
    pub note_content: String,
    pub tags: Vec<String>
}

#[derive(Debug, thiserror::Error)]
pub enum HindsightPromotionError {
    #[error("Success threshold not met")]
    ThresholdNotMet,

    #[error("Governance rejected promotion")]
    GovernanceRejected,

    #[error("Repository error: {0}")]
    Repository(#[from] RepositoryError)
}

pub struct HindsightPromoter {
    repository: Arc<dyn KnowledgeRepository<Error = RepositoryError>>,
    governance: Option<Arc<GovernanceEngine>>,
    config: HindsightPromotionConfig
}

impl HindsightPromoter {
    pub fn new(
        repository: Arc<dyn KnowledgeRepository<Error = RepositoryError>>,
        config: HindsightPromotionConfig
    ) -> Self {
        Self {
            repository,
            governance: None,
            config
        }
    }

    pub fn with_governance(mut self, governance: Arc<GovernanceEngine>) -> Self {
        self.governance = Some(governance);
        self
    }

    pub async fn promote(
        &self,
        ctx: TenantContext,
        request: HindsightPromotionRequest
    ) -> Result<String, HindsightPromotionError> {
        let _span = info_span!(
            "promote_hindsight_note",
            note_id = %request.note_id,
            target_layer = ?request.target_layer,
            success_count = request.success_count,
            min_successes = self.config.min_successes,
            has_governance = self.governance.is_some(),
            require_governance_check = self.config.require_governance_check
        )
        .entered();

        if request.success_count < self.config.min_successes {
            return Err(HindsightPromotionError::ThresholdNotMet);
        }

        if self.config.require_governance_check
            && let Some(engine) = &self.governance
        {
            let mut metadata = HashMap::new();
            metadata.insert("note_id".to_string(), serde_json::json!(request.note_id));
            metadata.insert(
                "success_count".to_string(),
                serde_json::json!(request.success_count)
            );
            metadata.insert("tags".to_string(), serde_json::json!(request.tags));
            let result = engine.validate(request.target_layer, &metadata);
            if !result.is_valid {
                return Err(HindsightPromotionError::GovernanceRejected);
            }
        }

        let path = format!("{}{}.md", self.config.proposal_prefix, request.note_id);
        let entry = KnowledgeEntry {
            path,
            content: request.note_content,
            layer: request.target_layer,
            kind: KnowledgeType::Hindsight,
            status: KnowledgeStatus::Proposed,
            summaries: HashMap::new(),
            metadata: HashMap::new(),
            commit_hash: None,
            author: None,
            updated_at: chrono::Utc::now().timestamp()
        };

        let message = format!("Propose hindsight promotion {}", request.note_id);
        let commit = self.repository.store(ctx, entry, &message).await?;
        Ok(commit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::Mutex;

    struct MockRepository {
        stored: Mutex<Vec<KnowledgeEntry>>
    }

    impl MockRepository {
        fn new() -> Self {
            Self {
                stored: Mutex::new(Vec::new())
            }
        }
    }

    #[async_trait]
    impl KnowledgeRepository for MockRepository {
        type Error = RepositoryError;

        async fn get(
            &self,
            _ctx: TenantContext,
            _layer: KnowledgeLayer,
            _path: &str
        ) -> Result<Option<KnowledgeEntry>, Self::Error> {
            Ok(None)
        }

        async fn store(
            &self,
            _ctx: TenantContext,
            entry: KnowledgeEntry,
            _message: &str
        ) -> Result<String, Self::Error> {
            self.stored.lock().unwrap().push(entry);
            Ok("commit".to_string())
        }

        async fn list(
            &self,
            _ctx: TenantContext,
            _layer: KnowledgeLayer,
            _prefix: &str
        ) -> Result<Vec<KnowledgeEntry>, Self::Error> {
            Ok(Vec::new())
        }

        async fn delete(
            &self,
            _ctx: TenantContext,
            _layer: KnowledgeLayer,
            _path: &str,
            _message: &str
        ) -> Result<String, Self::Error> {
            Ok(String::new())
        }

        async fn get_head_commit(
            &self,
            _ctx: TenantContext
        ) -> Result<Option<String>, Self::Error> {
            Ok(None)
        }

        async fn get_affected_items(
            &self,
            _ctx: TenantContext,
            _since_commit: &str
        ) -> Result<Vec<(KnowledgeLayer, String)>, Self::Error> {
            Ok(Vec::new())
        }

        async fn search(
            &self,
            _ctx: TenantContext,
            _query: &str,
            _layers: Vec<KnowledgeLayer>,
            _limit: usize
        ) -> Result<Vec<KnowledgeEntry>, Self::Error> {
            Ok(Vec::new())
        }

        fn root_path(&self) -> Option<std::path::PathBuf> {
            None
        }
    }

    fn ctx() -> TenantContext {
        TenantContext::new(
            mk_core::types::TenantId::new("t".to_string()).unwrap(),
            mk_core::types::UserId::new("u".to_string()).unwrap()
        )
    }

    #[tokio::test]
    async fn test_promote_threshold_not_met() {
        let repo = Arc::new(MockRepository::new());
        let promoter = HindsightPromoter::new(repo, HindsightPromotionConfig::default());
        let request = HindsightPromotionRequest {
            note_id: "n1".to_string(),
            target_layer: KnowledgeLayer::Project,
            success_count: 1,
            note_content: "content".to_string(),
            tags: vec![]
        };

        let result = promoter.promote(ctx(), request).await;
        assert!(matches!(
            result,
            Err(HindsightPromotionError::ThresholdNotMet)
        ));
    }

    #[tokio::test]
    async fn test_promote_creates_proposal() {
        let repo = Arc::new(MockRepository::new());
        let promoter = HindsightPromoter::new(repo.clone(), HindsightPromotionConfig::default());
        let request = HindsightPromotionRequest {
            note_id: "n1".to_string(),
            target_layer: KnowledgeLayer::Project,
            success_count: 5,
            note_content: "content".to_string(),
            tags: vec!["tag".to_string()]
        };

        let commit = promoter.promote(ctx(), request).await.unwrap();
        assert_eq!(commit, "commit");

        let stored = repo.stored.lock().unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].kind, KnowledgeType::Hindsight);
        assert_eq!(stored[0].status, KnowledgeStatus::Proposed);
        assert!(stored[0].path.starts_with("proposals/hindsight/"));
    }
}
