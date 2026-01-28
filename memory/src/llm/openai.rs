use async_openai::types::chat::{
    ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
    CreateChatCompletionRequestArgs
};
use async_openai::types::responses::ResponseFormat;
use async_trait::async_trait;
use mk_core::traits::LlmService;
use mk_core::types::{Policy, ValidationResult};
use std::num::NonZeroUsize;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct OpenAILlmService {
    client: async_openai::Client<async_openai::config::OpenAIConfig>,
    model: String,
    cache: Arc<RwLock<lru::LruCache<String, String>>>
}

impl OpenAILlmService {
    pub fn new(api_key: String, model: String) -> Self {
        let config = async_openai::config::OpenAIConfig::new().with_api_key(api_key);
        let client = async_openai::Client::with_config(config);
        let cache = lru::LruCache::new(NonZeroUsize::new(100).unwrap());

        Self {
            client,
            model,
            cache: Arc::new(RwLock::new(cache))
        }
    }
}

#[async_trait]
impl LlmService for OpenAILlmService {
    type Error = anyhow::Error;

    async fn generate(&self, prompt: &str) -> Result<String, Self::Error> {
        {
            let mut cache = self.cache.write().await;
            if let Some(cached) = cache.get(prompt) {
                return Ok(cached.clone());
            }
        }

        let request = CreateChatCompletionRequestArgs::default()
            .model(&self.model)
            .messages([ChatCompletionRequestUserMessageArgs::default()
                .content(prompt)
                .build()?
                .into()])
            .build()?;

        let response = self.client.chat().create(request).await?;
        let content = response
            .choices
            .first()
            .and_then(|choice| choice.message.content.clone())
            .unwrap_or_default();

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

        let request = CreateChatCompletionRequestArgs::default()
            .model(&self.model)
            .messages([
                ChatCompletionRequestSystemMessageArgs::default()
                    .content(system_prompt)
                    .build()?
                    .into(),
                ChatCompletionRequestUserMessageArgs::default()
                    .content(&*prompt)
                    .build()?
                    .into()
            ])
            .response_format(ResponseFormat::JsonObject)
            .build()?;

        let response = self.client.chat().create(request).await?;
        let result_json = response
            .choices
            .first()
            .and_then(|choice| choice.message.content.clone())
            .ok_or_else(|| anyhow::anyhow!("Empty response from OpenAI"))?;

        let result: ValidationResult = serde_json::from_str(&result_json)?;
        Ok(result)
    }
}
