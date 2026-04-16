// Feature-gated at mod.rs — no #![cfg] here (would duplicate the gate).

use async_trait::async_trait;
use aws_config::BehaviorVersion;
use aws_sdk_bedrockruntime::Client;
use aws_sdk_bedrockruntime::config::Region;
use aws_sdk_bedrockruntime::types::{ContentBlock, ConversationRole, Message};
use mk_core::traits::LlmService;
use mk_core::types::{Policy, ValidationResult};
use tokio::sync::OnceCell;

pub struct BedrockLlmService {
    region: String,
    model_id: String,
    client: OnceCell<Client>,
}

impl BedrockLlmService {
    pub fn new(region: String, model_id: String) -> Self {
        Self {
            region,
            model_id,
            client: OnceCell::new(),
        }
    }

    async fn client(&self) -> anyhow::Result<&Client> {
        self.client
            .get_or_try_init(|| async {
                let config = aws_config::defaults(BehaviorVersion::latest())
                    .region(Region::new(self.region.clone()))
                    .load()
                    .await;
                Ok::<Client, anyhow::Error>(Client::new(&config))
            })
            .await
    }

    async fn generate_text(&self, prompt: &str) -> anyhow::Result<String> {
        let message = Message::builder()
            .role(ConversationRole::User)
            .content(ContentBlock::Text(prompt.to_string()))
            .build()
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        let response = self
            .client()
            .await?
            .converse()
            .model_id(&self.model_id)
            .messages(message)
            .send()
            .await?;

        let output = response
            .output()
            .ok_or_else(|| anyhow::anyhow!("Bedrock returned no output"))?;
        let message = output
            .as_message()
            .map_err(|_| anyhow::anyhow!("Bedrock converse output was not a message"))?;

        let text = message
            .content()
            .iter()
            .find_map(|block| block.as_text().ok())
            .map(ToOwned::to_owned)
            .ok_or_else(|| anyhow::anyhow!("Bedrock converse returned no text content"))?;

        Ok(text)
    }
}

#[async_trait]
impl LlmService for BedrockLlmService {
    type Error = Box<dyn std::error::Error + Send + Sync>;

    async fn generate(&self, prompt: &str) -> Result<String, Self::Error> {
        self.generate_text(prompt)
            .await
            .map_err(|e| Box::new(std::io::Error::other(e.to_string())) as Self::Error)
    }

    async fn analyze_drift(
        &self,
        content: &str,
        policies: &[Policy],
    ) -> Result<ValidationResult, Self::Error> {
        let policies_json =
            serde_json::to_string_pretty(policies).map_err(|e| Box::new(e) as Self::Error)?;
        let prompt = format!(
            "Analyze the following content against the provided governance policies.\nReturn a JSON object with 'isValid' (boolean) and 'violations' (array of PolicyViolation objects).\n\nContent:\n{}\n\nPolicies:\n{}",
            content, policies_json
        );

        let text = self
            .generate_text(&prompt)
            .await
            .map_err(|e| Box::new(std::io::Error::other(e.to_string())) as Self::Error)?;

        let result_json = if let Some(start) = text.find('{') {
            if let Some(end) = text.rfind('}') {
                &text[start..=end]
            } else {
                &text
            }
        } else {
            &text
        };

        serde_json::from_str(result_json).map_err(|e| Box::new(e) as Self::Error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructor_preserves_region_and_model() {
        let service = BedrockLlmService::new(
            "us-east-1".into(),
            "anthropic.claude-3-5-haiku-20241022-v1:0".into(),
        );

        assert_eq!(service.region, "us-east-1");
        assert_eq!(service.model_id, "anthropic.claude-3-5-haiku-20241022-v1:0");
    }
}
