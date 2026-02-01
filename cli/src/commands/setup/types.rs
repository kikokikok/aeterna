use clap::ValueEnum;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
pub enum DeploymentTarget {
    DockerCompose,
    Kubernetes,
    OpencodeOnly
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
pub enum DeploymentMode {
    Local,
    Hybrid,
    Remote
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
pub enum AuthMethod {
    ApiKey,
    Oauth2,
    ServiceAccount
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
pub enum VectorBackend {
    Qdrant,
    Pgvector,
    Pinecone,
    Weaviate,
    Mongodb,
    VertexAi,
    Databricks
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
pub enum CacheType {
    Dragonfly,
    Valkey,
    External
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
pub enum PostgresqlType {
    CloudNativePg,
    External
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
pub enum LlmProvider {
    Openai,
    Anthropic,
    Ollama,
    None
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalRedisConfig {
    pub host: String,
    pub port: u16,
    pub password: Option<String>
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalPostgresConfig {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: Option<String>,
    pub password: Option<String>
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupConfig {
    pub target: DeploymentTarget,
    pub mode: DeploymentMode,

    pub central_url: Option<String>,
    pub central_auth: AuthMethod,
    pub api_key: Option<String>,

    pub vector_backend: VectorBackend,

    pub cache: CacheType,
    pub redis_external: Option<ExternalRedisConfig>,

    pub postgresql: PostgresqlType,
    pub pg_external: Option<ExternalPostgresConfig>,

    pub opal_enabled: bool,

    pub llm_provider: LlmProvider,
    pub openai_api_key: Option<String>,
    pub anthropic_api_key: Option<String>,
    pub ollama_host: Option<String>,

    pub opencode_enabled: bool,

    pub ingress_enabled: bool,
    pub ingress_host: Option<String>,
    pub service_monitor_enabled: bool,
    pub network_policy_enabled: bool,
    pub hpa_enabled: bool,
    pub pdb_enabled: bool
}

impl Default for SetupConfig {
    fn default() -> Self {
        Self {
            target: DeploymentTarget::DockerCompose,
            mode: DeploymentMode::Local,
            central_url: None,
            central_auth: AuthMethod::ApiKey,
            api_key: None,
            vector_backend: VectorBackend::Qdrant,
            cache: CacheType::Dragonfly,
            redis_external: None,
            postgresql: PostgresqlType::CloudNativePg,
            pg_external: None,
            opal_enabled: true,
            llm_provider: LlmProvider::None,
            openai_api_key: None,
            anthropic_api_key: None,
            ollama_host: None,
            opencode_enabled: false,
            ingress_enabled: false,
            ingress_host: None,
            service_monitor_enabled: false,
            network_policy_enabled: false,
            hpa_enabled: false,
            pdb_enabled: false
        }
    }
}
