use async_trait::async_trait;
use mk_core::traits::LlmService;
use mk_core::types::{Policy, ValidationResult};
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct MockLlmService {
    responses: Arc<RwLock<std::collections::HashMap<String, String>>>
}

impl MockLlmService {
    pub fn new() -> Self {
        Self {
            responses: Arc::new(RwLock::new(std::collections::HashMap::new()))
        }
    }

    pub async fn add_response(&self, prompt: String, response: String) {
        let mut responses = self.responses.write().await;
        responses.insert(prompt, response);
    }

    pub async fn set_response(&mut self, response: &str) {
        let mut res = self.responses.write().await;
        res.insert("DEFAULT".to_string(), response.to_string());
    }
}

impl Default for MockLlmService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mk_core::types::{
        ConstraintOperator, ConstraintSeverity, ConstraintTarget, Policy, PolicyMode, PolicyRule,
        RuleMergeStrategy
    };

    #[tokio::test]
    async fn test_mock_llm_new() {
        let service = MockLlmService::new();
        let result = service.generate("test prompt").await.unwrap();
        assert!(result.contains("Mock response for: test prompt"));
    }

    #[tokio::test]
    async fn test_mock_llm_add_response() {
        let service = MockLlmService::new();
        service
            .add_response("hello".to_string(), "world".to_string())
            .await;

        let result = service.generate("hello").await.unwrap();
        assert_eq!(result, "world");
    }

    #[tokio::test]
    async fn test_mock_llm_set_response() {
        let mut service = MockLlmService::new();
        service.set_response("default response").await;

        let result = service.generate("any prompt").await.unwrap();
        assert_eq!(result, "default response");
    }

    #[tokio::test]
    async fn test_mock_llm_generate_default_fallback() {
        let service = MockLlmService::new();
        let result = service.generate("unknown").await.unwrap();
        assert!(result.contains("Mock response for:"));
    }

    #[tokio::test]
    async fn test_mock_llm_analyze_drift_no_violations() {
        let service = MockLlmService::new();
        let policies = vec![Policy {
            id: "policy1".to_string(),
            name: "Test Policy".to_string(),
            description: Some("A test policy".to_string()),
            layer: mk_core::types::KnowledgeLayer::Team,
            mode: PolicyMode::Mandatory,
            merge_strategy: RuleMergeStrategy::Merge,
            metadata: std::collections::HashMap::new(),
            rules: vec![PolicyRule {
                id: "rule1".to_string(),
                rule_type: mk_core::types::RuleType::Deny,
                target: ConstraintTarget::Code,
                operator: ConstraintOperator::MustNotUse,
                value: serde_json::json!("test"),
                severity: ConstraintSeverity::Warn,
                message: "Test rule".to_string()
            }]
        }];

        let result = service
            .analyze_drift("clean content", &policies)
            .await
            .unwrap();
        assert!(result.is_valid);
        assert!(result.violations.is_empty());
    }

    #[tokio::test]
    async fn test_mock_llm_analyze_drift_with_violation() {
        let service = MockLlmService::new();
        let policies = vec![Policy {
            id: "policy1".to_string(),
            name: "Test Policy".to_string(),
            description: Some("A test policy".to_string()),
            layer: mk_core::types::KnowledgeLayer::Team,
            mode: PolicyMode::Mandatory,
            merge_strategy: RuleMergeStrategy::Merge,
            metadata: std::collections::HashMap::new(),
            rules: vec![PolicyRule {
                id: "rule1".to_string(),
                rule_type: mk_core::types::RuleType::Deny,
                target: ConstraintTarget::Code,
                operator: ConstraintOperator::MustNotUse,
                value: serde_json::json!("test"),
                severity: ConstraintSeverity::Warn,
                message: "Test rule".to_string()
            }]
        }];

        let result = service
            .analyze_drift("content with violate:rule1", &policies)
            .await
            .unwrap();
        assert!(!result.is_valid);
        assert_eq!(result.violations.len(), 1);
        assert_eq!(result.violations[0].rule_id, "rule1");
        assert_eq!(result.violations[0].policy_id, "policy1");
    }

    #[tokio::test]
    async fn test_mock_llm_default() {
        let service = MockLlmService::default();
        let result = service.generate("test").await.unwrap();
        assert!(result.contains("Mock response for:"));
    }
}

#[async_trait]
impl LlmService for MockLlmService {
    type Error = Box<dyn std::error::Error + Send + Sync>;

    async fn generate(&self, prompt: &str) -> Result<String, Self::Error> {
        let responses = self.responses.read().await;
        if let Some(response) = responses.get(prompt) {
            Ok(response.clone())
        } else if let Some(response) = responses.get("DEFAULT") {
            Ok(response.clone())
        } else {
            Ok(format!("Mock response for: {}", prompt))
        }
    }

    async fn analyze_drift(
        &self,
        content: &str,
        policies: &[Policy]
    ) -> Result<ValidationResult, Self::Error> {
        let mut is_valid = true;
        let mut violations = Vec::new();

        for policy in policies {
            for rule in &policy.rules {
                if content.contains(&format!("violate:{}", rule.id)) {
                    is_valid = false;
                    violations.push(mk_core::types::PolicyViolation {
                        rule_id: rule.id.clone(),
                        policy_id: policy.id.clone(),
                        severity: rule.severity,
                        message: format!(
                            "Semantic violation of rule {}: {}",
                            rule.id, rule.message
                        ),
                        context: std::collections::HashMap::new()
                    });
                }
            }
        }

        Ok(ValidationResult {
            is_valid,
            violations
        })
    }
}
