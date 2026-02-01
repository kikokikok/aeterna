use super::{BackendError, VectorBackend};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum VectorBackendType {
    #[default]
    Qdrant,
    Pinecone,
    Pgvector,
    VertexAi,
    Databricks,
    Weaviate,
    Mongodb
}

impl std::fmt::Display for VectorBackendType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VectorBackendType::Qdrant => write!(f, "qdrant"),
            VectorBackendType::Pinecone => write!(f, "pinecone"),
            VectorBackendType::Pgvector => write!(f, "pgvector"),
            VectorBackendType::VertexAi => write!(f, "vertex_ai"),
            VectorBackendType::Databricks => write!(f, "databricks"),
            VectorBackendType::Weaviate => write!(f, "weaviate"),
            VectorBackendType::Mongodb => write!(f, "mongodb")
        }
    }
}

impl std::str::FromStr for VectorBackendType {
    type Err = BackendError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "qdrant" => Ok(VectorBackendType::Qdrant),
            "pinecone" => Ok(VectorBackendType::Pinecone),
            "pgvector" => Ok(VectorBackendType::Pgvector),
            "vertex_ai" | "vertexai" => Ok(VectorBackendType::VertexAi),
            "databricks" => Ok(VectorBackendType::Databricks),
            "weaviate" => Ok(VectorBackendType::Weaviate),
            "mongodb" | "mongo" => Ok(VectorBackendType::Mongodb),
            _ => Err(BackendError::Configuration(format!(
                "Unknown backend type: {}. Valid options: qdrant, pinecone, pgvector, vertex_ai, \
                 databricks, weaviate, mongodb",
                s
            )))
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct BackendConfig {
    pub backend_type: VectorBackendType,
    pub embedding_dimension: usize,

    #[serde(default)]
    pub qdrant: Option<QdrantConfig>,

    #[serde(default)]
    pub pinecone: Option<PineconeConfig>,

    #[serde(default)]
    pub pgvector: Option<PgvectorConfig>,

    #[serde(default)]
    pub vertex_ai: Option<VertexAiConfig>,

    #[serde(default)]
    pub databricks: Option<DatabricksConfig>,

    #[serde(default)]
    pub weaviate: Option<WeaviateConfig>,

    #[serde(default)]
    pub mongodb: Option<MongodbConfig>
}

impl Default for BackendConfig {
    fn default() -> Self {
        Self {
            backend_type: VectorBackendType::Qdrant,
            embedding_dimension: 1536,
            qdrant: Some(QdrantConfig::default()),
            pinecone: None,
            pgvector: None,
            vertex_ai: None,
            databricks: None,
            weaviate: None,
            mongodb: None
        }
    }
}

impl BackendConfig {
    pub fn from_env() -> Result<Self, BackendError> {
        let backend_type = std::env::var("VECTOR_BACKEND")
            .unwrap_or_else(|_| "qdrant".to_string())
            .parse()?;

        let embedding_dimension = std::env::var("EMBEDDING_DIMENSION")
            .unwrap_or_else(|_| "1536".to_string())
            .parse()
            .map_err(|e| {
                BackendError::Configuration(format!("Invalid embedding dimension: {}", e))
            })?;

        let config = Self {
            backend_type,
            embedding_dimension,
            qdrant: QdrantConfig::from_env().ok(),
            pinecone: PineconeConfig::from_env().ok(),
            pgvector: PgvectorConfig::from_env().ok(),
            vertex_ai: VertexAiConfig::from_env().ok(),
            databricks: DatabricksConfig::from_env().ok(),
            weaviate: WeaviateConfig::from_env().ok(),
            mongodb: MongodbConfig::from_env().ok()
        };

        Ok(config)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct QdrantConfig {
    pub url: String,
    pub api_key: Option<String>,
    pub collection_prefix: String
}

impl Default for QdrantConfig {
    fn default() -> Self {
        Self {
            url: "http://localhost:6334".to_string(),
            api_key: None,
            collection_prefix: "aeterna".to_string()
        }
    }
}

impl QdrantConfig {
    pub fn from_env() -> Result<Self, BackendError> {
        Ok(Self {
            url: std::env::var("QDRANT_URL")
                .unwrap_or_else(|_| "http://localhost:6334".to_string()),
            api_key: std::env::var("QDRANT_API_KEY").ok(),
            collection_prefix: std::env::var("QDRANT_COLLECTION_PREFIX")
                .unwrap_or_else(|_| "aeterna".to_string())
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct PineconeConfig {
    pub api_key: String,
    pub environment: String,
    pub index_name: String
}

impl PineconeConfig {
    pub fn from_env() -> Result<Self, BackendError> {
        Ok(Self {
            api_key: std::env::var("PINECONE_API_KEY")
                .map_err(|_| BackendError::Configuration("PINECONE_API_KEY not set".into()))?,
            environment: std::env::var("PINECONE_ENVIRONMENT")
                .map_err(|_| BackendError::Configuration("PINECONE_ENVIRONMENT not set".into()))?,
            index_name: std::env::var("PINECONE_INDEX_NAME")
                .unwrap_or_else(|_| "aeterna-memories".to_string())
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct PgvectorConfig {
    pub connection_string: String,
    pub schema: String,
    pub table_name: String
}

impl Default for PgvectorConfig {
    fn default() -> Self {
        Self {
            connection_string: "postgres://localhost/aeterna".to_string(),
            schema: "public".to_string(),
            table_name: "vectors".to_string()
        }
    }
}

impl PgvectorConfig {
    pub fn from_env() -> Result<Self, BackendError> {
        Ok(Self {
            connection_string: std::env::var("PGVECTOR_URL")
                .or_else(|_| std::env::var("DATABASE_URL"))
                .map_err(|_| {
                    BackendError::Configuration("PGVECTOR_URL or DATABASE_URL not set".into())
                })?,
            schema: std::env::var("PGVECTOR_SCHEMA").unwrap_or_else(|_| "public".to_string()),
            table_name: std::env::var("PGVECTOR_TABLE").unwrap_or_else(|_| "vectors".to_string())
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct VertexAiConfig {
    pub project_id: String,
    pub location: String,
    pub index_endpoint: String,
    pub deployed_index_id: String
}

impl VertexAiConfig {
    pub fn from_env() -> Result<Self, BackendError> {
        Ok(Self {
            project_id: std::env::var("GCP_PROJECT_ID")
                .map_err(|_| BackendError::Configuration("GCP_PROJECT_ID not set".into()))?,
            location: std::env::var("VERTEX_AI_LOCATION")
                .unwrap_or_else(|_| "us-central1".to_string()),
            index_endpoint: std::env::var("VERTEX_AI_INDEX_ENDPOINT").map_err(|_| {
                BackendError::Configuration("VERTEX_AI_INDEX_ENDPOINT not set".into())
            })?,
            deployed_index_id: std::env::var("VERTEX_AI_DEPLOYED_INDEX_ID").map_err(|_| {
                BackendError::Configuration("VERTEX_AI_DEPLOYED_INDEX_ID not set".into())
            })?
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabricksConfig {
    pub workspace_url: String,
    pub token: String,
    pub catalog: String,
    pub schema: String
}

impl DatabricksConfig {
    pub fn from_env() -> Result<Self, BackendError> {
        Ok(Self {
            workspace_url: std::env::var("DATABRICKS_HOST")
                .map_err(|_| BackendError::Configuration("DATABRICKS_HOST not set".into()))?,
            token: std::env::var("DATABRICKS_TOKEN")
                .map_err(|_| BackendError::Configuration("DATABRICKS_TOKEN not set".into()))?,
            catalog: std::env::var("DATABRICKS_CATALOG").unwrap_or_else(|_| "main".to_string()),
            schema: std::env::var("DATABRICKS_SCHEMA").unwrap_or_else(|_| "aeterna".to_string())
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct WeaviateConfig {
    pub url: String,
    pub api_key: Option<String>,
    pub class_name: String
}

impl Default for WeaviateConfig {
    fn default() -> Self {
        Self {
            url: "http://localhost:8080".to_string(),
            api_key: None,
            class_name: "AeternaMemory".to_string()
        }
    }
}

impl WeaviateConfig {
    pub fn from_env() -> Result<Self, BackendError> {
        Ok(Self {
            url: std::env::var("WEAVIATE_URL")
                .unwrap_or_else(|_| "http://localhost:8080".to_string()),
            api_key: std::env::var("WEAVIATE_API_KEY").ok(),
            class_name: std::env::var("WEAVIATE_CLASS")
                .unwrap_or_else(|_| "AeternaMemory".to_string())
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct MongodbConfig {
    pub connection_string: String,
    pub database: String,
    pub collection: String,
    pub index_name: String
}

impl MongodbConfig {
    pub fn from_env() -> Result<Self, BackendError> {
        Ok(Self {
            connection_string: std::env::var("MONGODB_URI")
                .map_err(|_| BackendError::Configuration("MONGODB_URI not set".into()))?,
            database: std::env::var("MONGODB_DATABASE").unwrap_or_else(|_| "aeterna".to_string()),
            collection: std::env::var("MONGODB_COLLECTION")
                .unwrap_or_else(|_| "vectors".to_string()),
            index_name: std::env::var("MONGODB_VECTOR_INDEX")
                .unwrap_or_else(|_| "vector_index".to_string())
        })
    }
}

pub async fn create_backend(config: BackendConfig) -> Result<Arc<dyn VectorBackend>, BackendError> {
    match config.backend_type {
        VectorBackendType::Qdrant => {
            let qdrant_config = config
                .qdrant
                .ok_or_else(|| BackendError::Configuration("Qdrant config missing".into()))?;
            let backend =
                super::qdrant::QdrantBackend::new(qdrant_config, config.embedding_dimension)
                    .await?;
            Ok(Arc::new(backend))
        }
        VectorBackendType::Pinecone => {
            #[cfg(feature = "pinecone")]
            {
                let pinecone_config = config
                    .pinecone
                    .ok_or_else(|| BackendError::Configuration("Pinecone config missing".into()))?;
                let backend = super::pinecone::PineconeBackend::new(pinecone_config).await?;
                Ok(Arc::new(backend))
            }
            #[cfg(not(feature = "pinecone"))]
            {
                Err(BackendError::Configuration(
                    "Pinecone backend not enabled. Compile with --features pinecone".into()
                ))
            }
        }
        VectorBackendType::Pgvector => {
            #[cfg(feature = "pgvector")]
            {
                let pgvector_config = config
                    .pgvector
                    .ok_or_else(|| BackendError::Configuration("pgvector config missing".into()))?;
                let backend = super::pgvector::PgvectorBackend::new(
                    pgvector_config,
                    config.embedding_dimension
                )
                .await?;
                Ok(Arc::new(backend))
            }
            #[cfg(not(feature = "pgvector"))]
            {
                Err(BackendError::Configuration(
                    "pgvector backend not enabled. Compile with --features pgvector".into()
                ))
            }
        }
        VectorBackendType::VertexAi => {
            #[cfg(feature = "vertex-ai")]
            {
                let vertex_config = config.vertex_ai.ok_or_else(|| {
                    BackendError::Configuration("Vertex AI config missing".into())
                })?;
                let backend = super::vertex_ai::VertexAiBackend::new(vertex_config).await?;
                Ok(Arc::new(backend))
            }
            #[cfg(not(feature = "vertex-ai"))]
            {
                Err(BackendError::Configuration(
                    "Vertex AI backend not enabled. Compile with --features vertex-ai".into()
                ))
            }
        }
        VectorBackendType::Databricks => {
            #[cfg(feature = "databricks")]
            {
                let databricks_config = config.databricks.ok_or_else(|| {
                    BackendError::Configuration("Databricks config missing".into())
                })?;
                let backend = super::databricks::DatabricksBackend::new(databricks_config).await?;
                Ok(Arc::new(backend))
            }
            #[cfg(not(feature = "databricks"))]
            {
                Err(BackendError::Configuration(
                    "Databricks backend not enabled. Compile with --features databricks".into()
                ))
            }
        }
        VectorBackendType::Weaviate => {
            #[cfg(feature = "weaviate")]
            {
                let weaviate_config = config
                    .weaviate
                    .ok_or_else(|| BackendError::Configuration("Weaviate config missing".into()))?;
                let backend = super::weaviate::WeaviateBackend::new(weaviate_config).await?;
                Ok(Arc::new(backend))
            }
            #[cfg(not(feature = "weaviate"))]
            {
                Err(BackendError::Configuration(
                    "Weaviate backend not enabled. Compile with --features weaviate".into()
                ))
            }
        }
        VectorBackendType::Mongodb => {
            #[cfg(feature = "mongodb")]
            {
                let mongodb_config = config
                    .mongodb
                    .ok_or_else(|| BackendError::Configuration("MongoDB config missing".into()))?;
                let backend = super::mongodb::MongodbBackend::new(mongodb_config).await?;
                Ok(Arc::new(backend))
            }
            #[cfg(not(feature = "mongodb"))]
            {
                Err(BackendError::Configuration(
                    "MongoDB backend not enabled. Compile with --features mongodb".into()
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_type_parsing() {
        assert_eq!(
            "qdrant".parse::<VectorBackendType>().unwrap(),
            VectorBackendType::Qdrant
        );
        assert_eq!(
            "pinecone".parse::<VectorBackendType>().unwrap(),
            VectorBackendType::Pinecone
        );
        assert_eq!(
            "pgvector".parse::<VectorBackendType>().unwrap(),
            VectorBackendType::Pgvector
        );
        assert_eq!(
            "vertex_ai".parse::<VectorBackendType>().unwrap(),
            VectorBackendType::VertexAi
        );
        assert_eq!(
            "vertexai".parse::<VectorBackendType>().unwrap(),
            VectorBackendType::VertexAi
        );
        assert_eq!(
            "databricks".parse::<VectorBackendType>().unwrap(),
            VectorBackendType::Databricks
        );
        assert_eq!(
            "weaviate".parse::<VectorBackendType>().unwrap(),
            VectorBackendType::Weaviate
        );
        assert_eq!(
            "mongodb".parse::<VectorBackendType>().unwrap(),
            VectorBackendType::Mongodb
        );
        assert_eq!(
            "mongo".parse::<VectorBackendType>().unwrap(),
            VectorBackendType::Mongodb
        );

        assert!("unknown".parse::<VectorBackendType>().is_err());
    }

    #[test]
    fn test_backend_type_display() {
        assert_eq!(VectorBackendType::Qdrant.to_string(), "qdrant");
        assert_eq!(VectorBackendType::Pinecone.to_string(), "pinecone");
        assert_eq!(VectorBackendType::VertexAi.to_string(), "vertex_ai");
    }

    #[test]
    fn test_default_config() {
        let config = BackendConfig::default();
        assert_eq!(config.backend_type, VectorBackendType::Qdrant);
        assert_eq!(config.embedding_dimension, 1536);
        assert!(config.qdrant.is_some());
    }
}
