use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use super::types::*;

pub fn generate_all(config: &SetupConfig, output_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut generated = Vec::new();

    let aeterna_dir = output_dir.join(".aeterna");
    fs::create_dir_all(&aeterna_dir)?;

    let config_content = generate_config_toml(config);
    validate_toml(&config_content).context("Generated config.toml is invalid")?;
    let config_path = aeterna_dir.join("config.toml");
    backup_if_exists(&config_path)?;
    fs::write(&config_path, config_content)?;
    generated.push(config_path);

    match config.target {
        DeploymentTarget::DockerCompose => {
            let compose_content = generate_docker_compose(config);
            validate_yaml(&compose_content).context("Generated docker-compose.yaml is invalid")?;
            let compose_path = output_dir.join("docker-compose.yaml");
            backup_if_exists(&compose_path)?;
            fs::write(&compose_path, compose_content)?;
            generated.push(compose_path);
        }
        DeploymentTarget::Kubernetes => {
            let values_content = generate_helm_values(config);
            validate_yaml(&values_content).context("Generated values.yaml is invalid")?;
            let values_path = output_dir.join("values.yaml");
            backup_if_exists(&values_path)?;
            fs::write(&values_path, values_content)?;
            generated.push(values_path);
        }
        DeploymentTarget::OpencodeOnly => {}
    }

    if config.opencode_enabled {
        if let Some(mcp_path) = generate_opencode_config(config)? {
            generated.push(mcp_path);
        }
    }

    Ok(generated)
}

pub fn validate_yaml(content: &str) -> Result<()> {
    serde_yaml::from_str::<serde_yaml::Value>(content).context("YAML validation failed")?;
    Ok(())
}

pub fn validate_toml(content: &str) -> Result<()> {
    toml::from_str::<toml::Value>(content).context("TOML validation failed")?;
    Ok(())
}

fn backup_if_exists(path: &Path) -> Result<()> {
    if path.exists() {
        let backup_path = path.with_extension("yaml.bak");
        fs::copy(path, &backup_path)
            .with_context(|| format!("Failed to backup {}", path.display()))?;
    }
    Ok(())
}

pub fn generate_config_toml(config: &SetupConfig) -> String {
    let mut content = String::new();

    content.push_str(&format!("mode = \"{:?}\"\n", config.mode).to_lowercase());
    content.push('\n');

    if let Some(url) = &config.central_url {
        content.push_str("[central]\n");
        content.push_str(&format!("url = \"{}\"\n", url));
        content.push_str(&format!("auth = \"{:?}\"\n", config.central_auth).to_lowercase());
        content.push('\n');
    }

    if let Some(hybrid) = &config.hybrid {
        content.push_str("[hybrid]\n");
        content.push_str(&format!(
            "local_cache_size_mb = {}\n",
            hybrid.local_cache_size_mb
        ));
        content.push_str(&format!("offline_cedar = {}\n", hybrid.offline_cedar));
        content.push_str(&format!(
            "sync_interval_secs = {}\n",
            hybrid.sync_interval_secs
        ));
        content.push('\n');
    }

    content.push_str("[vector]\n");
    content.push_str(&format!("backend = \"{:?}\"\n", config.vector_backend).to_lowercase());

    if let Some(pc) = &config.pinecone {
        content.push_str(&format!("pinecone_environment = \"{}\"\n", pc.environment));
        content.push_str(&format!("pinecone_index = \"{}\"\n", pc.index_name));
    }
    if let Some(wc) = &config.weaviate {
        content.push_str(&format!("weaviate_host = \"{}\"\n", wc.host));
    }
    if let Some(mc) = &config.mongodb {
        content.push_str(&format!("mongodb_uri = \"{}\"\n", mc.connection_uri));
    }
    if let Some(vc) = &config.vertex_ai {
        content.push_str(&format!("vertex_project = \"{}\"\n", vc.project_id));
        content.push_str(&format!("vertex_region = \"{}\"\n", vc.region));
        content.push_str(&format!("vertex_endpoint = \"{}\"\n", vc.endpoint_url));
    }
    if let Some(dc) = &config.databricks {
        content.push_str(&format!(
            "databricks_workspace = \"{}\"\n",
            dc.workspace_url
        ));
        content.push_str(&format!("databricks_catalog = \"{}\"\n", dc.catalog));
    }
    content.push('\n');

    content.push_str("[cache]\n");
    content.push_str(&format!("type = \"{:?}\"\n", config.cache).to_lowercase());
    if let Some(redis) = &config.redis_external {
        content.push_str(&format!("host = \"{}\"\n", redis.host));
        content.push_str(&format!("port = {}\n", redis.port));
    }
    content.push('\n');

    content.push_str("[postgresql]\n");
    content.push_str(&format!("type = \"{:?}\"\n", config.postgresql).to_lowercase());
    if let Some(pg) = &config.pg_external {
        content.push_str(&format!("host = \"{}\"\n", pg.host));
        content.push_str(&format!("port = {}\n", pg.port));
        content.push_str(&format!("database = \"{}\"\n", pg.database));
    }
    content.push('\n');

    content.push_str("[authorization]\n");
    content.push_str(&format!("opal_enabled = {}\n", config.opal_enabled));
    content.push('\n');

    content.push_str("[llm]\n");
    content.push_str(&format!("provider = \"{:?}\"\n", config.llm_provider).to_lowercase());
    if let Some(host) = &config.ollama_host {
        content.push_str(&format!("ollama_host = \"{}\"\n", host));
    }
    content.push('\n');

    content.push_str("[features]\n");
    content.push_str(&format!("opencode = {}\n", config.opencode_enabled));
    content.push('\n');

    content
}

pub fn generate_docker_compose(config: &SetupConfig) -> String {
    let mut content = String::new();

    content.push_str("services:\n");

    if matches!(config.mode, DeploymentMode::Local | DeploymentMode::Hybrid) {
        if matches!(config.cache, CacheType::Dragonfly) {
            content.push_str("  dragonfly:\n");
            content.push_str("    image: docker.dragonflydb.io/dragonflydb/dragonfly:latest\n");
            content.push_str("    ports:\n");
            content.push_str("      - \"6379:6379\"\n");
            content.push_str("    volumes:\n");
            content.push_str("      - dragonfly_data:/data\n");
            content.push_str("    healthcheck:\n");
            content.push_str("      test: [\"CMD\", \"redis-cli\", \"ping\"]\n");
            content.push_str("      interval: 10s\n");
            content.push_str("      timeout: 5s\n");
            content.push_str("      retries: 5\n");
            content.push('\n');
        } else if matches!(config.cache, CacheType::Valkey) {
            content.push_str("  valkey:\n");
            content.push_str("    image: valkey/valkey:latest\n");
            content.push_str("    ports:\n");
            content.push_str("      - \"6379:6379\"\n");
            content.push_str("    volumes:\n");
            content.push_str("      - valkey_data:/data\n");
            content.push_str("    healthcheck:\n");
            content.push_str("      test: [\"CMD\", \"valkey-cli\", \"ping\"]\n");
            content.push_str("      interval: 10s\n");
            content.push_str("      timeout: 5s\n");
            content.push_str("      retries: 5\n");
            content.push('\n');
        }

        if matches!(config.postgresql, PostgresqlType::CloudNativePg) {
            content.push_str("  postgres:\n");
            content.push_str("    image: pgvector/pgvector:pg16\n");
            content.push_str("    environment:\n");
            content.push_str("      POSTGRES_USER: aeterna\n");
            content.push_str("      POSTGRES_PASSWORD: aeterna\n");
            content.push_str("      POSTGRES_DB: aeterna\n");
            content.push_str("    ports:\n");
            content.push_str("      - \"5432:5432\"\n");
            content.push_str("    volumes:\n");
            content.push_str("      - postgres_data:/var/lib/postgresql/data\n");
            content.push_str("    healthcheck:\n");
            content.push_str("      test: [\"CMD-SHELL\", \"pg_isready -U aeterna\"]\n");
            content.push_str("      interval: 10s\n");
            content.push_str("      timeout: 5s\n");
            content.push_str("      retries: 5\n");
            content.push('\n');
        }

        if matches!(config.vector_backend, VectorBackend::Qdrant) {
            content.push_str("  qdrant:\n");
            content.push_str("    image: qdrant/qdrant:v1.12.0\n");
            content.push_str("    ports:\n");
            content.push_str("      - \"6333:6333\"\n");
            content.push_str("      - \"6334:6334\"\n");
            content.push_str("    volumes:\n");
            content.push_str("      - qdrant_data:/qdrant/storage\n");
            content.push_str("    healthcheck:\n");
            content.push_str(
                "      test: [\"CMD\", \"curl\", \"-f\", \"http://localhost:6333/health\"]\n",
            );
            content.push_str("      interval: 30s\n");
            content.push_str("      timeout: 10s\n");
            content.push_str("      retries: 3\n");
            content.push('\n');
        }

        if config.opal_enabled {
            content.push_str("  opal-server:\n");
            content.push_str("    image: permitio/opal-server:0.7.5\n");
            content.push_str("    environment:\n");
            content.push_str(
                "      OPAL_BROADCAST_URI: postgres://aeterna:aeterna@postgres:5432/aeterna\n",
            );
            content.push_str("      OPAL_LOG_LEVEL: INFO\n");
            content.push_str("    ports:\n");
            content.push_str("      - \"7002:7002\"\n");
            content.push_str("    depends_on:\n");
            content.push_str("      postgres:\n");
            content.push_str("        condition: service_healthy\n");
            content.push('\n');

            content.push_str("  cedar-agent:\n");
            content.push_str("    image: permitio/opal-client-cedar:0.7.5\n");
            content.push_str("    environment:\n");
            content.push_str("      OPAL_SERVER_URL: http://opal-server:7002\n");
            content.push_str("      OPAL_LOG_LEVEL: INFO\n");
            content.push_str("    ports:\n");
            content.push_str("      - \"7766:7766\"\n");
            content.push_str("    depends_on:\n");
            content.push_str("      - opal-server\n");
            content.push('\n');
        }
    }

    content.push_str("volumes:\n");
    if matches!(config.cache, CacheType::Dragonfly) {
        content.push_str("  dragonfly_data:\n");
    }
    if matches!(config.cache, CacheType::Valkey) {
        content.push_str("  valkey_data:\n");
    }
    if matches!(config.postgresql, PostgresqlType::CloudNativePg) {
        content.push_str("  postgres_data:\n");
    }
    if matches!(config.vector_backend, VectorBackend::Qdrant) {
        content.push_str("  qdrant_data:\n");
    }

    content
}

pub fn generate_helm_values(config: &SetupConfig) -> String {
    let mut content = String::new();

    content.push_str("global:\n");
    content.push_str("  imageRegistry: \"\"\n");
    content.push_str("  storageClass: \"\"\n");
    content.push('\n');

    content.push_str("aeterna:\n");
    content.push_str("  mode: ");
    content.push_str(&format!("{:?}\n", config.mode).to_lowercase());
    content.push_str("  replicas: 2\n");
    content.push_str("  image:\n");
    content.push_str("    repository: ghcr.io/kikokikok/aeterna\n");
    content.push_str("    tag: latest\n");
    content.push('\n');

    if let Some(url) = &config.central_url {
        content.push_str("  central:\n");
        content.push_str(&format!("    url: \"{}\"\n", url));
        content.push_str(&format!("    auth: {:?}\n", config.central_auth).to_lowercase());
    }
    content.push('\n');

    if let Some(hybrid) = &config.hybrid {
        content.push_str("  hybrid:\n");
        content.push_str(&format!(
            "    localCacheSizeMb: {}\n",
            hybrid.local_cache_size_mb
        ));
        content.push_str(&format!("    offlineCedar: {}\n", hybrid.offline_cedar));
        content.push_str(&format!(
            "    syncIntervalSecs: {}\n",
            hybrid.sync_interval_secs
        ));
        content.push('\n');
    }

    content.push_str("vectorBackend:\n");
    content.push_str(&format!("  type: {:?}\n", config.vector_backend).to_lowercase());

    match config.vector_backend {
        VectorBackend::Qdrant => {
            content.push_str("  qdrant:\n");
            content.push_str("    enabled: true\n");
        }
        VectorBackend::Pinecone => {
            if let Some(pc) = &config.pinecone {
                content.push_str("  pinecone:\n");
                content.push_str("    enabled: true\n");
                content.push_str(&format!("    environment: \"{}\"\n", pc.environment));
                content.push_str(&format!("    indexName: \"{}\"\n", pc.index_name));
            }
        }
        VectorBackend::Weaviate => {
            if let Some(wc) = &config.weaviate {
                content.push_str("  weaviate:\n");
                content.push_str("    enabled: true\n");
                content.push_str("    external:\n");
                content.push_str(&format!("      host: \"{}\"\n", wc.host));
            }
        }
        VectorBackend::Mongodb => {
            if let Some(mc) = &config.mongodb {
                content.push_str("  mongodb:\n");
                content.push_str("    enabled: true\n");
                content.push_str(&format!("    uri: \"{}\"\n", mc.connection_uri));
            }
        }
        VectorBackend::VertexAi => {
            if let Some(vc) = &config.vertex_ai {
                content.push_str("  vertexAi:\n");
                content.push_str("    enabled: true\n");
                content.push_str(&format!("    projectId: \"{}\"\n", vc.project_id));
                content.push_str(&format!("    region: \"{}\"\n", vc.region));
                content.push_str(&format!("    indexEndpoint: \"{}\"\n", vc.endpoint_url));
            }
        }
        VectorBackend::Databricks => {
            if let Some(dc) = &config.databricks {
                content.push_str("  databricks:\n");
                content.push_str("    enabled: true\n");
                content.push_str(&format!("    workspaceUrl: \"{}\"\n", dc.workspace_url));
                content.push_str(&format!("    catalog: \"{}\"\n", dc.catalog));
            }
        }
        _ => {}
    }
    content.push('\n');

    content.push_str("cache:\n");
    content.push_str(&format!("  type: {:?}\n", config.cache).to_lowercase());

    if matches!(config.cache, CacheType::Dragonfly) {
        content.push_str("  dragonfly:\n");
        content.push_str("    enabled: true\n");
    } else if matches!(config.cache, CacheType::Valkey) {
        content.push_str("  valkey:\n");
        content.push_str("    enabled: true\n");
    } else if let Some(redis) = &config.redis_external {
        content.push_str("  external:\n");
        content.push_str(&format!("    host: \"{}\"\n", redis.host));
        content.push_str(&format!("    port: {}\n", redis.port));
    }
    content.push('\n');

    content.push_str("postgresql:\n");
    content.push_str(&format!(
        "  enabled: {}\n",
        matches!(config.postgresql, PostgresqlType::CloudNativePg)
    ));
    if let Some(pg) = &config.pg_external {
        content.push_str("  external:\n");
        content.push_str(&format!("    host: \"{}\"\n", pg.host));
        content.push_str(&format!("    port: {}\n", pg.port));
        content.push_str(&format!("    database: \"{}\"\n", pg.database));
    }
    content.push('\n');

    content.push_str("opal:\n");
    content.push_str(&format!("  enabled: {}\n", config.opal_enabled));
    content.push('\n');

    content.push_str("llm:\n");
    content.push_str(&format!("  provider: {:?}\n", config.llm_provider).to_lowercase());
    content.push('\n');

    content.push_str("ingress:\n");
    content.push_str(&format!("  enabled: {}\n", config.ingress_enabled));
    if let Some(host) = &config.ingress_host {
        content.push_str("  hosts:\n");
        content.push_str(&format!("    - host: \"{}\"\n", host));
        content.push_str("      paths:\n");
        content.push_str("        - path: /\n");
        content.push_str("          pathType: Prefix\n");
    }
    content.push('\n');

    content.push_str("observability:\n");
    content.push_str("  serviceMonitor:\n");
    content.push_str(&format!(
        "    enabled: {}\n",
        config.service_monitor_enabled
    ));
    content.push('\n');

    content.push_str("security:\n");
    content.push_str("  networkPolicy:\n");
    content.push_str(&format!("    enabled: {}\n", config.network_policy_enabled));
    content.push('\n');

    content
}

fn generate_opencode_config(config: &SetupConfig) -> Result<Option<PathBuf>> {
    let cwd = std::env::current_dir()?;
    let opencode_jsonc_path = cwd.join("opencode.jsonc");
    backup_if_exists(&opencode_jsonc_path)?;

    let mcp_transport = if matches!(config.mode, DeploymentMode::Local) {
        r#"{
        "type": "stdio",
        "command": "aeterna-mcp"
      }"#
        .to_string()
    } else {
        let url = config
            .central_url
            .as_deref()
            .unwrap_or("http://localhost:8080");
        format!(
            r#"{{
        "type": "http",
        "url": "{}/mcp"
      }}"#,
            url
        )
    };

    let content = format!(
        r#"{{
  "$schema": "https://opencode.ai/schemas/opencode.json",
  "plugins": [
    "@kiko-aeterna/opencode-plugin"
  ],
  "mcpServers": {{
    "aeterna": {{
      "transport": {}
    }}
  }}
}}
"#,
        mcp_transport
    );

    fs::write(&opencode_jsonc_path, content)?;
    Ok(Some(opencode_jsonc_path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn default_local_config() -> SetupConfig {
        SetupConfig::default()
    }

    fn kubernetes_config() -> SetupConfig {
        SetupConfig {
            target: DeploymentTarget::Kubernetes,
            ..SetupConfig::default()
        }
    }

    fn hybrid_config() -> SetupConfig {
        SetupConfig {
            mode: DeploymentMode::Hybrid,
            central_url: Some("https://central.example.com".into()),
            hybrid: Some(HybridConfig {
                local_cache_size_mb: 1024,
                offline_cedar: false,
                sync_interval_secs: 60,
            }),
            ..SetupConfig::default()
        }
    }

    fn pinecone_config() -> SetupConfig {
        SetupConfig {
            vector_backend: VectorBackend::Pinecone,
            pinecone: Some(PineconeConfig {
                api_key: "pk-test-123".into(),
                environment: "us-east1-gcp".into(),
                index_name: "test-index".into(),
            }),
            ..SetupConfig::default()
        }
    }

    fn weaviate_config() -> SetupConfig {
        SetupConfig {
            vector_backend: VectorBackend::Weaviate,
            weaviate: Some(WeaviateConfig {
                host: "http://weaviate:8080".into(),
                api_key: Some("wk-test".into()),
            }),
            ..SetupConfig::default()
        }
    }

    fn mongodb_config() -> SetupConfig {
        SetupConfig {
            vector_backend: VectorBackend::Mongodb,
            mongodb: Some(MongodbConfig {
                connection_uri: "mongodb+srv://user:pass@cluster.mongodb.net/db".into(),
            }),
            ..SetupConfig::default()
        }
    }

    fn vertex_ai_config() -> SetupConfig {
        SetupConfig {
            vector_backend: VectorBackend::VertexAi,
            vertex_ai: Some(VertexAiConfig {
                project_id: "my-project".into(),
                region: "us-central1".into(),
                endpoint_url: "https://us-central1-aiplatform.googleapis.com".into(),
                service_account_json: None,
            }),
            ..SetupConfig::default()
        }
    }

    fn databricks_config() -> SetupConfig {
        SetupConfig {
            vector_backend: VectorBackend::Databricks,
            databricks: Some(DatabricksConfig {
                workspace_url: "https://adb-1234.azuredatabricks.net".into(),
                token: "dapi-test".into(),
                catalog: "main".into(),
            }),
            ..SetupConfig::default()
        }
    }

    #[test]
    fn test_validate_yaml_valid() {
        assert!(validate_yaml("key: value\n").is_ok());
        assert!(validate_yaml("services:\n  web:\n    image: nginx\n").is_ok());
    }

    #[test]
    fn test_validate_yaml_invalid() {
        assert!(validate_yaml("  : : invalid: [\n").is_err());
    }

    #[test]
    fn test_validate_toml_valid() {
        assert!(validate_toml("key = \"value\"\n").is_ok());
        assert!(validate_toml("[section]\nkey = 42\n").is_ok());
    }

    #[test]
    fn test_validate_toml_invalid() {
        assert!(validate_toml("[section\nkey = ").is_err());
    }

    #[test]
    fn test_generate_config_toml_default() {
        let config = default_local_config();
        let toml = generate_config_toml(&config);
        assert!(toml.contains("mode = \"local\""));
        assert!(toml.contains("backend = \"qdrant\""));
        assert!(toml.contains("type = \"dragonfly\""));
        validate_toml(&toml).expect("generated TOML must be valid");
    }

    #[test]
    fn test_generate_config_toml_hybrid() {
        let config = hybrid_config();
        let toml = generate_config_toml(&config);
        assert!(toml.contains("[hybrid]"));
        assert!(toml.contains("local_cache_size_mb = 1024"));
        assert!(toml.contains("offline_cedar = false"));
        assert!(toml.contains("sync_interval_secs = 60"));
        validate_toml(&toml).expect("generated TOML must be valid");
    }

    #[test]
    fn test_generate_config_toml_pinecone() {
        let config = pinecone_config();
        let toml = generate_config_toml(&config);
        assert!(toml.contains("backend = \"pinecone\""));
        assert!(toml.contains("pinecone_environment = \"us-east1-gcp\""));
        assert!(toml.contains("pinecone_index = \"test-index\""));
        validate_toml(&toml).expect("generated TOML must be valid");
    }

    #[test]
    fn test_generate_config_toml_weaviate() {
        let config = weaviate_config();
        let toml = generate_config_toml(&config);
        assert!(toml.contains("backend = \"weaviate\""));
        assert!(toml.contains("weaviate_host = \"http://weaviate:8080\""));
        validate_toml(&toml).expect("generated TOML must be valid");
    }

    #[test]
    fn test_generate_config_toml_mongodb() {
        let config = mongodb_config();
        let toml = generate_config_toml(&config);
        assert!(toml.contains("backend = \"mongodb\""));
        assert!(toml.contains("mongodb_uri"));
        validate_toml(&toml).expect("generated TOML must be valid");
    }

    #[test]
    fn test_generate_config_toml_vertex_ai() {
        let config = vertex_ai_config();
        let toml = generate_config_toml(&config);
        assert!(toml.contains("backend = \"vertexai\""));
        assert!(toml.contains("vertex_project = \"my-project\""));
        assert!(toml.contains("vertex_region = \"us-central1\""));
        validate_toml(&toml).expect("generated TOML must be valid");
    }

    #[test]
    fn test_generate_config_toml_databricks() {
        let config = databricks_config();
        let toml = generate_config_toml(&config);
        assert!(toml.contains("backend = \"databricks\""));
        assert!(toml.contains("databricks_workspace"));
        assert!(toml.contains("databricks_catalog = \"main\""));
        validate_toml(&toml).expect("generated TOML must be valid");
    }

    #[test]
    fn test_generate_docker_compose_default() {
        let config = default_local_config();
        let yaml = generate_docker_compose(&config);
        assert!(yaml.contains("services:"));
        assert!(yaml.contains("dragonfly:"));
        assert!(yaml.contains("qdrant:"));
        assert!(yaml.contains("postgres:"));
        validate_yaml(&yaml).expect("generated YAML must be valid");
    }

    #[test]
    fn test_generate_helm_values_default() {
        let config = kubernetes_config();
        let yaml = generate_helm_values(&config);
        assert!(yaml.contains("vectorBackend:"));
        assert!(yaml.contains("type: qdrant"));
        assert!(yaml.contains("qdrant:"));
        validate_yaml(&yaml).expect("generated YAML must be valid");
    }

    #[test]
    fn test_generate_helm_values_hybrid() {
        let config = SetupConfig {
            target: DeploymentTarget::Kubernetes,
            ..hybrid_config()
        };
        let yaml = generate_helm_values(&config);
        assert!(yaml.contains("hybrid:"));
        assert!(yaml.contains("localCacheSizeMb: 1024"));
        assert!(yaml.contains("offlineCedar: false"));
        assert!(yaml.contains("syncIntervalSecs: 60"));
        validate_yaml(&yaml).expect("generated YAML must be valid");
    }

    #[test]
    fn test_generate_helm_values_pinecone() {
        let config = SetupConfig {
            target: DeploymentTarget::Kubernetes,
            ..pinecone_config()
        };
        let yaml = generate_helm_values(&config);
        assert!(yaml.contains("pinecone:"));
        assert!(yaml.contains("environment: \"us-east1-gcp\""));
        assert!(yaml.contains("indexName: \"test-index\""));
        validate_yaml(&yaml).expect("generated YAML must be valid");
    }

    #[test]
    fn test_generate_helm_values_weaviate() {
        let config = SetupConfig {
            target: DeploymentTarget::Kubernetes,
            ..weaviate_config()
        };
        let yaml = generate_helm_values(&config);
        assert!(yaml.contains("weaviate:"));
        assert!(yaml.contains("host: \"http://weaviate:8080\""));
        validate_yaml(&yaml).expect("generated YAML must be valid");
    }

    #[test]
    fn test_generate_helm_values_mongodb() {
        let config = SetupConfig {
            target: DeploymentTarget::Kubernetes,
            ..mongodb_config()
        };
        let yaml = generate_helm_values(&config);
        assert!(yaml.contains("mongodb:"));
        assert!(yaml.contains("uri:"));
        validate_yaml(&yaml).expect("generated YAML must be valid");
    }

    #[test]
    fn test_generate_helm_values_vertex_ai() {
        let config = SetupConfig {
            target: DeploymentTarget::Kubernetes,
            ..vertex_ai_config()
        };
        let yaml = generate_helm_values(&config);
        assert!(yaml.contains("vertexAi:"));
        assert!(yaml.contains("projectId: \"my-project\""));
        validate_yaml(&yaml).expect("generated YAML must be valid");
    }

    #[test]
    fn test_generate_helm_values_databricks() {
        let config = SetupConfig {
            target: DeploymentTarget::Kubernetes,
            ..databricks_config()
        };
        let yaml = generate_helm_values(&config);
        assert!(yaml.contains("databricks:"));
        assert!(yaml.contains("catalog: \"main\""));
        validate_yaml(&yaml).expect("generated YAML must be valid");
    }

    #[test]
    fn test_generate_all_docker_compose() {
        let tmp = TempDir::new().expect("create temp dir");
        let config = default_local_config();
        let files = generate_all(&config, tmp.path()).expect("generate_all");
        assert!(files.iter().any(|f| f.ends_with("config.toml")));
        assert!(files.iter().any(|f| f.ends_with("docker-compose.yaml")));
    }

    #[test]
    fn test_generate_all_kubernetes() {
        let tmp = TempDir::new().expect("create temp dir");
        let config = kubernetes_config();
        let files = generate_all(&config, tmp.path()).expect("generate_all");
        assert!(files.iter().any(|f| f.ends_with("config.toml")));
        assert!(files.iter().any(|f| f.ends_with("values.yaml")));
    }

    #[test]
    fn test_generate_all_opencode_only() {
        let tmp = TempDir::new().expect("create temp dir");
        let config = SetupConfig {
            target: DeploymentTarget::OpencodeOnly,
            opencode_enabled: true,
            ..SetupConfig::default()
        };
        let files = generate_all(&config, tmp.path()).expect("generate_all");
        assert!(files.iter().any(|f| f.ends_with("config.toml")));
    }

    #[test]
    fn test_backup_if_exists_no_file() {
        let tmp = TempDir::new().expect("create temp dir");
        let path = tmp.path().join("nonexistent.yaml");
        backup_if_exists(&path).expect("should succeed for nonexistent file");
    }

    #[test]
    fn test_backup_if_exists_creates_backup() {
        let tmp = TempDir::new().expect("create temp dir");
        let path = tmp.path().join("test.yaml");
        fs::write(&path, "original").expect("write original");
        backup_if_exists(&path).expect("should create backup");
        let backup = path.with_extension("yaml.bak");
        assert!(backup.exists());
        assert_eq!(fs::read_to_string(backup).expect("read backup"), "original");
    }

    #[test]
    fn test_generate_docker_compose_external_redis() {
        let config = SetupConfig {
            cache: CacheType::External,
            redis_external: Some(ExternalRedisConfig {
                host: "redis.example.com".into(),
                port: 6380,
                password: Some("secret".into()),
            }),
            ..SetupConfig::default()
        };
        let yaml = generate_docker_compose(&config);
        assert!(!yaml.contains("dragonfly:"));
        assert!(!yaml.contains("valkey:"));
        validate_yaml(&yaml).expect("generated YAML must be valid");
    }

    #[test]
    fn test_generate_helm_values_external_postgres() {
        let config = SetupConfig {
            target: DeploymentTarget::Kubernetes,
            postgresql: PostgresqlType::External,
            pg_external: Some(ExternalPostgresConfig {
                host: "pg.example.com".into(),
                port: 5432,
                database: "mydb".into(),
                username: Some("admin".into()),
                password: Some("secret".into()),
            }),
            ..SetupConfig::default()
        };
        let yaml = generate_helm_values(&config);
        assert!(yaml.contains("enabled: false"));
        assert!(yaml.contains("host: \"pg.example.com\""));
        validate_yaml(&yaml).expect("generated YAML must be valid");
    }
}
