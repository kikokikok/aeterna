use aisdk::core::LanguageModelRequest;
use aisdk::providers::openai::{Gpt4o, OpenAI};
use async_trait::async_trait;
use mk_core::traits::LlmService;
use mk_core::types::{Policy, ValidationResult};
use std::num::NonZeroUsize;
use std::sync::Arc;
use tokio::sync::RwLock;

pub enum AisdkModel {
    OpenAIGpt4o
}

pub struct AisdkLlmService {
    model: AisdkModel,
    api_key: Option<String>,
    cache: Arc<RwLock<lru::LruCache<String, String>>>
}

impl AisdkLlmService {
    pub fn new(api_key: Option<String>, model_name: &str) -> anyhow::Result<Self> {
        let model = match model_name {
            "gpt-4o" | "openai:gpt-4o" => AisdkModel::OpenAIGpt4o,
            _ => return Err(anyhow::anyhow!("Unsupported model: {}", model_name))
        };

        let cache = lru::LruCache::new(NonZeroUsize::new(100).unwrap());

        Ok(Self {
            model,
            api_key,
            cache: Arc::new(RwLock::new(cache))
        })
    }
}

#[async_trait]
impl LlmService for AisdkLlmService {
    type Error = anyhow::Error;

    async fn generate(&self, prompt: &str) -> Result<String, Self::Error> {
        {
            let mut cache = self.cache.write().await;
            if let Some(cached) = cache.get(prompt) {
                return Ok(cached.clone());
            }
        }

        let builder = match self.model {
            AisdkModel::OpenAIGpt4o => {
                let mut model_builder = OpenAI::<Gpt4o>::builder();
                if let Some(ref key) = self.api_key {
                    model_builder = model_builder.api_key(key);
                }
                let model = model_builder.build()?;
                LanguageModelRequest::builder().model(model)
            }
        };

        let response = builder.prompt(prompt).build().generate_text().await?;

        let content = response.text().clone().unwrap_or_default();

        {
            let mut cache = self.cache.write().await;
            cache.put(prompt.to_string(), content.clone());
        }

        Ok(content)
    }

    async fn analyze_drift(
        &self,
        content: &str,
        policies: &[Policy]
    ) -> Result<ValidationResult, Self::Error> {
        let policies_json = serde_json::to_string_pretty(policies)?;
        let prompt = format!(
            "Analyze the following content against the provided governance policies.\nReturn a \
             JSON object with 'isValid' (boolean) and 'violations' (array of PolicyViolation \
             objects).\nPolicyViolation schema: {{ 'ruleId': string, 'policyId': string, \
             'severity': 'info'|'warn'|'block', 'message': string, 'context': object \
             }}\n\nContent:\n{}\n\nPolicies:\n{}",
            content, policies_json
        );

        let system_prompt = "You are a governance analysis engine. You strictly evaluate content \
                             against policies and return structured JSON results.";

        let builder = match self.model {
            AisdkModel::OpenAIGpt4o => {
                let mut model_builder = OpenAI::<Gpt4o>::builder();
                if let Some(ref key) = self.api_key {
                    model_builder = model_builder.api_key(key);
                }
                let model = model_builder.build()?;
                LanguageModelRequest::builder().model(model)
            }
        };

        let full_prompt = format!("{}\n\n{}", system_prompt, prompt);

        let response = builder.prompt(&full_prompt).build().generate_text().await?;

        let text = response.text().clone().unwrap_or_default();

        let result_json = if let Some(start) = text.find('{') {
            if let Some(end) = text.rfind('}') {
                &text[start..=end]
            } else {
                &text
            }
        } else {
            &text
        };

        let result: ValidationResult = serde_json::from_str(result_json)?;
        Ok(result)
    }
}
