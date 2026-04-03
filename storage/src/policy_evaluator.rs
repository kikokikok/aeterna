use crate::repo_manager::{RepoRequest, Repository};
/// Code Search Policy Evaluator using Cedar Policy Engine
///
/// This service evaluates repository management operations against defined
/// Cedar policies to enforce governance and security.
///
/// `CachedPolicyEvaluator` wraps any `PolicyEvaluator` with a TTL-based
/// local cache so authorization decisions survive OPAL/Cedar agent crashes.
use async_trait::async_trait;
use cedar_policy::*;
use errors::CodeSearchError;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct PolicyContext {
    pub principal_id: String,
    pub principal_roles: Vec<String>,
    pub tenant_id: String,
}

#[async_trait]
pub trait PolicyEvaluator: Send + Sync {
    async fn evaluate_request(
        &self,
        context: &PolicyContext,
        action: &str,
        repo: &Repository,
    ) -> Result<bool, CodeSearchError>;

    async fn evaluate_approval(
        &self,
        context: &PolicyContext,
        request: &RepoRequest,
    ) -> Result<bool, CodeSearchError>;
}

pub struct CedarPolicyEvaluator {
    policies: PolicySet,
}

impl CedarPolicyEvaluator {
    pub fn new(policy_text: &str) -> Result<Self, CodeSearchError> {
        let policies =
            PolicySet::from_str(policy_text).map_err(|e| CodeSearchError::DatabaseError {
                reason: format!("Failed to parse policies: {}", e),
            })?;

        Ok(Self { policies })
    }

    fn build_entities(&self, context: &PolicyContext) -> Entities {
        // ... (build_entities implementation) ...
        // Create Role entities in Aeterna namespace
        let mut entity_vec = Vec::new();
        for role in &context.principal_roles {
            let role_uid = EntityUid::from_str(&format!("Aeterna::Role::\"{}\"", role)).unwrap();
            entity_vec.push(
                Entity::new(role_uid, [].into_iter().collect(), [].into_iter().collect())
                    .expect("Valid role entity"),
            );
        }

        // Create User entity with tenant_id attribute in Aeterna namespace
        let user_uid =
            EntityUid::from_str(&format!("Aeterna::User::\"{}\"", context.principal_id)).unwrap();
        let mut attrs = std::collections::HashMap::new();
        attrs.insert(
            "tenant_id".to_string(),
            RestrictedExpression::new_string(context.tenant_id.clone()),
        );
        attrs.insert(
            "role".to_string(),
            RestrictedExpression::new_string(
                context
                    .principal_roles
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "viewer".to_string()),
            ),
        );

        let mut parents = std::collections::HashSet::new();
        for role in &context.principal_roles {
            parents.insert(EntityUid::from_str(&format!("Aeterna::Role::\"{}\"", role)).unwrap());
        }

        entity_vec.push(Entity::new(user_uid, attrs, parents).expect("Valid user entity"));

        Entities::from_entities(entity_vec, None).expect("Valid entities set")
    }
}

#[async_trait]
impl PolicyEvaluator for CedarPolicyEvaluator {
    async fn evaluate_request(
        &self,
        context: &PolicyContext,
        action: &str,
        repo: &Repository,
    ) -> Result<bool, CodeSearchError> {
        let principal =
            EntityUid::from_str(&format!("Aeterna::User::\"{}\"", context.principal_id)).unwrap();
        let action_uid =
            EntityUid::from_str(&format!("CodeSearch::Action::\"{}\"", action)).unwrap();
        let resource =
            EntityUid::from_str(&format!("CodeSearch::Repository::\"{}\"", repo.id)).unwrap();

        let mut resource_attrs = std::collections::HashMap::new();
        resource_attrs.insert(
            "tenant_id".to_string(),
            RestrictedExpression::new_string(repo.tenant_id.clone()),
        );
        resource_attrs.insert(
            "name".to_string(),
            RestrictedExpression::new_string(repo.name.clone()),
        );
        resource_attrs.insert(
            "status".to_string(),
            RestrictedExpression::new_string("ready".to_string()),
        ); // Simulated status for check

        let request = Request::new(principal, action_uid, resource, Context::empty(), None)
            .map_err(
                |e: cedar_policy::RequestValidationError| CodeSearchError::DatabaseError {
                    reason: e.to_string(),
                },
            )?;

        let entities = self.build_entities(context);
        let authorizer = Authorizer::new();
        let decision = authorizer.is_authorized(&request, &self.policies, &entities);

        Ok(decision.decision() == Decision::Allow)
    }

    async fn evaluate_approval(
        &self,
        context: &PolicyContext,
        request: &RepoRequest,
    ) -> Result<bool, CodeSearchError> {
        let principal =
            EntityUid::from_str(&format!("Aeterna::User::\"{}\"", context.principal_id)).unwrap();
        let action_uid = EntityUid::from_str("CodeSearch::Action::\"ApproveRepository\"").unwrap();
        let resource =
            EntityUid::from_str(&format!("CodeSearch::Request::\"{}\"", request.id)).unwrap();

        let cedar_request = Request::new(principal, action_uid, resource, Context::empty(), None)
            .map_err(|e: cedar_policy::RequestValidationError| {
            CodeSearchError::DatabaseError {
                reason: e.to_string(),
            }
        })?;

        let entities = self.build_entities(context);
        let authorizer = Authorizer::new();
        let decision = authorizer.is_authorized(&cedar_request, &self.policies, &entities);

        Ok(decision.decision() == Decision::Allow)
    }
}

#[derive(Clone, Debug)]
struct CacheEntry {
    allowed: bool,
    expires_at: Instant,
}

impl CacheEntry {
    fn new(allowed: bool, ttl: Duration) -> Self {
        Self {
            allowed,
            expires_at: Instant::now() + ttl,
        }
    }

    fn is_valid(&self) -> bool {
        Instant::now() < self.expires_at
    }
}

fn cache_key(principal: &str, action: &str, resource: &impl std::fmt::Display) -> String {
    format!("{}::{}::{}", principal, action, resource)
}

pub struct CachedPolicyEvaluator {
    inner: Arc<dyn PolicyEvaluator>,
    cache: RwLock<HashMap<String, CacheEntry>>,
    ttl: Duration,
}

impl CachedPolicyEvaluator {
    pub fn new(inner: Arc<dyn PolicyEvaluator>, ttl: Duration) -> Self {
        Self {
            inner,
            cache: RwLock::new(HashMap::new()),
            ttl,
        }
    }

    fn get_cached(&self, key: &str) -> Option<bool> {
        let cache = self.cache.read();
        cache.get(key).filter(|e| e.is_valid()).map(|e| e.allowed)
    }

    fn set_cached(&self, key: String, allowed: bool) {
        let mut cache = self.cache.write();
        cache.insert(key, CacheEntry::new(allowed, self.ttl));
    }

    pub fn invalidate(&self) {
        let mut cache = self.cache.write();
        cache.clear();
    }

    pub fn evict_expired(&self) {
        let mut cache = self.cache.write();
        cache.retain(|_, entry| entry.is_valid());
    }
}

#[async_trait]
impl PolicyEvaluator for CachedPolicyEvaluator {
    async fn evaluate_request(
        &self,
        context: &PolicyContext,
        action: &str,
        repo: &Repository,
    ) -> Result<bool, CodeSearchError> {
        let key = cache_key(&context.principal_id, action, &repo.id);

        if let Some(cached) = self.get_cached(&key) {
            tracing::debug!(cache_hit = true, key = %key, "policy cache hit");
            return Ok(cached);
        }

        let result = self.inner.evaluate_request(context, action, repo).await;

        if let Ok(allowed) = &result {
            self.set_cached(key, *allowed);
        }

        result
    }

    async fn evaluate_approval(
        &self,
        context: &PolicyContext,
        request: &RepoRequest,
    ) -> Result<bool, CodeSearchError> {
        let key = cache_key(&context.principal_id, "ApproveRepository", &request.id);

        if let Some(cached) = self.get_cached(&key) {
            tracing::debug!(cache_hit = true, key = %key, "policy cache hit");
            return Ok(cached);
        }

        let result = self.inner.evaluate_approval(context, request).await;

        if let Ok(allowed) = &result {
            self.set_cached(key, *allowed);
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    // ---------------------------------------------------------------------------
    // Helpers
    // ---------------------------------------------------------------------------

    fn allow_all_policy() -> &'static str {
        r#"
permit(
    principal,
    action,
    resource
);
"#
    }

    fn deny_all_policy() -> &'static str {
        r#"
forbid(
    principal,
    action,
    resource
);
"#
    }

    fn allow_admin_only_policy() -> &'static str {
        r#"
permit(
    principal in Aeterna::Role::"admin",
    action,
    resource
);
"#
    }

    fn make_context(principal_id: &str, roles: &[&str], tenant_id: &str) -> PolicyContext {
        PolicyContext {
            principal_id: principal_id.to_string(),
            principal_roles: roles.iter().map(|s| s.to_string()).collect(),
            tenant_id: tenant_id.to_string(),
        }
    }

    fn make_repo(id: &str, tenant_id: &str, name: &str) -> crate::repo_manager::Repository {
        use chrono::Utc;
        use uuid::Uuid;
        crate::repo_manager::Repository {
            id: id.parse::<Uuid>().unwrap_or_else(|_| Uuid::new_v4()),
            tenant_id: tenant_id.to_string(),
            identity_id: None,
            name: name.to_string(),
            r#type: crate::repo_manager::RepositoryType::Remote,
            remote_url: None,
            local_path: None,
            current_branch: "main".to_string(),
            tracked_branches: vec![],
            sync_strategy: crate::repo_manager::SyncStrategy::Job,
            sync_interval_mins: None,
            status: crate::repo_manager::RepositoryStatus::Ready,
            last_indexed_commit: None,
            last_indexed_at: None,
            last_used_at: None,
            owner_id: None,
            shard_id: None,
            cold_storage_uri: None,
            config: serde_json::Value::Null,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn make_request(id: &str) -> crate::repo_manager::RepoRequest {
        use chrono::Utc;
        use uuid::Uuid;
        crate::repo_manager::RepoRequest {
            id: id.parse::<Uuid>().unwrap_or_else(|_| Uuid::new_v4()),
            repository_id: Uuid::new_v4(),
            requester_id: "user-1".to_string(),
            status: crate::repo_manager::RepoRequestStatus::Pending,
            policy_result: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    // ---------------------------------------------------------------------------
    // CedarPolicyEvaluator — construction
    // ---------------------------------------------------------------------------

    #[test]
    fn test_cedar_evaluator_bad_policy_returns_error() {
        let result = CedarPolicyEvaluator::new("this is not valid cedar syntax !!!");
        assert!(
            result.is_err(),
            "Expected parse error for invalid Cedar policy"
        );
    }

    #[test]
    fn test_cedar_evaluator_valid_policy_constructs_ok() {
        let result = CedarPolicyEvaluator::new(allow_all_policy());
        assert!(result.is_ok(), "Expected Ok for valid Cedar policy");
    }

    // ---------------------------------------------------------------------------
    // CedarPolicyEvaluator — evaluate_request
    // ---------------------------------------------------------------------------

    #[tokio::test]
    async fn test_evaluate_request_allow_all() {
        let evaluator = CedarPolicyEvaluator::new(allow_all_policy()).unwrap();
        let ctx = make_context("user-1", &["developer"], "tenant-a");
        let repo = make_repo(
            "00000000-0000-0000-0000-000000000001",
            "tenant-a",
            "my-repo",
        );

        let allowed = evaluator
            .evaluate_request(&ctx, "ReadCode", &repo)
            .await
            .unwrap();
        assert!(allowed, "allow-all policy should allow the request");
    }

    #[tokio::test]
    async fn test_evaluate_request_deny_all() {
        let evaluator = CedarPolicyEvaluator::new(deny_all_policy()).unwrap();
        let ctx = make_context("user-1", &["admin"], "tenant-a");
        let repo = make_repo(
            "00000000-0000-0000-0000-000000000002",
            "tenant-a",
            "my-repo",
        );

        let allowed = evaluator
            .evaluate_request(&ctx, "WriteCode", &repo)
            .await
            .unwrap();
        assert!(!allowed, "deny-all (forbid) policy should deny the request");
    }

    #[tokio::test]
    async fn test_evaluate_request_role_based_allow() {
        let evaluator = CedarPolicyEvaluator::new(allow_admin_only_policy()).unwrap();
        let admin_ctx = make_context("alice", &["admin"], "tenant-a");
        let dev_ctx = make_context("bob", &["developer"], "tenant-a");
        let repo = make_repo(
            "00000000-0000-0000-0000-000000000003",
            "tenant-a",
            "my-repo",
        );

        let admin_allowed = evaluator
            .evaluate_request(&admin_ctx, "ManageRepo", &repo)
            .await
            .unwrap();
        let dev_allowed = evaluator
            .evaluate_request(&dev_ctx, "ManageRepo", &repo)
            .await
            .unwrap();

        assert!(
            admin_allowed,
            "admin role should be allowed by admin-only policy"
        );
        assert!(
            !dev_allowed,
            "developer role should not be allowed by admin-only policy"
        );
    }

    // ---------------------------------------------------------------------------
    // CedarPolicyEvaluator — evaluate_approval
    // ---------------------------------------------------------------------------

    #[tokio::test]
    async fn test_evaluate_approval_allow_all() {
        let evaluator = CedarPolicyEvaluator::new(allow_all_policy()).unwrap();
        let ctx = make_context("user-1", &["developer"], "tenant-a");
        let request = make_request("00000000-0000-0000-0000-000000000010");

        let allowed = evaluator.evaluate_approval(&ctx, &request).await.unwrap();
        assert!(allowed, "allow-all policy should allow approval");
    }

    #[tokio::test]
    async fn test_evaluate_approval_deny_all() {
        let evaluator = CedarPolicyEvaluator::new(deny_all_policy()).unwrap();
        let ctx = make_context("user-1", &["admin"], "tenant-a");
        let request = make_request("00000000-0000-0000-0000-000000000011");

        let allowed = evaluator.evaluate_approval(&ctx, &request).await.unwrap();
        assert!(!allowed, "deny-all policy should deny approval");
    }

    // ---------------------------------------------------------------------------
    // CacheEntry
    // ---------------------------------------------------------------------------

    #[test]
    fn test_cache_entry_valid_before_expiry() {
        let entry = CacheEntry::new(true, Duration::from_secs(60));
        assert!(entry.is_valid(), "Entry should be valid before TTL expires");
        assert!(entry.allowed);
    }

    #[test]
    fn test_cache_entry_expired_immediately() {
        let entry = CacheEntry::new(false, Duration::from_millis(0));
        // Give the CPU a tiny bit so Instant::now() > expires_at
        std::thread::sleep(Duration::from_millis(5));
        assert!(
            !entry.is_valid(),
            "Entry with 0 TTL should immediately expire"
        );
        assert!(!entry.allowed);
    }

    // ---------------------------------------------------------------------------
    // cache_key
    // ---------------------------------------------------------------------------

    #[test]
    fn test_cache_key_format() {
        let key = cache_key("user-1", "ReadCode", &"repo-42");
        assert_eq!(key, "user-1::ReadCode::repo-42");
    }

    #[test]
    fn test_cache_key_unique_on_different_actions() {
        let k1 = cache_key("user-1", "Read", &"r");
        let k2 = cache_key("user-1", "Write", &"r");
        assert_ne!(k1, k2);
    }

    // ---------------------------------------------------------------------------
    // CachedPolicyEvaluator
    // ---------------------------------------------------------------------------

    #[tokio::test]
    async fn test_cached_evaluator_cache_miss_then_hit() {
        let inner = Arc::new(CedarPolicyEvaluator::new(allow_all_policy()).unwrap());
        let cached = CachedPolicyEvaluator::new(inner, Duration::from_secs(60));

        let ctx = make_context("user-1", &["developer"], "tenant-a");
        let repo = make_repo("00000000-0000-0000-0000-000000000020", "tenant-a", "repo");

        // First call: cache miss — should call inner and store result.
        let result1 = cached.evaluate_request(&ctx, "Read", &repo).await.unwrap();
        // Second call with same args: should be a cache hit.
        let result2 = cached.evaluate_request(&ctx, "Read", &repo).await.unwrap();

        assert_eq!(result1, result2);
        assert!(result1);
    }

    #[tokio::test]
    async fn test_cached_evaluator_invalidate_clears_cache() {
        let inner = Arc::new(CedarPolicyEvaluator::new(allow_all_policy()).unwrap());
        let cached = CachedPolicyEvaluator::new(inner, Duration::from_secs(60));

        let ctx = make_context("user-1", &["developer"], "tenant-a");
        let repo = make_repo("00000000-0000-0000-0000-000000000021", "tenant-a", "repo");

        // Populate cache.
        let _ = cached.evaluate_request(&ctx, "Read", &repo).await.unwrap();
        assert!(!cached.cache.read().is_empty(), "Cache should be populated");

        cached.invalidate();
        assert!(
            cached.cache.read().is_empty(),
            "Cache should be empty after invalidate()"
        );
    }

    #[tokio::test]
    async fn test_cached_evaluator_evict_expired_removes_stale_entries() {
        let inner = Arc::new(CedarPolicyEvaluator::new(allow_all_policy()).unwrap());
        let cached = CachedPolicyEvaluator::new(inner, Duration::from_millis(1));

        let ctx = make_context("user-1", &["developer"], "tenant-a");
        let repo = make_repo("00000000-0000-0000-0000-000000000022", "tenant-a", "repo");

        // Populate cache with a very short TTL entry.
        let _ = cached.evaluate_request(&ctx, "Read", &repo).await.unwrap();
        // Wait for TTL to expire.
        std::thread::sleep(Duration::from_millis(10));

        cached.evict_expired();
        assert!(
            cached.cache.read().is_empty(),
            "Expired entries should be removed by evict_expired()"
        );
    }

    #[tokio::test]
    async fn test_cached_evaluator_approval_hit() {
        let inner = Arc::new(CedarPolicyEvaluator::new(allow_all_policy()).unwrap());
        let cached = CachedPolicyEvaluator::new(inner, Duration::from_secs(60));

        let ctx = make_context("user-1", &["developer"], "tenant-a");
        let request = make_request("00000000-0000-0000-0000-000000000030");

        let r1 = cached.evaluate_approval(&ctx, &request).await.unwrap();
        let r2 = cached.evaluate_approval(&ctx, &request).await.unwrap();
        assert_eq!(r1, r2);
        assert!(r1);
    }
}
