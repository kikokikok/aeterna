// Feature-gated at mod.rs — no #![cfg] here (would duplicate the gate).

use async_trait::async_trait;
use gcp_auth::TokenProvider;
use mk_core::traits::EmbeddingService;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Serialize)]
struct EmbedContentRequest {
    content: GoogleContent,
    task_type: String,
}

#[derive(Debug, Serialize)]
struct GoogleContent {
    role: String,
    parts: Vec<GooglePart>,
}

#[derive(Debug, Serialize)]
struct GooglePart {
    text: String,
}

#[derive(Debug, Deserialize)]
struct EmbedContentResponse {
    embedding: Option<GoogleEmbedding>,
}

#[derive(Debug, Deserialize)]
struct GoogleEmbedding {
    #[serde(default)]
    values: Vec<f32>,
}

struct CachedToken {
    token: String,
    expires_at: std::time::Instant,
}

type GoogleTokenProvider = Arc<dyn TokenProvider>;

pub struct GoogleEmbeddingService {
    client: Client,
    project_id: String,
    location: String,
    model: String,
    base_url: String,
    dimension: usize,
    access_token: Arc<RwLock<Option<CachedToken>>>,
    token_provider: Arc<tokio::sync::OnceCell<GoogleTokenProvider>>,
}

impl GoogleEmbeddingService {
    pub fn new(project_id: String, location: String, model: String) -> Self {
        let dimension = match model.as_str() {
            "text-embedding-005" | "text-multilingual-embedding-002" => 768,
            _ => 768,
        };

        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Google embedding HTTP client should build"),
            project_id,
            location,
            model,
            base_url: "https://aiplatform.googleapis.com".to_string(),
            dimension,
            access_token: Arc::new(RwLock::new(None)),
            token_provider: Arc::new(tokio::sync::OnceCell::new()),
        }
    }

    #[cfg(test)]
    fn with_base_url(mut self, base_url: String) -> Self {
        self.base_url = base_url;
        self
    }

    #[cfg(test)]
    fn with_cached_access_token(mut self, token: impl Into<String>) -> Self {
        self.access_token = Arc::new(RwLock::new(Some(CachedToken {
            token: token.into(),
            expires_at: std::time::Instant::now() + std::time::Duration::from_secs(3600),
        })));
        self
    }

    fn endpoint(&self) -> String {
        format!(
            "{}/v1/projects/{}/locations/{}/publishers/google/models/{}:embedContent",
            self.base_url, self.project_id, self.location, self.model
        )
    }

    async fn access_token(&self) -> anyhow::Result<String> {
        {
            let cached = self.access_token.read().await;
            if let Some(token) = cached.as_ref()
                && token.expires_at > std::time::Instant::now()
            {
                return Ok(token.token.clone());
            }
        }

        let token = if let Ok(token) = std::env::var("GOOGLE_ACCESS_TOKEN") {
            token
        } else {
            self.fetch_adc_token().await?
        };

        {
            let mut cached = self.access_token.write().await;
            *cached = Some(CachedToken {
                token: token.clone(),
                expires_at: std::time::Instant::now() + std::time::Duration::from_secs(3500),
            });
        }

        Ok(token)
    }

    async fn token_provider(&self) -> anyhow::Result<&GoogleTokenProvider> {
        self.token_provider
            .get_or_try_init(|| async {
                let provider = gcp_auth::provider().await.map_err(|e| {
                    anyhow::anyhow!("failed to initialize Google ADC token provider: {e}")
                })?;
                Ok::<GoogleTokenProvider, anyhow::Error>(provider)
            })
            .await
    }

    async fn fetch_adc_token(&self) -> anyhow::Result<String> {
        let provider = self.token_provider().await?;
        let token = provider
            .token(&["https://www.googleapis.com/auth/cloud-platform"])
            .await
            .map_err(|e| anyhow::anyhow!("Google access token not available from ADC: {e}"))?;
        Ok(token.as_str().to_string())
    }
}

#[async_trait]
impl EmbeddingService for GoogleEmbeddingService {
    type Error = Box<dyn std::error::Error + Send + Sync>;

    async fn embed(&self, text: &str) -> Result<Vec<f32>, Self::Error> {
        let token = self
            .access_token()
            .await
            .map_err(|e| Box::new(std::io::Error::other(e.to_string())) as Self::Error)?;

        let response = self
            .client
            .post(self.endpoint())
            .bearer_auth(token)
            .json(&EmbedContentRequest {
                content: GoogleContent {
                    role: "user".to_string(),
                    parts: vec![GooglePart {
                        text: text.to_string(),
                    }],
                },
                task_type: "RETRIEVAL_DOCUMENT".to_string(),
            })
            .send()
            .await
            .map_err(|e| Box::new(e) as Self::Error)?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Box::new(std::io::Error::other(format!(
                "Google embedContent failed with {status}: {body}"
            ))));
        }

        let payload: EmbedContentResponse = response
            .json()
            .await
            .map_err(|e| Box::new(e) as Self::Error)?;
        payload
            .embedding
            .map(|embedding| embedding.values)
            .ok_or_else(|| {
                Box::new(std::io::Error::other(
                    "Google embedContent returned no embedding",
                )) as Self::Error
            })
    }

    fn dimension(&self) -> usize {
        self.dimension
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustls::crypto::aws_lc_rs;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn install_crypto_provider() {
        let _ = aws_lc_rs::default_provider().install_default();
    }

    #[tokio::test]
    async fn prefers_explicit_google_access_token() {
        install_crypto_provider();
        let service = GoogleEmbeddingService::new(
            "proj".into(),
            "global".into(),
            "text-embedding-005".into(),
        )
        .with_cached_access_token("test-token");

        let token = service.access_token().await.unwrap();
        assert_eq!(token, "test-token");
    }

    #[tokio::test]
    async fn adapts_embed_content_response() {
        install_crypto_provider();
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/projects/proj/locations/global/publishers/google/models/text-embedding-005:embedContent"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "embedding": {
                    "values": [0.1, 0.2, 0.3]
                }
            })))
            .mount(&server)
            .await;

        let service = GoogleEmbeddingService::new(
            "proj".into(),
            "global".into(),
            "text-embedding-005".into(),
        )
        .with_base_url(server.uri())
        .with_cached_access_token("test-token");

        let vector = service.embed("hello").await.unwrap();
        assert_eq!(vector, vec![0.1, 0.2, 0.3]);
        assert_eq!(service.dimension(), 768);
    }
}
