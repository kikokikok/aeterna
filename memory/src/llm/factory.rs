#[cfg(feature = "embedding-integration")]
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use mk_core::traits::LlmService;

#[cfg(feature = "embedding-integration")]
struct AnyhowLlmAdapter<T> {
    inner: T,
}

#[cfg(feature = "embedding-integration")]
#[async_trait]
impl<T> LlmService for AnyhowLlmAdapter<T>
where
    T: LlmService<Error = anyhow::Error> + Send + Sync,
{
    type Error = Box<dyn std::error::Error + Send + Sync>;

    async fn generate(&self, prompt: &str) -> Result<String, Self::Error> {
        self.inner
            .generate(prompt)
            .await
            .map_err(|e| -> Self::Error { Box::new(std::io::Error::other(e.to_string())) })
    }

    async fn analyze_drift(
        &self,
        content: &str,
        policies: &[mk_core::types::Policy],
    ) -> Result<mk_core::types::ValidationResult, Self::Error> {
        self.inner
            .analyze_drift(content, policies)
            .await
            .map_err(|e| -> Self::Error { Box::new(std::io::Error::other(e.to_string())) })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LlmFactoryError {
    #[error("LLM provider configuration error: {0}")]
    Configuration(String),
    #[error("LLM provider unavailable in this build: {0}")]
    Unavailable(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LlmProviderType {
    Openai,
    Google,
    Bedrock,
    #[default]
    None,
}

impl std::fmt::Display for LlmProviderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LlmProviderType::Openai => write!(f, "openai"),
            LlmProviderType::Google => write!(f, "google"),
            LlmProviderType::Bedrock => write!(f, "bedrock"),
            LlmProviderType::None => write!(f, "none"),
        }
    }
}

impl std::str::FromStr for LlmProviderType {
    type Err = LlmFactoryError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "openai" => Ok(LlmProviderType::Openai),
            "google" | "vertex" | "vertex_ai" | "vertexai" | "gemini" => {
                Ok(LlmProviderType::Google)
            }
            "bedrock" | "aws_bedrock" | "aws-bedrock" => Ok(LlmProviderType::Bedrock),
            "none" => Ok(LlmProviderType::None),
            _ => Err(LlmFactoryError::Configuration(format!(
                "Unknown LLM provider: {s}. Valid options: openai, google, bedrock, none"
            ))),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct OpenAiLlmConfig {
    pub model: String,
    pub api_key: String,
    /// Optional override for the OpenAI-compatible API base URL.
    ///
    /// When set (via `AETERNA_OPENAI_BASE_URL`), requests are routed there
    /// instead of `https://api.openai.com/v1`. Used by the e2e suite to
    /// target ollama, GitHub Models, or a recorded-fixture replay server,
    /// and by self-hosted users running a local OpenAI-compat gateway.
    pub base_url: Option<String>,
}

impl OpenAiLlmConfig {
    pub fn from_env() -> Result<Self, LlmFactoryError> {
        Ok(Self {
            model: std::env::var("AETERNA_OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o".to_string()),
            api_key: std::env::var("OPENAI_API_KEY")
                .map_err(|_| LlmFactoryError::Configuration("OPENAI_API_KEY not set".into()))?,
            base_url: std::env::var("AETERNA_OPENAI_BASE_URL").ok().filter(|s| !s.is_empty()),
        })
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct GoogleLlmConfig {
    pub project_id: String,
    pub location: String,
    pub model: String,
}

impl GoogleLlmConfig {
    pub fn from_env() -> Result<Self, LlmFactoryError> {
        Ok(Self {
            project_id: std::env::var("AETERNA_GOOGLE_PROJECT_ID").map_err(|_| {
                LlmFactoryError::Configuration("AETERNA_GOOGLE_PROJECT_ID not set".into())
            })?,
            location: std::env::var("AETERNA_GOOGLE_LOCATION").map_err(|_| {
                LlmFactoryError::Configuration("AETERNA_GOOGLE_LOCATION not set".into())
            })?,
            model: std::env::var("AETERNA_GOOGLE_MODEL").map_err(|_| {
                LlmFactoryError::Configuration("AETERNA_GOOGLE_MODEL not set".into())
            })?,
        })
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct BedrockLlmConfig {
    pub region: String,
    pub model_id: String,
}

impl BedrockLlmConfig {
    pub fn from_env() -> Result<Self, LlmFactoryError> {
        Ok(Self {
            region: std::env::var("AETERNA_BEDROCK_REGION").map_err(|_| {
                LlmFactoryError::Configuration("AETERNA_BEDROCK_REGION not set".into())
            })?,
            model_id: std::env::var("AETERNA_BEDROCK_MODEL").map_err(|_| {
                LlmFactoryError::Configuration("AETERNA_BEDROCK_MODEL not set".into())
            })?,
        })
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct LlmProviderConfig {
    pub provider_type: LlmProviderType,
    pub openai: Option<OpenAiLlmConfig>,
    pub google: Option<GoogleLlmConfig>,
    pub bedrock: Option<BedrockLlmConfig>,
}

impl LlmProviderConfig {
    pub fn from_env() -> Result<Self, LlmFactoryError> {
        let provider_type = std::env::var("AETERNA_LLM_PROVIDER")
            .unwrap_or_else(|_| "none".to_string())
            .parse()?;

        Ok(Self {
            provider_type,
            openai: matches!(provider_type, LlmProviderType::Openai)
                .then(OpenAiLlmConfig::from_env)
                .transpose()?,
            google: matches!(provider_type, LlmProviderType::Google)
                .then(GoogleLlmConfig::from_env)
                .transpose()?,
            bedrock: matches!(provider_type, LlmProviderType::Bedrock)
                .then(BedrockLlmConfig::from_env)
                .transpose()?,
        })
    }
}

pub fn create_llm_service_from_env() -> Result<
    Option<Arc<dyn LlmService<Error = Box<dyn std::error::Error + Send + Sync>> + Send + Sync>>,
    LlmFactoryError,
> {
    let config = LlmProviderConfig::from_env()?;
    create_llm_service(config)
}

pub fn create_llm_service(
    config: LlmProviderConfig,
) -> Result<
    Option<Arc<dyn LlmService<Error = Box<dyn std::error::Error + Send + Sync>> + Send + Sync>>,
    LlmFactoryError,
> {
    match config.provider_type {
        LlmProviderType::None => Ok(None),
        LlmProviderType::Openai => {
            #[allow(unused_variables)]
            let openai = config.openai.ok_or_else(|| {
                LlmFactoryError::Configuration("OpenAI LLM config missing".into())
            })?;

            #[cfg(feature = "embedding-integration")]
            {
                let service = AnyhowLlmAdapter {
                    inner: super::openai::OpenAILlmService::new(
                        openai.api_key,
                        openai.model,
                        openai.base_url,
                    ),
                };
                Ok(Some(Arc::new(service)))
            }

            #[cfg(not(feature = "embedding-integration"))]
            {
                return Err(LlmFactoryError::Unavailable(
                    "openai support requires the embedding-integration feature".into(),
                ));
            }
        }
        LlmProviderType::Google => {
            let google = config.google.ok_or_else(|| {
                LlmFactoryError::Configuration("Google LLM config missing".into())
            })?;

            #[cfg(feature = "google-provider")]
            {
                let service = super::google::GoogleLlmService::new(
                    google.project_id,
                    google.location,
                    google.model,
                );
                Ok(Some(Arc::new(service)))
            }

            #[cfg(not(feature = "google-provider"))]
            {
                let _ = google;
                return Err(LlmFactoryError::Unavailable(
                    "google support requires the google-provider feature".into(),
                ));
            }
        }
        LlmProviderType::Bedrock => {
            let bedrock = config.bedrock.ok_or_else(|| {
                LlmFactoryError::Configuration("Bedrock LLM config missing".into())
            })?;

            #[cfg(feature = "bedrock-provider")]
            {
                let service =
                    super::bedrock::BedrockLlmService::new(bedrock.region, bedrock.model_id);
                Ok(Some(Arc::new(service)))
            }

            #[cfg(not(feature = "bedrock-provider"))]
            {
                let _ = bedrock;
                return Err(LlmFactoryError::Unavailable(
                    "bedrock support requires the bedrock-provider feature".into(),
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_provider_aliases() {
        assert_eq!(
            "openai".parse::<LlmProviderType>().unwrap(),
            LlmProviderType::Openai
        );
        assert_eq!(
            "gemini".parse::<LlmProviderType>().unwrap(),
            LlmProviderType::Google
        );
        assert_eq!(
            "aws-bedrock".parse::<LlmProviderType>().unwrap(),
            LlmProviderType::Bedrock
        );
        assert_eq!(
            "none".parse::<LlmProviderType>().unwrap(),
            LlmProviderType::None
        );
    }

    #[test]
    fn rejects_unknown_provider() {
        let err = "wat".parse::<LlmProviderType>().unwrap_err();
        assert!(err.to_string().contains("Unknown LLM provider"));
    }

    #[test]
    fn none_provider_returns_no_service() {
        let config = LlmProviderConfig {
            provider_type: LlmProviderType::None,
            openai: None,
            google: None,
            bedrock: None,
        };

        let service = create_llm_service(config).unwrap();
        assert!(service.is_none());
    }

    #[test]
    #[cfg(not(feature = "google-provider"))]
    fn google_provider_is_explicitly_unavailable() {
        let config = LlmProviderConfig {
            provider_type: LlmProviderType::Google,
            openai: None,
            google: Some(GoogleLlmConfig {
                project_id: "p".into(),
                location: "global".into(),
                model: "gemini-2.5-flash".into(),
            }),
            bedrock: None,
        };

        let err = match create_llm_service(config) {
            Ok(_) => panic!("expected google provider to be unavailable"),
            Err(err) => err,
        };
        assert!(err.to_string().contains("google"));
    }

    #[test]
    #[cfg(feature = "google-provider")]
    fn google_provider_is_constructed_when_feature_enabled() {
        let config = LlmProviderConfig {
            provider_type: LlmProviderType::Google,
            openai: None,
            google: Some(GoogleLlmConfig {
                project_id: "p".into(),
                location: "global".into(),
                model: "gemini-2.5-flash".into(),
            }),
            bedrock: None,
        };

        let service = create_llm_service(config).unwrap();
        assert!(service.is_some());
    }

    #[test]
    #[cfg(not(feature = "bedrock-provider"))]
    fn bedrock_provider_is_explicitly_unavailable_without_feature() {
        let config = LlmProviderConfig {
            provider_type: LlmProviderType::Bedrock,
            openai: None,
            google: None,
            bedrock: Some(BedrockLlmConfig {
                region: "us-east-1".into(),
                model_id: "anthropic.claude-3-5-haiku-20241022-v1:0".into(),
            }),
        };

        let err = match create_llm_service(config) {
            Ok(_) => panic!("expected bedrock provider to be unavailable"),
            Err(err) => err,
        };
        assert!(err.to_string().contains("bedrock"));
    }

    #[test]
    #[cfg(feature = "bedrock-provider")]
    fn bedrock_provider_is_constructed_when_feature_enabled() {
        let config = LlmProviderConfig {
            provider_type: LlmProviderType::Bedrock,
            openai: None,
            google: None,
            bedrock: Some(BedrockLlmConfig {
                region: "us-east-1".into(),
                model_id: "anthropic.claude-3-5-haiku-20241022-v1:0".into(),
            }),
        };

        let service = create_llm_service(config).unwrap();
        assert!(service.is_some());
    }

    /// `AETERNA_OPENAI_BASE_URL` (when set + non-empty) is plumbed through
    /// `OpenAiLlmConfig::from_env` so e2e adapters and self-hosted users
    /// can route to OpenAI-compat endpoints (ollama, GitHub Models, etc).
    /// Empty string is treated as unset to match shell-export ergonomics.
    ///
    /// SAFETY: `std::env::{set_var,remove_var}` are unsafe in edition 2024
    /// because they race other threads. Cargo-test is multithreaded, so
    /// strictly speaking concurrent tests reading these vars could observe
    /// torn state. In practice no other test in this crate touches
    /// `AETERNA_OPENAI_BASE_URL`, and `OPENAI_API_KEY` is read here only
    /// to satisfy `from_env`'s required-key check. If we add more env-using
    /// tests later, gate them with a process-wide mutex.
    #[test]
    fn openai_config_reads_optional_base_url_from_env() {
        let prior_key = std::env::var("OPENAI_API_KEY").ok();
        let prior_url = std::env::var("AETERNA_OPENAI_BASE_URL").ok();
        // SAFETY: see method-level comment.
        unsafe {
            std::env::set_var("OPENAI_API_KEY", "sk-test");

            std::env::remove_var("AETERNA_OPENAI_BASE_URL");
            let cfg = OpenAiLlmConfig::from_env().expect("config");
            assert_eq!(cfg.base_url, None, "unset env => None");

            std::env::set_var("AETERNA_OPENAI_BASE_URL", "");
            let cfg = OpenAiLlmConfig::from_env().expect("config");
            assert_eq!(cfg.base_url, None, "empty env => None (treat as unset)");

            std::env::set_var("AETERNA_OPENAI_BASE_URL", "http://localhost:11434/v1");
            let cfg = OpenAiLlmConfig::from_env().expect("config");
            assert_eq!(cfg.base_url.as_deref(), Some("http://localhost:11434/v1"));

            // Restore.
            match prior_key {
                Some(v) => std::env::set_var("OPENAI_API_KEY", v),
                None => std::env::remove_var("OPENAI_API_KEY"),
            }
            match prior_url {
                Some(v) => std::env::set_var("AETERNA_OPENAI_BASE_URL", v),
                None => std::env::remove_var("AETERNA_OPENAI_BASE_URL"),
            }
        }
    }
}
