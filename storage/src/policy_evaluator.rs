/// Code Search Policy Evaluator using Cedar Policy Engine
///
/// This service evaluates repository management operations against defined
/// Cedar policies to enforce governance and security.

use async_trait::async_trait;
use cedar_policy::*;
use std::str::FromStr;
use errors::CodeSearchError;
use crate::repo_manager::{Repository, RepoRequest};

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
        let policies = PolicySet::from_str(policy_text)
            .map_err(|e| CodeSearchError::DatabaseError { reason: format!("Failed to parse policies: {}", e) })?;
        
        Ok(Self {
            policies,
        })
    }

    fn build_entities(&self, context: &PolicyContext) -> Entities {
        // ... (build_entities implementation) ...
        // Create Role entities in Aeterna namespace
        let mut entity_vec = Vec::new();
        for role in &context.principal_roles {
            let role_uid = EntityUid::from_str(&format!("Aeterna::Role::\"{}\"", role)).unwrap();
            entity_vec.push(Entity::new(
                role_uid,
                [].into_iter().collect(),
                [].into_iter().collect(),
            ).expect("Valid role entity"));
        }
        
        // Create User entity with tenant_id attribute in Aeterna namespace
        let user_uid = EntityUid::from_str(&format!("Aeterna::User::\"{}\"", context.principal_id)).unwrap();
        let mut attrs = std::collections::HashMap::new();
        attrs.insert("tenant_id".to_string(), RestrictedExpression::new_string(context.tenant_id.clone()));
        attrs.insert("role".to_string(), RestrictedExpression::new_string(context.principal_roles.first().cloned().unwrap_or_else(|| "viewer".to_string())));
        
        let mut parents = std::collections::HashSet::new();
        for role in &context.principal_roles {
            parents.insert(EntityUid::from_str(&format!("Aeterna::Role::\"{}\"", role)).unwrap());
        }

        entity_vec.push(Entity::new(
            user_uid,
            attrs,
            parents,
        ).expect("Valid user entity"));

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
        let principal = EntityUid::from_str(&format!("Aeterna::User::\"{}\"", context.principal_id)).unwrap();
        let action_uid = EntityUid::from_str(&format!("CodeSearch::Action::\"{}\"", action)).unwrap();
        let resource = EntityUid::from_str(&format!("CodeSearch::Repository::\"{}\"", repo.id)).unwrap();

        let mut resource_attrs = std::collections::HashMap::new();
        resource_attrs.insert("tenant_id".to_string(), RestrictedExpression::new_string(repo.tenant_id.clone()));
        resource_attrs.insert("name".to_string(), RestrictedExpression::new_string(repo.name.clone()));
        resource_attrs.insert("status".to_string(), RestrictedExpression::new_string("ready".to_string())); // Simulated status for check
        
        let request = Request::new(
            principal,
            action_uid,
            resource,
            Context::empty(),
            None,
        ).map_err(|e: cedar_policy::RequestValidationError| CodeSearchError::DatabaseError { reason: e.to_string() })?;

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
        let principal = EntityUid::from_str(&format!("Aeterna::User::\"{}\"", context.principal_id)).unwrap();
        let action_uid = EntityUid::from_str("CodeSearch::Action::\"ApproveRepository\"").unwrap();
        let resource = EntityUid::from_str(&format!("CodeSearch::Request::\"{}\"", request.id)).unwrap();

        let cedar_request = Request::new(
            principal,
            action_uid,
            resource,
            Context::empty(),
            None,
        ).map_err(|e: cedar_policy::RequestValidationError| CodeSearchError::DatabaseError { reason: e.to_string() })?;

        let entities = self.build_entities(context);
        let authorizer = Authorizer::new();
        let decision = authorizer.is_authorized(&cedar_request, &self.policies, &entities);

        Ok(decision.decision() == Decision::Allow)
    }
}
