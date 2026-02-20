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
