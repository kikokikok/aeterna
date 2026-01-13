use async_trait::async_trait;
use mk_core::traits::LlmService;
use mk_core::types::{Policy, ValidationResult};
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct MockLlmService {
    responses: Arc<RwLock<std::collections::HashMap<String, String>>>,
}

impl MockLlmService {
    pub fn new() -> Self {
        Self {
            responses: Arc::new(RwLock::new(std::collections::HashMap::new())),
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
        policies: &[Policy],
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
                        context: std::collections::HashMap::new(),
                    });
                }
            }
        }

        Ok(ValidationResult {
            is_valid,
            violations,
        })
    }
}
