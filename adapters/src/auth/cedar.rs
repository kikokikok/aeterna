use async_trait::async_trait;
use cedar_policy::*;
use mk_core::traits::AuthorizationService;
use mk_core::types::{Role, TenantContext, UserId};
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::RwLock;

#[derive(Error, Debug)]
pub enum CedarError {
    #[error("Cedar evaluation error: {0}")]
    Evaluation(String),
    #[error("Policy parsing error: {0}")]
    Parse(String),
    #[error("Schema error: {0}")]
    Schema(String),
    #[error("Entity fetch error: {0}")]
    EntityFetch(String),
}

struct CachedEntities {
    entities: Entities,
    fetched_at: Instant,
}

pub struct CedarAuthorizer {
    policies: PolicySet,
    entity_cache: Arc<RwLock<Option<CachedEntities>>>,
    opal_fetcher_url: Option<String>,
    cache_ttl: Duration,
    http_client: reqwest::Client,
}

impl CedarAuthorizer {
    pub fn new(policy_text: &str, _schema_text: &str) -> Result<Self, CedarError> {
        let policies =
            PolicySet::from_str(policy_text).map_err(|e| CedarError::Parse(e.to_string()))?;

        Ok(Self {
            policies,
            entity_cache: Arc::new(RwLock::new(None)),
            opal_fetcher_url: None,
            cache_ttl: Duration::from_secs(30),
            http_client: reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap_or_default(),
        })
    }

    pub fn with_opal_fetcher(mut self, url: String) -> Self {
        self.opal_fetcher_url = Some(url);
        self
    }

    pub fn with_cache_ttl(mut self, ttl: Duration) -> Self {
        self.cache_ttl = ttl;
        self
    }

    async fn get_entities(&self) -> Result<Entities, CedarError> {
        let opal_url = match &self.opal_fetcher_url {
            Some(url) => url,
            None => return Ok(Entities::empty()),
        };

        {
            let cache = self.entity_cache.read().await;
            if let Some(cached) = cache.as_ref() {
                if cached.fetched_at.elapsed() < self.cache_ttl {
                    return Ok(cached.entities.clone());
                }
            }
        }

        match self.fetch_entities_from_opal(opal_url).await {
            Ok(entities) => {
                let mut cache = self.entity_cache.write().await;
                *cache = Some(CachedEntities {
                    entities: entities.clone(),
                    fetched_at: Instant::now(),
                });
                Ok(entities)
            }
            Err(e) => {
                tracing::warn!(error = %e, "OPAL fetcher unreachable, using cached entities");
                let cache = self.entity_cache.read().await;
                match cache.as_ref() {
                    Some(cached) => Ok(cached.entities.clone()),
                    None => Err(CedarError::EntityFetch(format!(
                        "OPAL fetcher unreachable and no cached entities: {e}"
                    ))),
                }
            }
        }
    }

    async fn fetch_entities_from_opal(&self, base_url: &str) -> Result<Entities, CedarError> {
        let url = format!("{base_url}/v1/all");
        let resp = self
            .http_client
            .get(&url)
            .send()
            .await
            .map_err(|e| CedarError::EntityFetch(format!("HTTP request failed: {e}")))?;

        if !resp.status().is_success() {
            return Err(CedarError::EntityFetch(format!(
                "OPAL fetcher returned {}",
                resp.status()
            )));
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| CedarError::EntityFetch(format!("JSON parse failed: {e}")))?;

        let cedar_entities = body
            .get("entities")
            .and_then(|e| e.as_array())
            .cloned()
            .unwrap_or_default();

        let mut entity_vec = Vec::new();
        for entity_json in cedar_entities {
            let uid_obj = entity_json
                .get("uid")
                .ok_or_else(|| CedarError::EntityFetch("Entity missing uid".into()))?;
            let entity_type = uid_obj
                .get("type")
                .and_then(|t| t.as_str())
                .unwrap_or("Unknown");
            let entity_id = uid_obj
                .get("id")
                .and_then(|i| i.as_str())
                .unwrap_or("unknown");

            let uid_str = format!("{entity_type}::\"{entity_id}\"");
            let uid = EntityUid::from_str(&uid_str).map_err(|e| {
                CedarError::EntityFetch(format!("Invalid entity UID '{uid_str}': {e}"))
            })?;

            let parents_json = entity_json
                .get("parents")
                .and_then(|p| p.as_array())
                .cloned()
                .unwrap_or_default();

            let mut parents = std::collections::HashSet::new();
            for parent_json in parents_json {
                let p_type = parent_json
                    .get("type")
                    .and_then(|t| t.as_str())
                    .unwrap_or("Unknown");
                let p_id = parent_json
                    .get("id")
                    .and_then(|i| i.as_str())
                    .unwrap_or("unknown");
                let p_uid_str = format!("{p_type}::\"{p_id}\"");
                if let Ok(p_uid) = EntityUid::from_str(&p_uid_str) {
                    parents.insert(p_uid);
                }
            }

            let attrs_json = entity_json
                .get("attrs")
                .cloned()
                .unwrap_or(serde_json::json!({}));

            let attrs_map = attrs_json.as_object().cloned().unwrap_or_default();

            let mut attr_values = std::collections::HashMap::new();
            for (key, value) in attrs_map {
                if let Ok(rv) = RestrictedExpression::from_str(
                    &serde_json::to_string(&value).unwrap_or_default(),
                ) {
                    attr_values.insert(key, rv);
                }
            }

            let entity = Entity::new(uid, attr_values, parents)
                .map_err(|e| CedarError::EntityFetch(format!("Entity construction failed: {e}")))?;

            entity_vec.push(entity);
        }

        Entities::from_entities(entity_vec, None)
            .map_err(|e| CedarError::EntityFetch(format!("Entity collection failed: {e}")))
    }
}

#[async_trait]
impl AuthorizationService for CedarAuthorizer {
    type Error = CedarError;

    async fn check_permission(
        &self,
        ctx: &TenantContext,
        action: &str,
        resource: &str,
    ) -> Result<bool, Self::Error> {
        let entities = self.get_entities().await?;

        if let Some(agent_id) = &ctx.agent_id {
            let agent_principal = EntityUid::from_str(&format!("User::\"{}\"", agent_id))
                .map_err(|e| CedarError::Evaluation(e.to_string()))?;
            let delegate_action = EntityUid::from_str("Action::\"ActAs\"")
                .map_err(|e| CedarError::Evaluation(e.to_string()))?;
            let user_resource = EntityUid::from_str(&format!("User::\"{}\"", ctx.user_id.as_str()))
                .map_err(|e| CedarError::Evaluation(e.to_string()))?;

            let delegation_request = Request::new(
                agent_principal,
                delegate_action,
                user_resource,
                Context::empty(),
                None,
            )
            .map_err(|e| CedarError::Evaluation(e.to_string()))?;

            let authorizer = Authorizer::new();
            let delegation_answer =
                authorizer.is_authorized(&delegation_request, &self.policies, &entities);

            if delegation_answer.decision() != Decision::Allow {
                return Ok(false);
            }
        }

        let principal_str = format!("User::\"{}\"", ctx.user_id.as_str());

        let principal = EntityUid::from_str(&principal_str)
            .map_err(|e| CedarError::Evaluation(e.to_string()))?;
        let action_uid = EntityUid::from_str(&format!("Action::\"{}\"", action))
            .map_err(|e| CedarError::Evaluation(e.to_string()))?;
        let resource_uid =
            EntityUid::from_str(resource).map_err(|e| CedarError::Evaluation(e.to_string()))?;

        let request = Request::new(principal, action_uid, resource_uid, Context::empty(), None)
            .map_err(|e| CedarError::Evaluation(e.to_string()))?;

        let authorizer = Authorizer::new();
        let answer = authorizer.is_authorized(&request, &self.policies, &entities);

        Ok(answer.decision() == Decision::Allow)
    }

    async fn get_user_roles(&self, _ctx: &TenantContext) -> Result<Vec<Role>, Self::Error> {
        Ok(vec![])
    }

    async fn assign_role(
        &self,
        _ctx: &TenantContext,
        _user_id: &UserId,
        _role: Role,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn remove_role(
        &self,
        _ctx: &TenantContext,
        _user_id: &UserId,
        _role: Role,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mk_core::types::{TenantId, UserId};

    #[tokio::test]
    async fn test_cedar_authorization() -> Result<(), anyhow::Error> {
        let schema = r#"{
            "": {
                "entityTypes": {
                    "User": {},
                    "Unit": {}
                },
                "actions": {
                    "View": {
                        "appliesTo": {
                            "principalTypes": ["User"],
                            "resourceTypes": ["Unit"]
                        }
                    }
                }
            }
        }"#;

        let policies = r#"
            permit(principal == User::"u1", action == Action::"View", resource == Unit::"unit1");
        "#;

        let authorizer = CedarAuthorizer::new(policies, schema).map_err(|e| anyhow::anyhow!(e))?;

        let ctx = TenantContext::new(
            TenantId::new("t1".into()).unwrap(),
            UserId::new("u1".into()).unwrap(),
        );

        let allowed = authorizer
            .check_permission(&ctx, "View", "Unit::\"unit1\"")
            .await?;
        assert!(allowed);

        let denied = authorizer
            .check_permission(&ctx, "View", "Unit::\"unit2\"")
            .await?;
        assert!(!denied);

        Ok(())
    }

    #[tokio::test]
    async fn test_cedar_without_opal_uses_empty_entities() {
        let policies = r#"
            permit(principal == User::"u1", action == Action::"View", resource == Unit::"unit1");
        "#;

        let authorizer = CedarAuthorizer::new(policies, "{}").unwrap();
        let entities = authorizer.get_entities().await.unwrap();
        assert!(entities.iter().next().is_none());
    }

    #[tokio::test]
    async fn test_cedar_with_opal_url_fails_gracefully_no_cache() {
        let policies = r#"
            permit(principal, action, resource);
        "#;

        let authorizer = CedarAuthorizer::new(policies, "{}")
            .unwrap()
            .with_opal_fetcher("http://localhost:1/nonexistent".into());

        let result = authorizer.get_entities().await;
        assert!(result.is_err());
    }
}
