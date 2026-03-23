#![cfg(feature = "bedrock-provider")]

use async_trait::async_trait;
use aws_config::BehaviorVersion;
use aws_sdk_bedrockruntime::Client;
use aws_sdk_bedrockruntime::config::Region;
use aws_sdk_bedrockruntime::primitives::Blob;
use mk_core::traits::EmbeddingService;
use tokio::sync::OnceCell;

pub struct BedrockEmbeddingService {
    region: String,
    model_id: String,
    dimension: usize,
    client: OnceCell<Client>,
}

impl BedrockEmbeddingService {
    pub fn new(region: String, model_id: String) -> Self {
        let dimension = match model_id.as_str() {
            "amazon.titan-embed-text-v2:0" => 1024,
            "cohere.embed-english-v3" | "cohere.embed-multilingual-v3" => 1024,
            _ => 1024,
        };

        Self {
            region,
            model_id,
            dimension,
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

    fn request_body(&self, text: &str) -> anyhow::Result<Vec<u8>> {
        if self.model_id.starts_with("amazon.titan-embed") {
            Ok(serde_json::to_vec(&serde_json::json!({
                "inputText": text,
                "dimensions": self.dimension,
                "normalize": true,
            }))?)
        } else if self.model_id.starts_with("cohere.embed-") {
            Ok(serde_json::to_vec(&serde_json::json!({
                "texts": [text],
                "input_type": "search_document",
                "embedding_types": ["float"],
            }))?)
        } else {
            anyhow::bail!("Unsupported Bedrock embedding model: {}", self.model_id)
        }
    }

    fn extract_embedding(&self, payload: &serde_json::Value) -> anyhow::Result<Vec<f32>> {
        if self.model_id.starts_with("amazon.titan-embed") {
            serde_json::from_value(
                payload.get("embedding").cloned().ok_or_else(|| {
                    anyhow::anyhow!("Bedrock embedding response missing 'embedding'")
                })?,
            )
            .map_err(Into::into)
        } else if self.model_id.starts_with("cohere.embed-") {
            let embeddings = payload
                .get("embeddings")
                .and_then(|value| value.as_array())
                .ok_or_else(|| {
                    anyhow::anyhow!("Bedrock embedding response missing 'embeddings'")
                })?;

            serde_json::from_value(
                embeddings.first().cloned().ok_or_else(|| {
                    anyhow::anyhow!("Bedrock embedding response returned no vectors")
                })?,
            )
            .map_err(Into::into)
        } else {
            anyhow::bail!("Unsupported Bedrock embedding model: {}", self.model_id)
        }
    }
}

#[async_trait]
impl EmbeddingService for BedrockEmbeddingService {
    type Error = Box<dyn std::error::Error + Send + Sync>;

    async fn embed(&self, text: &str) -> Result<Vec<f32>, Self::Error> {
        let body = self
            .request_body(text)
            .map_err(|e| Box::new(std::io::Error::other(e.to_string())) as Self::Error)?;

        let response = self
            .client()
            .await
            .map_err(|e| Box::new(std::io::Error::other(e.to_string())) as Self::Error)?
            .invoke_model()
            .model_id(&self.model_id)
            .content_type("application/json")
            .accept("application/json")
            .body(Blob::new(body))
            .send()
            .await
            .map_err(|e| Box::new(std::io::Error::other(e.to_string())) as Self::Error)?;

        let payload: serde_json::Value = serde_json::from_slice(response.body().as_ref())
            .map_err(|e| Box::new(e) as Self::Error)?;

        self.extract_embedding(&payload)
            .map_err(|e| Box::new(std::io::Error::other(e.to_string())) as Self::Error)
    }

    fn dimension(&self) -> usize {
        self.dimension
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titan_request_shape_is_generated() {
        let service =
            BedrockEmbeddingService::new("us-east-1".into(), "amazon.titan-embed-text-v2:0".into());

        let body: serde_json::Value =
            serde_json::from_slice(&service.request_body("hello").unwrap()).unwrap();
        assert_eq!(body["inputText"], "hello");
        assert_eq!(body["dimensions"], 1024);
        assert_eq!(body["normalize"], true);
    }

    #[test]
    fn cohere_response_shape_is_adapted() {
        let service =
            BedrockEmbeddingService::new("us-east-1".into(), "cohere.embed-english-v3".into());

        let embedding = service
            .extract_embedding(&serde_json::json!({
                "embeddings": [[0.1, 0.2, 0.3]]
            }))
            .unwrap();

        assert_eq!(embedding, vec![0.1, 0.2, 0.3]);
    }
}
