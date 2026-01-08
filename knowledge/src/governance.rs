use mk_core::types::{KnowledgeLayer, Policy, PolicyViolation, ValidationResult};
use std::collections::HashMap;

pub struct GovernanceEngine {
    policies: HashMap<KnowledgeLayer, Vec<Policy>>,
}

impl GovernanceEngine {
    pub fn new() -> Self {
        Self {
            policies: HashMap::new(),
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

    pub fn validate(
        &self,
        target_layer: KnowledgeLayer,
        context: &HashMap<String, serde_json::Value>,
    ) -> ValidationResult {
        let mut violations = Vec::new();

        let layers = vec![
            KnowledgeLayer::Company,
            KnowledgeLayer::Org,
            KnowledgeLayer::Team,
            KnowledgeLayer::Project,
        ];

        for layer in layers {
            if let Some(layer_policies) = self.policies.get(&layer) {
                for policy in layer_policies {
                    for rule in &policy.rules {
                        if let Some(violation) = self.evaluate_rule(policy, rule, context) {
                            violations.push(violation);
                        }
                    }
                }
            }

            if layer == target_layer {
                break;
            }
        }

        ValidationResult {
            is_valid: violations.is_empty(),
            violations,
        }
    }

    fn evaluate_rule(
        &self,
        _policy: &Policy,
        _rule: &mk_core::types::PolicyRule,
        _context: &HashMap<String, serde_json::Value>,
    ) -> Option<PolicyViolation> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mk_core::types::{ConstraintOperator, ConstraintSeverity, ConstraintTarget, PolicyRule};

    #[test]
    fn test_governance_engine_hierarchy() {
        let mut engine = GovernanceEngine::new();

        let company_policy = Policy {
            id: "p1".to_string(),
            name: "Company Standards".to_string(),
            description: None,
            layer: KnowledgeLayer::Company,
            rules: vec![PolicyRule {
                id: "r1".to_string(),
                target: ConstraintTarget::Config,
                operator: ConstraintOperator::MustExist,
                value: serde_json::Value::Null,
                severity: ConstraintSeverity::Block,
                message: "Config must exist".to_string(),
            }],
            metadata: HashMap::new(),
        };

        engine.add_policy(company_policy);

        let context = HashMap::new();
        let result = engine.validate(KnowledgeLayer::Project, &context);

        assert!(result.is_valid);
    }
}
