use serde::{Deserialize, Serialize};
use std::sync::Arc;

use mk_core::traits::EmbeddingService;

#[derive(Debug, thiserror::Error)]
pub enum EmbeddingFactoryError {
    #[error("Embedding provider configuration error: {0}")]
    Configuration(String),
    #[error("Embedding provider unavailable in this build: {0}")]
    Unavailable(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingProviderType {
    Openai,
    Google,
    Bedrock,
    #[default]
    None,
}

impl std::fmt::Display for EmbeddingProviderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EmbeddingProviderType::Openai => write!(f, "openai"),
            EmbeddingProviderType::Google => write!(f, "google"),
            EmbeddingProviderType::Bedrock => write!(f, "bedrock"),
            EmbeddingProviderType::None => write!(f, "none"),
        }
    }
}

impl std::str::FromStr for EmbeddingProviderType {
    type Err = EmbeddingFactoryError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "openai" => Ok(EmbeddingProviderType::Openai),
            "google" | "vertex" | "vertex_ai" | "vertexai" | "gemini" => {
                Ok(EmbeddingProviderType::Google)
            }
            "bedrock" | "aws_bedrock" | "aws-bedrock" => Ok(EmbeddingProviderType::Bedrock),
            "none" => Ok(EmbeddingProviderType::None),
            _ => Err(EmbeddingFactoryError::Configuration(format!(
                "Unknown embedding provider: {s}. Valid options: openai, google, bedrock, none"
            ))),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct OpenAiEmbeddingConfig {
    pub model: String,
    pub api_key: String,
}

impl OpenAiEmbeddingConfig {
    pub fn from_env() -> Result<Self, EmbeddingFactoryError> {
        Ok(Self {
            model: std::env::var("AETERNA_OPENAI_EMBEDDING_MODEL")
                .unwrap_or_else(|_| "text-embedding-3-small".to_string()),
            api_key: std::env::var("OPENAI_API_KEY").map_err(|_| {
                EmbeddingFactoryError::Configuration("OPENAI_API_KEY not set".into())
            })?,
        })
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct GoogleEmbeddingConfig {
    pub project_id: String,
    pub location: String,
    pub model: String,
}

impl GoogleEmbeddingConfig {
    pub fn from_env() -> Result<Self, EmbeddingFactoryError> {
        Ok(Self {
            project_id: std::env::var("AETERNA_GOOGLE_PROJECT_ID").map_err(|_| {
                EmbeddingFactoryError::Configuration("AETERNA_GOOGLE_PROJECT_ID not set".into())
            })?,
            location: std::env::var("AETERNA_GOOGLE_LOCATION").map_err(|_| {
                EmbeddingFactoryError::Configuration("AETERNA_GOOGLE_LOCATION not set".into())
            })?,
            model: std::env::var("AETERNA_GOOGLE_EMBEDDING_MODEL").map_err(|_| {
                EmbeddingFactoryError::Configuration(
                    "AETERNA_GOOGLE_EMBEDDING_MODEL not set".into(),
                )
            })?,
        })
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct BedrockEmbeddingConfig {
    pub region: String,
    pub model_id: String,
}

impl BedrockEmbeddingConfig {
    pub fn from_env() -> Result<Self, EmbeddingFactoryError> {
        Ok(Self {
            region: std::env::var("AETERNA_BEDROCK_REGION").map_err(|_| {
                EmbeddingFactoryError::Configuration("AETERNA_BEDROCK_REGION not set".into())
            })?,
            model_id: std::env::var("AETERNA_BEDROCK_EMBEDDING_MODEL").map_err(|_| {
                EmbeddingFactoryError::Configuration(
                    "AETERNA_BEDROCK_EMBEDDING_MODEL not set".into(),
                )
            })?,
        })
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct EmbeddingProviderConfig {
    pub provider_type: EmbeddingProviderType,
    pub openai: Option<OpenAiEmbeddingConfig>,
    pub google: Option<GoogleEmbeddingConfig>,
    pub bedrock: Option<BedrockEmbeddingConfig>,
}

impl EmbeddingProviderConfig {
    pub fn from_env() -> Result<Self, EmbeddingFactoryError> {
        let provider_type = std::env::var("AETERNA_LLM_PROVIDER")
            .unwrap_or_else(|_| "none".to_string())
            .parse()?;

        Ok(Self {
            provider_type,
            openai: matches!(provider_type, EmbeddingProviderType::Openai)
                .then(OpenAiEmbeddingConfig::from_env)
                .transpose()?,
            google: matches!(provider_type, EmbeddingProviderType::Google)
                .then(GoogleEmbeddingConfig::from_env)
                .transpose()?,
            bedrock: matches!(provider_type, EmbeddingProviderType::Bedrock)
                .then(BedrockEmbeddingConfig::from_env)
                .transpose()?,
        })
    }
}

pub fn create_embedding_service_from_env() -> Result<
    Option<
        Arc<dyn EmbeddingService<Error = Box<dyn std::error::Error + Send + Sync>> + Send + Sync>,
    >,
    EmbeddingFactoryError,
> {
    let config = EmbeddingProviderConfig::from_env()?;
    create_embedding_service(config)
}

pub fn create_embedding_service(
    config: EmbeddingProviderConfig,
) -> Result<
    Option<
        Arc<dyn EmbeddingService<Error = Box<dyn std::error::Error + Send + Sync>> + Send + Sync>,
    >,
    EmbeddingFactoryError,
> {
    match config.provider_type {
        EmbeddingProviderType::None => Ok(None),
        EmbeddingProviderType::Openai => {
            #[allow(unused_variables)]
            let openai = config.openai.ok_or_else(|| {
                EmbeddingFactoryError::Configuration("OpenAI embedding config missing".into())
            })?;

            #[cfg(feature = "embedding-integration")]
            {
                let service =
                    super::openai::OpenAIEmbeddingService::new(openai.api_key, &openai.model);
                Ok(Some(Arc::new(service)))
            }

            #[cfg(not(feature = "embedding-integration"))]
            {
                return Err(EmbeddingFactoryError::Unavailable(
                    "openai embeddings require the embedding-integration feature".into(),
                ));
            }
        }
        EmbeddingProviderType::Google => {
            let google = config.google.ok_or_else(|| {
                EmbeddingFactoryError::Configuration("Google embedding config missing".into())
            })?;

            #[cfg(feature = "google-provider")]
            {
                let service = super::google::GoogleEmbeddingService::new(
                    google.project_id,
                    google.location,
                    google.model,
                );
                Ok(Some(Arc::new(service)))
            }

            #[cfg(not(feature = "google-provider"))]
            {
                let _ = google;
                return Err(EmbeddingFactoryError::Unavailable(
                    "google support requires the google-provider feature".into(),
                ));
            }
        }
        EmbeddingProviderType::Bedrock => {
            let bedrock = config.bedrock.ok_or_else(|| {
                EmbeddingFactoryError::Configuration("Bedrock embedding config missing".into())
            })?;

            #[cfg(feature = "bedrock-provider")]
            {
                let service =
                    super::bedrock::BedrockEmbeddingService::new(bedrock.region, bedrock.model_id);
                Ok(Some(Arc::new(service)))
            }

            #[cfg(not(feature = "bedrock-provider"))]
            {
                let _ = bedrock;
                return Err(EmbeddingFactoryError::Unavailable(
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
            "openai".parse::<EmbeddingProviderType>().unwrap(),
            EmbeddingProviderType::Openai
        );
        assert_eq!(
            "vertexai".parse::<EmbeddingProviderType>().unwrap(),
            EmbeddingProviderType::Google
        );
        assert_eq!(
            "bedrock".parse::<EmbeddingProviderType>().unwrap(),
            EmbeddingProviderType::Bedrock
        );
        assert_eq!(
            "none".parse::<EmbeddingProviderType>().unwrap(),
            EmbeddingProviderType::None
        );
    }

    #[test]
    fn rejects_unknown_provider() {
        let err = "wat".parse::<EmbeddingProviderType>().unwrap_err();
        assert!(err.to_string().contains("Unknown embedding provider"));
    }

    #[test]
    fn none_provider_returns_no_service() {
        let config = EmbeddingProviderConfig {
            provider_type: EmbeddingProviderType::None,
            openai: None,
            google: None,
            bedrock: None,
        };

        let service = create_embedding_service(config).unwrap();
        assert!(service.is_none());
    }

    #[test]
    #[cfg(not(feature = "bedrock-provider"))]
    fn bedrock_provider_is_explicitly_unavailable() {
        let config = EmbeddingProviderConfig {
            provider_type: EmbeddingProviderType::Bedrock,
            openai: None,
            google: None,
            bedrock: Some(BedrockEmbeddingConfig {
                region: "eu-west-1".into(),
                model_id: "amazon.titan-embed-text-v2:0".into(),
            }),
        };

        let err = match create_embedding_service(config) {
            Ok(_) => panic!("expected bedrock provider to be unavailable"),
            Err(err) => err,
        };
        assert!(err.to_string().contains("bedrock"));
    }

    #[test]
    #[cfg(feature = "bedrock-provider")]
    fn bedrock_provider_is_constructed_when_feature_enabled() {
        let config = EmbeddingProviderConfig {
            provider_type: EmbeddingProviderType::Bedrock,
            openai: None,
            google: None,
            bedrock: Some(BedrockEmbeddingConfig {
                region: "us-east-1".into(),
                model_id: "amazon.titan-embed-text-v2:0".into(),
            }),
        };

        let service = create_embedding_service(config).unwrap();
        assert!(service.is_some());
    }
}
