use crate::telemetry::KnowledgeTelemetry;
use mk_core::traits::{EmbeddingService, EventPublisher, LlmService};
use mk_core::types::{
    ConstraintSeverity, GovernanceEvent, KnowledgeLayer, Policy, PolicyViolation, TenantContext,
    ValidationResult
};
use std::collections::HashMap;
use std::sync::Arc;
use storage::events::EventError;

pub struct GovernanceEngine {
    policies: HashMap<KnowledgeLayer, Vec<Policy>>,
    telemetry: KnowledgeTelemetry,
    storage:
        Option<Arc<dyn mk_core::traits::StorageBackend<Error = storage::postgres::PostgresError>>>,
    event_publisher: Option<Arc<dyn EventPublisher<Error = EventError>>>,
    embedding_service: Option<Arc<dyn EmbeddingService<Error = anyhow::Error>>>,
    llm_service: Option<Arc<dyn LlmService<Error = anyhow::Error>>>,
    knowledge_repository: Option<
        Arc<dyn mk_core::traits::KnowledgeRepository<Error = crate::repository::RepositoryError>>
    >
}

impl GovernanceEngine {
    pub fn new() -> Self {
        Self {
            policies: HashMap::new(),
            telemetry: KnowledgeTelemetry,
            storage: None,
            event_publisher: None,
            embedding_service: None,
            llm_service: None,
            knowledge_repository: None
        }
    }

    pub fn with_storage(
        mut self,
        storage: Arc<dyn mk_core::traits::StorageBackend<Error = storage::postgres::PostgresError>>
    ) -> Self {
        self.storage = Some(storage);
        self
    }

    pub fn with_event_publisher(
        mut self,
        publisher: Arc<dyn EventPublisher<Error = EventError>>
    ) -> Self {
        self.event_publisher = Some(publisher);
        self
    }

    pub fn with_embedding_service(
        mut self,
        embedding_service: Arc<dyn EmbeddingService<Error = anyhow::Error>>
    ) -> Self {
        self.embedding_service = Some(embedding_service);
        self
    }

    pub fn with_llm_service(
        mut self,
        llm_service: Arc<dyn LlmService<Error = anyhow::Error>>
    ) -> Self {
        self.llm_service = Some(llm_service);
        self
    }

    pub fn with_repository(
        mut self,
        repository: Arc<
            dyn mk_core::traits::KnowledgeRepository<Error = crate::repository::RepositoryError>
        >
    ) -> Self {
        self.knowledge_repository = Some(repository);
        self
    }

    pub fn storage(
        &self
    ) -> Option<Arc<dyn mk_core::traits::StorageBackend<Error = storage::postgres::PostgresError>>>
    {
        self.storage.clone()
    }

    pub fn llm_service(&self) -> Option<Arc<dyn LlmService<Error = anyhow::Error>>> {
        self.llm_service.clone()
    }

    pub fn repository(
        &self
    ) -> Option<
        Arc<dyn mk_core::traits::KnowledgeRepository<Error = crate::repository::RepositoryError>>
    > {
        self.knowledge_repository.clone()
    }
    pub async fn publish_event(&self, event: GovernanceEvent) -> Result<(), EventError> {
        if let Some(publisher) = &self.event_publisher {
            publisher.publish(event).await
        } else {
            Ok(())
        }
    }
}

impl Default for GovernanceEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl GovernanceEngine {
    pub fn add_policy(&mut self, policy: Policy) {
        self.policies.entry(policy.layer).or_default().push(policy);
    }

    pub fn event_publisher(&self) -> Option<Arc<dyn EventPublisher<Error = EventError>>> {
        self.event_publisher.clone()
    }

    pub fn validate(
        &self,
        target_layer: KnowledgeLayer,
        context: &HashMap<String, serde_json::Value>
    ) -> ValidationResult {
        let mut resolved_map: HashMap<String, Policy> = HashMap::new();
        let mut mandatory_policies: HashMap<String, KnowledgeLayer> = HashMap::new();

        let layers = [
            KnowledgeLayer::Company,
            KnowledgeLayer::Org,
            KnowledgeLayer::Team,
            KnowledgeLayer::Project
        ];

        for layer in &layers {
            if let Some(layer_policies) = self.policies.get(layer) {
                for policy in layer_policies {
                    let mut p = policy.clone();
                    p.layer = *layer;
                    self.merge_policy(&mut resolved_map, &mut mandatory_policies, p);
                }
            }
            if layer == &target_layer {
                break;
            }
        }

        let mut violations = Vec::new();
        let mut resolved_vec: Vec<Policy> = resolved_map.into_values().collect();
        resolved_vec.sort_by_key(|p| p.layer);

        for policy in resolved_vec {
            for rule in &policy.rules {
                if let Some(violation) = self.evaluate_rule(&policy, rule, context) {
                    self.telemetry.record_violation(
                        &format!("{:?}", policy.layer),
                        &format!("{:?}", rule.severity)
                    );
                    violations.push(violation);
                }
            }
        }

        ValidationResult {
            is_valid: violations.is_empty(),
            violations
        }
    }

    pub async fn validate_with_context(
        &self,
        target_layer: KnowledgeLayer,
        context: &HashMap<String, serde_json::Value>,
        tenant_ctx: Option<&TenantContext>
    ) -> ValidationResult {
        let mut violations = Vec::new();

        let active_policies = self
            .resolve_active_policies(target_layer, context, tenant_ctx)
            .await;

        for policy in active_policies {
            for rule in &policy.rules {
                if let Some(violation) = self.evaluate_rule(&policy, rule, context) {
                    self.telemetry.record_violation(
                        &format!("{:?}", policy.layer),
                        &format!("{:?}", rule.severity)
                    );
                    violations.push(violation);
                }
            }
        }

        if !violations.is_empty() {
            self.emit_drift_event(context, tenant_ctx, &violations)
                .await;
        }

        ValidationResult {
            is_valid: violations.is_empty(),
            violations
        }
    }

    async fn resolve_active_policies(
        &self,
        target_layer: KnowledgeLayer,
        context: &HashMap<String, serde_json::Value>,
        tenant_ctx: Option<&TenantContext>
    ) -> Vec<Policy> {
        let mut resolved_map: HashMap<String, Policy> = HashMap::new();
        let mut mandatory_policies: HashMap<String, KnowledgeLayer> = HashMap::new();

        let layers = [
            KnowledgeLayer::Company,
            KnowledgeLayer::Org,
            KnowledgeLayer::Team,
            KnowledgeLayer::Project
        ];

        for layer in &layers {
            if let Some(layer_policies) = self.policies.get(layer) {
                for policy in layer_policies {
                    self.merge_policy(&mut resolved_map, &mut mandatory_policies, policy.clone());
                }
            }
            if layer == &target_layer {
                break;
            }
        }

        if let Some(storage) = &self.storage {
            let unit_id = context
                .get("unitId")
                .or_else(|| context.get("projectId"))
                .and_then(|v| v.as_str());

            if let Some(uid) = unit_id {
                let ctx = tenant_ctx.cloned().unwrap_or_default();

                let mut units = Vec::new();
                if let Ok(mut ancestors) = storage.get_ancestors(ctx.clone(), uid).await {
                    units.append(&mut ancestors);
                }

                units.reverse();

                for unit in units {
                    if let Ok(unit_policies) =
                        storage.get_unit_policies(ctx.clone(), &unit.id).await
                    {
                        for policy in unit_policies {
                            self.merge_policy(&mut resolved_map, &mut mandatory_policies, policy);
                        }
                    }
                }

                if let Ok(unit_policies) = storage.get_unit_policies(ctx, uid).await {
                    for policy in unit_policies {
                        self.merge_policy(&mut resolved_map, &mut mandatory_policies, policy);
                    }
                }
            }
        }

        resolved_map.into_values().collect()
    }

    fn merge_policy(
        &self,
        resolved: &mut HashMap<String, Policy>,
        mandatory_policies: &mut HashMap<String, KnowledgeLayer>,
        incoming: Policy
    ) {
        use mk_core::types::{PolicyMode, RuleMergeStrategy};

        let policy_id = incoming.id.clone();

        if let Some(mandatory_layer) = mandatory_policies.get(&policy_id) {
            if incoming.layer != *mandatory_layer {
                return;
            }
        }

        if incoming.mode == PolicyMode::Mandatory {
            mandatory_policies.insert(policy_id.clone(), incoming.layer);
        }

        if let Some(existing) = resolved.get_mut(&policy_id) {
            match incoming.merge_strategy {
                RuleMergeStrategy::Override => {
                    *existing = incoming;
                }
                RuleMergeStrategy::Merge => {
                    for rule in incoming.rules {
                        if !existing.rules.iter().any(|r| r.id == rule.id) {
                            existing.rules.push(rule);
                        }
                    }
                    for (k, v) in incoming.metadata {
                        existing.metadata.insert(k, v);
                    }
                    existing.layer = incoming.layer;
                }
                RuleMergeStrategy::Intersect => {
                    existing
                        .rules
                        .retain(|r| incoming.rules.iter().any(|ir| ir.id == r.id));
                    existing.layer = incoming.layer;
                }
            }
        } else {
            resolved.insert(policy_id, incoming);
        }
    }

    pub async fn check_drift(
        &self,
        tenant_ctx: &TenantContext,
        _project_id: &str,
        context: &HashMap<String, serde_json::Value>
    ) -> Result<f32, anyhow::Error> {
        let mut violations = Vec::new();

        let content = context.get("content").and_then(|v| v.as_str());
        if let Some(c) = content {
            let mut semantic_violations = self.check_contradictions(tenant_ctx, c, 0.8).await?;
            violations.append(&mut semantic_violations);
        }

        let active_policies = self
            .resolve_active_policies(KnowledgeLayer::Project, context, Some(tenant_ctx))
            .await;

        let mandatory_policies_count = active_policies
            .iter()
            .filter(|p| p.mode == mk_core::types::PolicyMode::Mandatory)
            .count();

        if mandatory_policies_count == 0 {
            violations.push(PolicyViolation {
                rule_id: "missing_mandatory_policies".to_string(),
                policy_id: "governance_requirement".to_string(),
                severity: ConstraintSeverity::Warn,
                message: "No mandatory policies detected for this project layer".to_string(),
                context: context.clone()
            });
        }

        for policy in &active_policies {
            if let Some(expected_hash) =
                policy.metadata.get("version_hash").and_then(|v| v.as_str())
            {
                let actual_hash = context.get("version_hash").and_then(|v| v.as_str());
                if let Some(actual) = actual_hash {
                    if actual != expected_hash {
                        violations.push(PolicyViolation {
                            rule_id: "stale_policy_reference".to_string(),
                            policy_id: policy.id.clone(),
                            severity: ConstraintSeverity::Warn,
                            message: format!(
                                "Project uses stale policy version (expected: {}, actual: {})",
                                expected_hash, actual
                            ),
                            context: context.clone()
                        });
                    }
                }
            }
        }

        let drift_score = self.calculate_drift_score(&violations);

        if drift_score > 0.0 {
            self.emit_drift_event(context, Some(tenant_ctx), &violations)
                .await;
        }

        if let Some(storage) = &self.storage {
            let _ = storage
                .store_drift_result(mk_core::types::DriftResult {
                    project_id: _project_id.to_string(),
                    tenant_id: tenant_ctx.tenant_id.clone(),
                    drift_score,
                    violations: violations.clone(),
                    timestamp: chrono::Utc::now().timestamp()
                })
                .await;
        }

        Ok(drift_score)
    }

    fn calculate_drift_score(&self, violations: &[PolicyViolation]) -> f32 {
        if violations.is_empty() {
            return 0.0;
        }

        let score = violations
            .iter()
            .map(|v| match v.severity {
                ConstraintSeverity::Block => 1.0,
                ConstraintSeverity::Warn => 0.5,
                ConstraintSeverity::Info => 0.1
            })
            .sum::<f32>();

        score.min(1.0)
    }

    async fn emit_drift_event(
        &self,
        context: &HashMap<String, serde_json::Value>,
        tenant_ctx: Option<&TenantContext>,
        violations: &[PolicyViolation]
    ) {
        if let Some(publisher) = &self.event_publisher {
            let project_id = context
                .get("projectId")
                .and_then(|v| v.as_str())
                .or_else(|| context.get("unitId").and_then(|v| v.as_str()));

            if let Some(pid) = project_id {
                let drift_score = violations
                    .iter()
                    .map(|v| match v.severity {
                        mk_core::types::ConstraintSeverity::Block => 1.0,
                        mk_core::types::ConstraintSeverity::Warn => 0.5,
                        mk_core::types::ConstraintSeverity::Info => 0.1
                    })
                    .sum::<f32>();

                let _ = publisher
                    .publish(GovernanceEvent::DriftDetected {
                        project_id: pid.to_string(),
                        tenant_id: tenant_ctx.map(|c| c.tenant_id.clone()).unwrap_or_default(),
                        drift_score: drift_score.min(1.0),
                        timestamp: chrono::Utc::now().timestamp()
                    })
                    .await;
            }
        }
    }

    fn evaluate_rule(
        &self,
        policy: &Policy,
        rule: &mk_core::types::PolicyRule,
        context: &HashMap<String, serde_json::Value>
    ) -> Option<PolicyViolation> {
        use mk_core::types::{ConstraintOperator, RuleType};

        let target_key = match rule.target {
            mk_core::types::ConstraintTarget::File => "path",
            mk_core::types::ConstraintTarget::Code => "content",
            mk_core::types::ConstraintTarget::Dependency => "dependencies",
            mk_core::types::ConstraintTarget::Import => "imports",
            mk_core::types::ConstraintTarget::Config => "config"
        };

        let value = context.get(target_key);

        let is_condition_met = match rule.operator {
            ConstraintOperator::MustExist => value.is_some(),
            ConstraintOperator::MustNotExist => value.is_none(),
            ConstraintOperator::MustUse => {
                if let Some(v) = value {
                    if let Some(arr) = v.as_array() {
                        arr.contains(&rule.value)
                    } else {
                        v == &rule.value
                    }
                } else {
                    false
                }
            }
            ConstraintOperator::MustNotUse => {
                if let Some(v) = value {
                    if let Some(arr) = v.as_array() {
                        !arr.contains(&rule.value)
                    } else {
                        v != &rule.value
                    }
                } else {
                    true
                }
            }
            ConstraintOperator::MustMatch => {
                if let Some(v) = value {
                    if let Some(s) = v.as_str() {
                        if let Some(re_str) = rule.value.as_str() {
                            if let Ok(re) = regex::Regex::new(re_str) {
                                re.is_match(s)
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            ConstraintOperator::MustNotMatch => {
                if let Some(v) = value {
                    if let Some(s) = v.as_str() {
                        if let Some(re_str) = rule.value.as_str() {
                            if let Ok(re) = regex::Regex::new(re_str) {
                                !re.is_match(s)
                            } else {
                                true
                            }
                        } else {
                            true
                        }
                    } else {
                        true
                    }
                } else {
                    true
                }
            }
        };

        let is_violated = match rule.rule_type {
            RuleType::Allow => !is_condition_met,
            RuleType::Deny => is_condition_met
        };

        if is_violated {
            Some(PolicyViolation {
                rule_id: rule.id.clone(),
                policy_id: policy.id.clone(),
                severity: rule.severity,
                message: rule.message.clone(),
                context: context.clone()
            })
        } else {
            None
        }
    }

    pub async fn check_contradictions(
        &self,
        tenant_ctx: &TenantContext,
        content: &str,
        threshold: f32
    ) -> Result<Vec<PolicyViolation>, anyhow::Error> {
        let embedding_service = self
            .embedding_service
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Embedding service not configured"))?;

        let content_embedding = embedding_service.embed(content).await?;

        let mut violations = Vec::new();

        let mut context = HashMap::new();
        context.insert("content".to_string(), serde_json::json!(content));

        let active_policies = self
            .resolve_active_policies(KnowledgeLayer::Project, &context, Some(tenant_ctx))
            .await;

        for policy in active_policies {
            for rule in &policy.rules {
                if let Some(rule_embedding_val) =
                    policy.metadata.get(&format!("rule_{}_embedding", rule.id))
                {
                    if let Ok(rule_embedding) =
                        serde_json::from_value::<Vec<f32>>(rule_embedding_val.clone())
                    {
                        let similarity =
                            self.cosine_similarity(&content_embedding, &rule_embedding);
                        if similarity > threshold {
                            violations.push(PolicyViolation {
                                rule_id: rule.id.clone(),
                                policy_id: policy.id.clone(),
                                severity: rule.severity,
                                message: format!(
                                    "Semantic contradiction detected (similarity: {:.2}): {}",
                                    similarity, rule.message
                                ),
                                context: context.clone()
                            });
                        }
                    }
                }
            }
        }

        Ok(violations)
    }

    fn cosine_similarity(&self, v1: &[f32], v2: &[f32]) -> f32 {
        if v1.len() != v2.len() || v1.is_empty() {
            return 0.0;
        }
        let dot_product: f32 = v1.iter().zip(v2.iter()).map(|(a, b)| a * b).sum();
        let norm1: f32 = v1.iter().map(|a| a * a).sum::<f32>().sqrt();
        let norm2: f32 = v2.iter().map(|a| a * a).sum::<f32>().sqrt();

        if norm1 == 0.0 || norm2 == 0.0 {
            0.0
        } else {
            dot_product / (norm1 * norm2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mk_core::types::{ConstraintOperator, ConstraintSeverity, ConstraintTarget, PolicyRule};

    #[test]
    fn test_governance_engine_evaluation() {
        let mut engine = GovernanceEngine::new();

        let company_policy = Policy {
            id: "p1".to_string(),
            name: "Security Standards".to_string(),
            description: None,
            layer: KnowledgeLayer::Company,
            rules: vec![
                PolicyRule {
                    id: "r1".to_string(),
                    target: ConstraintTarget::Dependency,
                    operator: ConstraintOperator::MustNotUse,
                    value: serde_json::json!("unsafe-lib"),
                    severity: ConstraintSeverity::Block,
                    message: "unsafe-lib is banned".to_string(),
                    rule_type: mk_core::types::RuleType::Allow
                },
                PolicyRule {
                    id: "r2".to_string(),
                    target: ConstraintTarget::Code,
                    operator: ConstraintOperator::MustMatch,
                    value: serde_json::json!("^# ADR"),
                    severity: ConstraintSeverity::Warn,
                    message: "ADRs must start with # ADR".to_string(),
                    rule_type: mk_core::types::RuleType::Allow
                },
            ],
            metadata: HashMap::new(),
            mode: mk_core::types::PolicyMode::Optional,
            merge_strategy: mk_core::types::RuleMergeStrategy::Merge
        };

        engine.add_policy(company_policy);

        let mut context = HashMap::new();
        context.insert(
            "dependencies".to_string(),
            serde_json::json!(["safe-lib", "unsafe-lib"])
        );
        context.insert("content".to_string(), serde_json::json!("# ADR 001\n..."));

        let result = engine.validate(KnowledgeLayer::Project, &context);
        assert!(!result.is_valid);
        assert_eq!(result.violations.len(), 1);
        assert_eq!(result.violations[0].rule_id, "r1");

        let mut context = HashMap::new();
        context.insert("dependencies".to_string(), serde_json::json!(["safe-lib"]));
        context.insert("content".to_string(), serde_json::json!("ADR 001\n..."));

        let result = engine.validate(KnowledgeLayer::Project, &context);
        assert!(!result.is_valid);
        assert_eq!(result.violations.len(), 1);
        assert_eq!(result.violations[0].rule_id, "r2");

        let mut context = HashMap::new();
        context.insert("dependencies".to_string(), serde_json::json!(["safe-lib"]));
        context.insert("content".to_string(), serde_json::json!("# ADR 001\n..."));

        let result = engine.validate(KnowledgeLayer::Project, &context);
        assert!(result.is_valid);
    }
}
