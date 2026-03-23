#![cfg(feature = "google-provider")]

use async_trait::async_trait;
use gcp_auth::TokenProvider;
use mk_core::traits::LlmService;
use mk_core::types::{Policy, ValidationResult};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Serialize)]
struct GenerateContentRequest {
    contents: Vec<GoogleContent>,
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
struct GenerateContentResponse {
    #[serde(default)]
    candidates: Vec<GoogleCandidate>,
}

#[derive(Debug, Deserialize)]
struct GoogleCandidate {
    content: Option<GoogleResponseContent>,
}

#[derive(Debug, Deserialize)]
struct GoogleResponseContent {
    #[serde(default)]
    parts: Vec<GoogleResponsePart>,
}

#[derive(Debug, Deserialize)]
struct GoogleResponsePart {
    text: Option<String>,
}

struct CachedToken {
    token: String,
    expires_at: std::time::Instant,
}

type GoogleTokenProvider = Arc<dyn TokenProvider>;

pub struct GoogleLlmService {
    client: Client,
    project_id: String,
    location: String,
    model: String,
    base_url: String,
    access_token: Arc<RwLock<Option<CachedToken>>>,
    token_provider: Arc<tokio::sync::OnceCell<GoogleTokenProvider>>,
}

impl GoogleLlmService {
    pub fn new(project_id: String, location: String, model: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Google LLM HTTP client should build"),
            project_id,
            location,
            model,
            base_url: "https://aiplatform.googleapis.com".to_string(),
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
            "{}/v1/projects/{}/locations/{}/publishers/google/models/{}:generateContent",
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

    async fn generate_text(&self, prompt: &str) -> anyhow::Result<String> {
        let token = self.access_token().await?;
        let response = self
            .client
            .post(self.endpoint())
            .bearer_auth(token)
            .json(&GenerateContentRequest {
                contents: vec![GoogleContent {
                    role: "user".to_string(),
                    parts: vec![GooglePart {
                        text: prompt.to_string(),
                    }],
                }],
            })
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Google generateContent failed with {status}: {body}")
        }

        let payload: GenerateContentResponse = response.json().await?;
        let text = payload
            .candidates
            .first()
            .and_then(|candidate| candidate.content.as_ref())
            .and_then(|content| content.parts.first())
            .and_then(|part| part.text.clone())
            .ok_or_else(|| anyhow::anyhow!("Google generateContent returned no text"))?;

        Ok(text)
    }
}

#[async_trait]
impl LlmService for GoogleLlmService {
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
    use rustls::crypto::aws_lc_rs;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn install_crypto_provider() {
        let _ = aws_lc_rs::default_provider().install_default();
    }

    #[tokio::test]
    async fn prefers_explicit_google_access_token() {
        install_crypto_provider();
        let service =
            GoogleLlmService::new("proj".into(), "global".into(), "gemini-2.5-flash".into())
                .with_cached_access_token("test-token");

        let token = service.access_token().await.unwrap();
        assert_eq!(token, "test-token");
    }

    #[tokio::test]
    async fn adapts_generate_content_response() {
        install_crypto_provider();
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/projects/proj/locations/global/publishers/google/models/gemini-2.5-flash:generateContent"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "candidates": [{
                    "content": {
                        "parts": [{ "text": "hello from google" }]
                    }
                }]
            })))
            .mount(&server)
            .await;

        let service =
            GoogleLlmService::new("proj".into(), "global".into(), "gemini-2.5-flash".into())
                .with_base_url(server.uri())
                .with_cached_access_token("test-token");

        let text = service.generate("hi").await.unwrap();
        assert_eq!(text, "hello from google");
    }
}
