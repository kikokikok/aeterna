use crate::telemetry::KnowledgeTelemetry;
use mk_core::types::{KnowledgeLayer, Policy, PolicyViolation, TenantContext, ValidationResult};
use std::collections::HashMap;
use std::sync::Arc;

pub struct GovernanceEngine {
    policies: HashMap<KnowledgeLayer, Vec<Policy>>,
    telemetry: KnowledgeTelemetry,
    storage:
        Option<Arc<dyn mk_core::traits::StorageBackend<Error = storage::postgres::PostgresError>>>,
    event_tx: Option<tokio::sync::mpsc::UnboundedSender<mk_core::types::GovernanceEvent>>
}

impl GovernanceEngine {
    pub fn new() -> Self {
        Self {
            policies: HashMap::new(),
            telemetry: KnowledgeTelemetry,
            storage: None,
            event_tx: None
        }
    }

    pub fn with_storage(
        mut self,
        storage: Arc<dyn mk_core::traits::StorageBackend<Error = storage::postgres::PostgresError>>
    ) -> Self {
        self.storage = Some(storage);
        self
    }

    pub fn with_events(
        mut self,
        tx: tokio::sync::mpsc::UnboundedSender<mk_core::types::GovernanceEvent>
    ) -> Self {
        self.event_tx = Some(tx);
        self
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

    pub fn event_tx(
        &self
    ) -> Option<&tokio::sync::mpsc::UnboundedSender<mk_core::types::GovernanceEvent>> {
        self.event_tx.as_ref()
    }

    pub fn validate(
        &self,
        target_layer: KnowledgeLayer,
        context: &HashMap<String, serde_json::Value>
    ) -> ValidationResult {
        let mut violations = Vec::new();
        let layers = [
            KnowledgeLayer::Company,
            KnowledgeLayer::Org,
            KnowledgeLayer::Team,
            KnowledgeLayer::Project
        ];

        for layer in &layers {
            if let Some(layer_policies) = self.policies.get(layer) {
                for policy in layer_policies {
                    for rule in &policy.rules {
                        if let Some(violation) = self.evaluate_rule(policy, rule, context) {
                            self.telemetry.record_violation(
                                &format!("{:?}", layer),
                                &format!("{:?}", rule.severity)
                            );
                            violations.push(violation);
                        }
                    }
                }
            }
            if layer == &target_layer {
                break;
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
        _tenant_ctx: Option<&TenantContext>
    ) -> ValidationResult {
        let mut violations = Vec::new();
        let mut active_policies = Vec::new();

        let layers = [
            KnowledgeLayer::Company,
            KnowledgeLayer::Org,
            KnowledgeLayer::Team,
            KnowledgeLayer::Project
        ];

        for layer in &layers {
            if let Some(layer_policies) = self.policies.get(layer) {
                active_policies.extend(layer_policies.clone());
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
                let ctx = _tenant_ctx.cloned().unwrap_or_default();
                if let Ok(ancestors) = storage.get_ancestors(ctx.clone(), uid).await {
                    for ancestor in ancestors {
                        if let Ok(unit_policies) =
                            storage.get_unit_policies(ctx.clone(), &ancestor.id).await
                        {
                            active_policies.extend(unit_policies);
                        }
                    }
                }
                if let Ok(unit_policies) = storage.get_unit_policies(ctx, uid).await {
                    active_policies.extend(unit_policies);
                }
            }
        }

        for policy in &active_policies {
            for rule in &policy.rules {
                if let Some(violation) = self.evaluate_rule(policy, rule, context) {
                    self.telemetry.record_violation(
                        &format!("{:?}", policy.layer),
                        &format!("{:?}", rule.severity)
                    );
                    violations.push(violation);
                }
            }
        }

        if !violations.is_empty() {
            if let Some(tx) = &self.event_tx {
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

                    let _ = tx.send(mk_core::types::GovernanceEvent::DriftDetected {
                        project_id: pid.to_string(),
                        tenant_id: _tenant_ctx.map(|c| c.tenant_id.clone()).unwrap_or_default(),
                        drift_score: drift_score.min(1.0),
                        timestamp: chrono::Utc::now().timestamp()
                    });
                }
            }
        }

        ValidationResult {
            is_valid: violations.is_empty(),
            violations
        }
    }

    fn evaluate_rule(
        &self,
        policy: &Policy,
        rule: &mk_core::types::PolicyRule,
        context: &HashMap<String, serde_json::Value>
    ) -> Option<PolicyViolation> {
        use mk_core::types::ConstraintOperator;

        let target_key = match rule.target {
            mk_core::types::ConstraintTarget::File => "path",
            mk_core::types::ConstraintTarget::Code => "content",
            mk_core::types::ConstraintTarget::Dependency => "dependencies",
            mk_core::types::ConstraintTarget::Import => "imports",
            mk_core::types::ConstraintTarget::Config => "config"
        };

        let value = context.get(target_key);

        let is_violated = match rule.operator {
            ConstraintOperator::MustExist => value.is_none(),
            ConstraintOperator::MustNotExist => value.is_some(),
            ConstraintOperator::MustUse => {
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
            ConstraintOperator::MustNotUse => {
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
            ConstraintOperator::MustMatch => {
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
            ConstraintOperator::MustNotMatch => {
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
                    message: "unsafe-lib is banned".to_string()
                },
                PolicyRule {
                    id: "r2".to_string(),
                    target: ConstraintTarget::Code,
                    operator: ConstraintOperator::MustMatch,
                    value: serde_json::json!("^# ADR"),
                    severity: ConstraintSeverity::Warn,
                    message: "ADRs must start with # ADR".to_string()
                },
            ],
            metadata: HashMap::new()
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
