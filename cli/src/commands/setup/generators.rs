use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::types::*;

pub fn generate_all(config: &SetupConfig, output_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut generated = Vec::new();

    let aeterna_dir = output_dir.join(".aeterna");
    fs::create_dir_all(&aeterna_dir)?;

    let config_path = aeterna_dir.join("config.toml");
    backup_if_exists(&config_path)?;
    let config_content = generate_config_toml(config);
    fs::write(&config_path, config_content)?;
    generated.push(config_path);

    match config.target {
        DeploymentTarget::DockerCompose => {
            let compose_path = output_dir.join("docker-compose.yaml");
            backup_if_exists(&compose_path)?;
            let compose_content = generate_docker_compose(config);
            fs::write(&compose_path, compose_content)?;
            generated.push(compose_path);
        }
        DeploymentTarget::Kubernetes => {
            let values_path = output_dir.join("values.yaml");
            backup_if_exists(&values_path)?;
            let values_content = generate_helm_values(config);
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

fn backup_if_exists(path: &Path) -> Result<()> {
    if path.exists() {
        let backup_path = path.with_extension("yaml.bak");
        fs::copy(path, &backup_path)
            .with_context(|| format!("Failed to backup {}", path.display()))?;
    }
    Ok(())
}

fn generate_config_toml(config: &SetupConfig) -> String {
    let mut content = String::new();

    content.push_str(&format!("mode = \"{:?}\"\n", config.mode).to_lowercase());
    content.push('\n');

    if let Some(url) = &config.central_url {
        content.push_str("[central]\n");
        content.push_str(&format!("url = \"{}\"\n", url));
        content.push_str(&format!("auth = \"{:?}\"\n", config.central_auth).to_lowercase());
        content.push('\n');
    }

    content.push_str("[vector]\n");
    content.push_str(&format!("backend = \"{:?}\"\n", config.vector_backend).to_lowercase());
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

fn generate_docker_compose(config: &SetupConfig) -> String {
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
                "      test: [\"CMD\", \"curl\", \"-f\", \"http://localhost:6333/health\"]\n"
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
                "      OPAL_BROADCAST_URI: postgres://aeterna:aeterna@postgres:5432/aeterna\n"
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

fn generate_helm_values(config: &SetupConfig) -> String {
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

    content.push_str("vectorBackend:\n");
    content.push_str(&format!("  type: {:?}\n", config.vector_backend).to_lowercase());

    if matches!(config.vector_backend, VectorBackend::Qdrant) {
        content.push_str("  qdrant:\n");
        content.push_str("    enabled: true\n");
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
    "@aeterna/opencode-plugin"
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
