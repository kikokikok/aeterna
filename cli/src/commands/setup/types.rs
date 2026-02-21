use clap::ValueEnum;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
pub enum DeploymentTarget {
    DockerCompose,
    Kubernetes,
    OpencodeOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
pub enum DeploymentMode {
    Local,
    Hybrid,
    Remote,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
pub enum AuthMethod {
    ApiKey,
    Oauth2,
    ServiceAccount,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
pub enum VectorBackend {
    Qdrant,
    Pgvector,
    Pinecone,
    Weaviate,
    Mongodb,
    VertexAi,
    Databricks,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
pub enum CacheType {
    Dragonfly,
    Valkey,
    External,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
pub enum PostgresqlType {
    CloudNativePg,
    External,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
pub enum LlmProvider {
    Openai,
    Anthropic,
    Ollama,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalRedisConfig {
    pub host: String,
    pub port: u16,
    pub password: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalPostgresConfig {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PineconeConfig {
    pub api_key: String,
    pub environment: String,
    pub index_name: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WeaviateConfig {
    pub host: String,
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MongodbConfig {
    pub connection_uri: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VertexAiConfig {
    pub project_id: String,
    pub region: String,
    pub endpoint_url: String,
    pub service_account_json: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DatabricksConfig {
    pub workspace_url: String,
    pub token: String,
    pub catalog: String,
}

/// Hybrid mode: `local_cache_size_mb` is in MB, `sync_interval_secs` is in seconds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HybridConfig {
    pub local_cache_size_mb: u32,
    pub offline_cedar: bool,
    pub sync_interval_secs: u64,
}

impl Default for HybridConfig {
    fn default() -> Self {
        Self {
            local_cache_size_mb: 512,
            offline_cedar: true,
            sync_interval_secs: 300,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupConfig {
    pub target: DeploymentTarget,
    pub mode: DeploymentMode,

    pub central_url: Option<String>,
    pub central_auth: AuthMethod,
    pub api_key: Option<String>,

    pub hybrid: Option<HybridConfig>,

    pub vector_backend: VectorBackend,

    pub pinecone: Option<PineconeConfig>,
    pub weaviate: Option<WeaviateConfig>,
    pub mongodb: Option<MongodbConfig>,
    pub vertex_ai: Option<VertexAiConfig>,
    pub databricks: Option<DatabricksConfig>,

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
    pub pdb_enabled: bool,
}

impl Default for SetupConfig {
    fn default() -> Self {
        Self {
            target: DeploymentTarget::DockerCompose,
            mode: DeploymentMode::Local,
            central_url: None,
            central_auth: AuthMethod::ApiKey,
            api_key: None,
            hybrid: None,
            vector_backend: VectorBackend::Qdrant,
            pinecone: None,
            weaviate: None,
            mongodb: None,
            vertex_ai: None,
            databricks: None,
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
            pdb_enabled: false,
        }
    }
}

pub async fn validate_server_connectivity(url: &str, timeout_secs: u64) -> Result<(), String> {
    use std::time::Duration;
    use tokio::net::TcpStream;
    use tokio::time::timeout;

    let addr = parse_host_port(url).map_err(|e| format!("Invalid URL: {e}"))?;

    match timeout(Duration::from_secs(timeout_secs), TcpStream::connect(&addr)).await {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(e)) => Err(format!("Connection to {addr} failed: {e}")),
        Err(_) => Err(format!(
            "Connection to {addr} timed out after {timeout_secs}s"
        )),
    }
}

pub fn parse_host_port(url: &str) -> Result<String, String> {
    let stripped = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .ok_or_else(|| "URL must start with http:// or https://".to_string())?;

    let is_https = url.starts_with("https://");

    let authority = stripped.split('/').next().unwrap_or(stripped);

    if authority.contains(':') {
        Ok(authority.to_string())
    } else {
        let default_port = if is_https { 443 } else { 80 };
        Ok(format!("{authority}:{default_port}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = SetupConfig::default();
        assert_eq!(cfg.target, DeploymentTarget::DockerCompose);
        assert_eq!(cfg.mode, DeploymentMode::Local);
        assert_eq!(cfg.vector_backend, VectorBackend::Qdrant);
        assert_eq!(cfg.cache, CacheType::Dragonfly);
        assert!(cfg.opal_enabled);
        assert!(!cfg.opencode_enabled);
        assert!(cfg.hybrid.is_none());
        assert!(cfg.pinecone.is_none());
        assert!(cfg.weaviate.is_none());
        assert!(cfg.mongodb.is_none());
        assert!(cfg.vertex_ai.is_none());
        assert!(cfg.databricks.is_none());
    }

    #[test]
    fn test_hybrid_config_defaults() {
        let hc = HybridConfig::default();
        assert_eq!(hc.local_cache_size_mb, 512);
        assert!(hc.offline_cedar);
        assert_eq!(hc.sync_interval_secs, 300);
    }

    #[test]
    fn test_pinecone_config() {
        let pc = PineconeConfig {
            api_key: "pk-test".into(),
            environment: "us-east1-gcp".into(),
            index_name: "my-index".into(),
        };
        assert_eq!(pc.api_key, "pk-test");
        assert_eq!(pc.environment, "us-east1-gcp");
        assert_eq!(pc.index_name, "my-index");
    }

    #[test]
    fn test_weaviate_config() {
        let wc = WeaviateConfig {
            host: "http://weaviate.local:8080".into(),
            api_key: Some("wk-test".into()),
        };
        assert_eq!(wc.host, "http://weaviate.local:8080");
        assert_eq!(wc.api_key.as_deref(), Some("wk-test"));
    }

    #[test]
    fn test_mongodb_config() {
        let mc = MongodbConfig {
            connection_uri: "mongodb+srv://user:pass@cluster.mongodb.net/db".into(),
        };
        assert!(mc.connection_uri.starts_with("mongodb"));
    }

    #[test]
    fn test_vertex_ai_config() {
        let vc = VertexAiConfig {
            project_id: "my-project".into(),
            region: "us-central1".into(),
            endpoint_url: "https://us-central1-aiplatform.googleapis.com".into(),
            service_account_json: Some("/path/to/sa.json".into()),
        };
        assert_eq!(vc.project_id, "my-project");
        assert_eq!(vc.region, "us-central1");
    }

    #[test]
    fn test_databricks_config() {
        let dc = DatabricksConfig {
            workspace_url: "https://adb-1234.azuredatabricks.net".into(),
            token: "dapi-test".into(),
            catalog: "main".into(),
        };
        assert_eq!(dc.catalog, "main");
    }

    #[test]
    fn test_parse_host_port_https() {
        assert_eq!(
            parse_host_port("https://example.com").unwrap(),
            "example.com:443"
        );
    }

    #[test]
    fn test_parse_host_port_http() {
        assert_eq!(
            parse_host_port("http://example.com").unwrap(),
            "example.com:80"
        );
    }

    #[test]
    fn test_parse_host_port_with_port() {
        assert_eq!(
            parse_host_port("https://example.com:8080").unwrap(),
            "example.com:8080"
        );
    }

    #[test]
    fn test_parse_host_port_with_path() {
        assert_eq!(
            parse_host_port("https://example.com:9090/api/v1").unwrap(),
            "example.com:9090"
        );
    }

    #[test]
    fn test_parse_host_port_invalid() {
        assert!(parse_host_port("ftp://example.com").is_err());
        assert!(parse_host_port("not-a-url").is_err());
    }

    #[tokio::test]
    async fn test_validate_connectivity_invalid_url() {
        let result = validate_server_connectivity("not-a-url", 1).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_validate_connectivity_timeout() {
        let result = validate_server_connectivity("http://192.0.2.1:12345", 1).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("timed out") || err.contains("failed"),
            "Unexpected error: {err}"
        );
    }

    #[test]
    fn test_config_serialization_roundtrip() {
        let cfg = SetupConfig {
            hybrid: Some(HybridConfig::default()),
            pinecone: Some(PineconeConfig {
                api_key: "test".into(),
                environment: "env".into(),
                index_name: "idx".into(),
            }),
            ..SetupConfig::default()
        };
        let json = serde_json::to_string(&cfg).expect("serialize");
        let _: SetupConfig = serde_json::from_str(&json).expect("deserialize");
    }

    #[test]
    fn test_hybrid_config_custom_values_roundtrip() {
        let hybrid = HybridConfig {
            local_cache_size_mb: 2048,
            offline_cedar: false,
            sync_interval_secs: 60,
        };
        let json = serde_json::to_string(&hybrid).expect("serialize hybrid");
        let deserialized: HybridConfig = serde_json::from_str(&json).expect("deserialize hybrid");
        assert_eq!(deserialized.local_cache_size_mb, 2048);
        assert!(!deserialized.offline_cedar);
        assert_eq!(deserialized.sync_interval_secs, 60);
    }

    #[test]
    fn test_hybrid_config_toml_roundtrip() {
        let hybrid = HybridConfig {
            local_cache_size_mb: 1024,
            offline_cedar: true,
            sync_interval_secs: 120,
        };
        let toml_str = toml::to_string(&hybrid).expect("serialize to TOML");
        let deserialized: HybridConfig = toml::from_str(&toml_str).expect("deserialize from TOML");
        assert_eq!(deserialized.local_cache_size_mb, 1024);
        assert!(deserialized.offline_cedar);
        assert_eq!(deserialized.sync_interval_secs, 120);
    }

    #[test]
    fn test_full_config_with_hybrid_roundtrip() {
        let cfg = SetupConfig {
            mode: DeploymentMode::Hybrid,
            central_url: Some("https://central.example.com".into()),
            hybrid: Some(HybridConfig {
                local_cache_size_mb: 256,
                offline_cedar: false,
                sync_interval_secs: 30,
            }),
            ..SetupConfig::default()
        };
        let json = serde_json::to_string(&cfg).expect("serialize full config");
        let deserialized: SetupConfig =
            serde_json::from_str(&json).expect("deserialize full config");
        assert_eq!(deserialized.mode, DeploymentMode::Hybrid);
        let hybrid = deserialized.hybrid.expect("hybrid should be present");
        assert_eq!(hybrid.local_cache_size_mb, 256);
        assert!(!hybrid.offline_cedar);
        assert_eq!(hybrid.sync_interval_secs, 30);
    }

    #[test]
    fn test_full_config_with_all_external_services_roundtrip() {
        let cfg = SetupConfig {
            vector_backend: VectorBackend::Pinecone,
            pinecone: Some(PineconeConfig {
                api_key: "pk-key".into(),
                environment: "us-east1".into(),
                index_name: "idx".into(),
            }),
            weaviate: Some(WeaviateConfig {
                host: "http://weaviate:8080".into(),
                api_key: Some("wk-key".into()),
            }),
            mongodb: Some(MongodbConfig {
                connection_uri: "mongodb+srv://u:p@c.net/db".into(),
            }),
            vertex_ai: Some(VertexAiConfig {
                project_id: "proj".into(),
                region: "us-central1".into(),
                endpoint_url: "https://ep".into(),
                service_account_json: Some("/sa.json".into()),
            }),
            databricks: Some(DatabricksConfig {
                workspace_url: "https://adb.net".into(),
                token: "dapi-tok".into(),
                catalog: "main".into(),
            }),
            ..SetupConfig::default()
        };
        let json = serde_json::to_string(&cfg).expect("serialize");
        let d: SetupConfig = serde_json::from_str(&json).expect("deserialize");
        assert!(d.pinecone.is_some());
        assert!(d.weaviate.is_some());
        assert!(d.mongodb.is_some());
        assert!(d.vertex_ai.is_some());
        assert!(d.databricks.is_some());
    }
}
